//! Approval requests: `send_draft` from an agent, `approve_request`, `reject_request`,
//! `list_approvals` and `--allow-send` (`AGENTS.md §4` "Outbound policy").

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{self, Mail, T0, of_kind};
use app_core::{
    Addr, ApprovalId, Command, CoreError, Ctx, DraftId, Effect, Engine, Event, MailboxRole, Origin,
    Outcome, PersistDelta, ProviderPreset,
};
use serde_json::json;

const WINDOW: i64 = 10_000;

fn account() -> Mail {
    Mail::new(
        ProviderPreset::Hostinger,
        &[
            ("INBOX", None),
            ("Sent", Some(MailboxRole::Sent)),
            ("Drafts", Some(MailboxRole::Drafts)),
        ],
    )
}

fn run(m: &mut Mail, cmd: Command, now_ms: i64, origin: Origin) -> Result<Outcome, CoreError> {
    m.seed += 1;
    let ctx = Ctx {
        now_ms,
        origin,
        ..support::ctx(m.seed + 1_000)
    };
    m.engine.dispatch(cmd, &ctx)
}

fn draft(m: &mut Mail, origin: Origin) -> DraftId {
    let account = m.account;
    let outcome = run(
        m,
        Command::CreateDraft {
            account,
            to: vec![Addr {
                name: None,
                email: "bob@example.org".into(),
            }],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Lunch?".into(),
            body: "Noon?".into(),
        },
        T0,
        origin,
    )
    .unwrap();
    serde_json::from_value(outcome.receipt.data["id"].clone()).unwrap()
}

/// An agent drafts a message and asks to send it.
fn requested(m: &mut Mail) -> (DraftId, ApprovalId) {
    let id = draft(m, Origin::Mcp);
    let outcome = run(m, Command::SendDraft { id }, T0 + 1_000, Origin::Mcp).unwrap();
    let approval = serde_json::from_value(outcome.receipt.data["approval"]["id"].clone()).unwrap();
    (id, approval)
}

fn persisted_approvals(outcome: &Outcome) -> (usize, usize) {
    outcome
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            } => Some((delta.approvals.len(), delta.removed_approvals.len())),
            _ => None,
        })
        .fold((0, 0), |(a, r), (x, y)| (a + x, r + y))
}

#[test]
fn an_agents_send_becomes_an_approval_request() {
    let mut m = account();
    let id = draft(&mut m, Origin::Mcp);
    let outcome = run(&mut m, Command::SendDraft { id }, T0 + 1_000, Origin::Mcp).unwrap();
    let data = &outcome.receipt.data;
    assert_eq!(data["state"], "awaiting_approval");
    assert_eq!(data["approval"]["draft"], json!(id));
    assert_eq!(data["approval"]["origin"], "mcp");
    assert_eq!(data["approval"]["requested_ms"], T0 + 1_000);
    assert_eq!(
        data["approval"]["message"]["untrusted_content"]["subject"],
        "Lunch?"
    );
    assert!(of_kind(&outcome.effects, "schedule_tick").is_empty());
    assert!(of_kind(&outcome.effects, "smtp_send").is_empty());
    assert_eq!(persisted_approvals(&outcome), (1, 0));
    assert_eq!(outcome.receipt.undo_label, None, "nothing to undo");
    let state = m.engine.state();
    assert_eq!(state.approvals.len(), 1);
    assert_eq!(state.drafts[&id].send_at, None, "nothing is scheduled");

    // Asking again is the same request.
    let again = run(&mut m, Command::SendDraft { id }, T0 + 2_000, Origin::Mcp).unwrap();
    assert_eq!(again.receipt.data["approval"]["id"], data["approval"]["id"]);
    assert_eq!(m.engine.state().approvals.len(), 1);

    // A draft that can't be sent is refused, not queued for approval.
    let account = m.account;
    let empty = run(
        &mut m,
        Command::CreateDraft {
            account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
        },
        T0,
        Origin::Mcp,
    )
    .unwrap();
    let empty: DraftId = serde_json::from_value(empty.receipt.data["id"].clone()).unwrap();
    let err = run(&mut m, Command::SendDraft { id: empty }, T0, Origin::Mcp).unwrap_err();
    assert!(matches!(err, CoreError::NotSendable { .. }), "{err:?}");
    assert_eq!(m.engine.state().approvals.len(), 1);
}

#[test]
fn approving_sends_like_the_users_own_send_and_undo_cancels_it() {
    let mut m = account();
    let (id, approval) = requested(&mut m);
    let outcome = run(
        &mut m,
        Command::ApproveRequest { id: approval },
        T0 + 5_000,
        Origin::Gui,
    )
    .unwrap();
    assert_eq!(outcome.receipt.data["approved"], json!(approval));
    assert_eq!(outcome.receipt.data["send_at"], T0 + 5_000 + WINDOW);
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Send"));
    assert!(outcome.effects.contains(&Effect::ScheduleTick {
        at_ms: T0 + 5_000 + WINDOW
    }));
    assert_eq!(persisted_approvals(&outcome), (0, 1));
    assert!(m.engine.state().approvals.is_empty());
    assert_eq!(
        m.engine.state().drafts[&id].send_at,
        Some(T0 + 5_000 + WINDOW)
    );

    // Undo cancels the send; redo is the user's send again.
    run(&mut m, Command::Undo {}, T0 + 6_000, Origin::Gui).unwrap();
    assert_eq!(m.engine.state().drafts[&id].send_at, None);
    assert!(
        m.engine.state().approvals.is_empty(),
        "the request is settled"
    );
    run(&mut m, Command::Redo {}, T0 + 7_000, Origin::Cli).unwrap();
    assert_eq!(
        m.engine.state().drafts[&id].send_at,
        Some(T0 + 7_000 + WINDOW)
    );

    // After the window the mail leaves.
    let ctx = Ctx {
        now_ms: T0 + 7_000 + WINDOW,
        origin: Origin::Runtime,
        ..support::ctx(9_999)
    };
    let tick = m
        .engine
        .ingest(
            Event::Tick {
                now_ms: T0 + 7_000 + WINDOW,
            },
            &ctx,
        )
        .unwrap();
    assert_eq!(of_kind(&tick.effects, "smtp_send").len(), 1);
}

#[test]
fn an_agent_redoing_an_approval_only_asks_again() {
    let mut m = account();
    let (id, approval) = requested(&mut m);
    run(
        &mut m,
        Command::ApproveRequest { id: approval },
        T0 + 2_000,
        Origin::Gui,
    )
    .unwrap();
    run(&mut m, Command::Undo {}, T0 + 3_000, Origin::Gui).unwrap();
    run(&mut m, Command::Redo {}, T0 + 4_000, Origin::Mcp).unwrap();
    assert_eq!(m.engine.state().drafts[&id].send_at, None);
    assert_eq!(m.engine.state().approvals.len(), 1);
}

#[test]
fn only_the_user_decides_requests() {
    let mut m = account();
    let (_, approval) = requested(&mut m);
    let before = m.engine.state().clone();
    for origin in [Origin::Mcp, Origin::McpAllowSend, Origin::Runtime] {
        for cmd in [
            Command::ApproveRequest { id: approval },
            Command::RejectRequest { id: approval },
        ] {
            let name = cmd.name();
            let err = run(&mut m, cmd, T0 + 2_000, origin).unwrap_err();
            assert_eq!(
                err,
                CoreError::OriginNotAllowed {
                    command: name.to_owned(),
                    origin: origin.as_str().to_owned(),
                }
            );
        }
    }
    assert_eq!(m.engine.state(), &before, "nothing changed");
    for origin in [Origin::Gui, Origin::Tui, Origin::Cli, Origin::Batch] {
        let missing = ApprovalId::from_uuid(uuid_of(42));
        let err = run(&mut m, Command::ApproveRequest { id: missing }, T0, origin).unwrap_err();
        assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "approval"));
        let err = run(&mut m, Command::RejectRequest { id: missing }, T0, origin).unwrap_err();
        assert!(matches!(err, CoreError::NotFound { .. }));
    }
}

#[test]
fn the_terminal_ui_decides_like_the_gui() {
    let mut m = account();
    let (id, approval) = requested(&mut m);
    let outcome = run(
        &mut m,
        Command::ApproveRequest { id: approval },
        T0 + 5_000,
        Origin::Tui,
    )
    .unwrap();
    assert_eq!(outcome.receipt.data["send_at"], T0 + 5_000 + WINDOW);
    assert!(m.engine.state().approvals.is_empty());
    assert_eq!(
        m.engine.state().drafts[&id].send_at,
        Some(T0 + 5_000 + WINDOW)
    );
    assert!(Origin::Tui.is_user() && !Origin::Tui.is_agent());
}

fn uuid_of(n: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(n)
}

#[test]
fn rejecting_keeps_the_draft_and_tells_the_agent() {
    let mut m = account();
    let (id, approval) = requested(&mut m);
    let undo_before = m.engine.history().undo_len();
    let outcome = run(
        &mut m,
        Command::RejectRequest { id: approval },
        T0 + 2_000,
        Origin::Cli,
    )
    .unwrap();
    assert_eq!(outcome.receipt.data["rejected"], json!(approval));
    assert_eq!(outcome.receipt.data["undoable"], false);
    assert_eq!(outcome.receipt.undo_label, None);
    assert_eq!(m.engine.history().undo_len(), undo_before, "not recorded");
    assert_eq!(persisted_approvals(&outcome), (0, 1));
    let state = m.engine.state();
    assert!(state.approvals.is_empty());
    assert_eq!(
        state.drafts[&id].send_error.as_deref(),
        Some("the user declined to send it")
    );
    assert!(of_kind(&outcome.effects, "smtp_send").is_empty());
}

#[test]
fn an_agent_editing_the_draft_withdraws_its_request_but_the_user_may_edit() {
    let mut m = account();
    let (id, _) = requested(&mut m);
    // The user tidies it up before approving: the request stays.
    let edit = Command::UpdateDraft {
        id,
        to: None,
        cc: None,
        bcc: None,
        subject: Some("Lunch at noon?".into()),
        body: None,
    };
    let outcome = run(&mut m, edit.clone(), T0 + 2_000, Origin::Gui).unwrap();
    assert!(outcome.receipt.data.get("approvals_withdrawn").is_none());
    assert_eq!(m.engine.state().approvals.len(), 1);
    // The agent changes it: the user would approve text they never saw, so it is withdrawn.
    let outcome = run(&mut m, edit, T0 + 3_000, Origin::Mcp).unwrap();
    assert_eq!(
        outcome.receipt.data["approvals_withdrawn"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(persisted_approvals(&outcome), (0, 1));
    assert!(m.engine.state().approvals.is_empty());

    // Attachments count as edits too.
    let (id, _) = requested(&mut m);
    run(
        &mut m,
        Command::AttachFile {
            draft: id,
            path: "/home/me/secret.txt".into(),
            index: None,
        },
        T0 + 4_000,
        Origin::Mcp,
    )
    .unwrap();
    assert!(m.engine.state().approvals.is_empty());
}

#[test]
fn requests_go_with_their_draft() {
    let mut m = account();
    let (id, _) = requested(&mut m);
    let outcome = run(
        &mut m,
        Command::DiscardDraft { id },
        T0 + 2_000,
        Origin::Mcp,
    )
    .unwrap();
    assert_eq!(persisted_approvals(&outcome), (0, 1));
    assert!(m.engine.state().approvals.is_empty());
    run(&mut m, Command::Undo {}, T0 + 3_000, Origin::Mcp).unwrap();
    assert!(m.engine.state().drafts.contains_key(&id));
    assert!(
        m.engine.state().approvals.is_empty(),
        "the agent has to ask again"
    );

    // The user sending it directly settles the request.
    let (id, _) = requested(&mut m);
    let outcome = run(&mut m, Command::SendDraft { id }, T0 + 4_000, Origin::Gui).unwrap();
    assert_eq!(persisted_approvals(&outcome), (0, 1));
    assert!(m.engine.state().approvals.is_empty());

    // A snapshot whose request lost its draft drops it.
    let (id, _) = requested(&mut m);
    let mut state = m.engine.state().clone();
    state.drafts.shift_remove(&id);
    assert!(Engine::new(state).state().approvals.is_empty());

    // Removing the account removes its requests.
    let _ = requested(&mut m);
    let account = m.account;
    run(
        &mut m,
        Command::RemoveAccount {
            id: account,
            confirm: true,
        },
        T0 + 5_000,
        Origin::Gui,
    )
    .unwrap();
    assert!(m.engine.state().approvals.is_empty());
}

#[test]
fn allow_send_and_undo_by_an_agent() {
    let mut m = account();
    // `dail mcp --allow-send`: the agent's send leaves without approval.
    let id = draft(&mut m, Origin::McpAllowSend);
    let outcome = run(
        &mut m,
        Command::SendDraft { id },
        T0 + 1_000,
        Origin::McpAllowSend,
    )
    .unwrap();
    assert_eq!(outcome.receipt.data["send_at"], T0 + 1_000 + WINDOW);
    assert!(m.engine.state().approvals.is_empty());

    // Undoing the user's cancelled send from MCP doesn't send either: it asks.
    let id = draft(&mut m, Origin::Gui);
    run(&mut m, Command::SendDraft { id }, T0 + 2_000, Origin::Gui).unwrap();
    run(&mut m, Command::CancelSend { id }, T0 + 3_000, Origin::Gui).unwrap();
    run(&mut m, Command::Undo {}, T0 + 4_000, Origin::Mcp).unwrap();
    assert_eq!(m.engine.state().drafts[&id].send_at, None);
    assert!(
        m.engine
            .state()
            .approvals
            .values()
            .any(|approval| approval.draft == id)
    );
}

#[test]
fn list_approvals_shows_each_request_with_its_draft() {
    let mut m = account();
    let empty = run(&mut m, Command::ListApprovals {}, T0, Origin::Mcp).unwrap();
    assert_eq!(empty.receipt.data, json!({ "approvals": [] }));
    let (id, approval) = requested(&mut m);
    let before = m.engine.state().clone();
    let listed = run(&mut m, Command::ListApprovals {}, T0 + 2_000, Origin::Cli).unwrap();
    assert_eq!(m.engine.state(), &before, "a query changes nothing");
    let entry = &listed.receipt.data["approvals"][0];
    assert_eq!(entry["id"], json!(approval));
    assert_eq!(entry["draft"], json!(id));
    assert_eq!(
        entry["message"]["untrusted_content"]["to"][0]["email"],
        "bob@example.org"
    );
}
