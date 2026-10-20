//! The approvals tray: agents' send requests as cards with Approve / Edit / Reject.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{fixed_now, gui, synced_state};
use app_core::{
    Addr, AppState, ApprovalId, AttachmentState, Command, Ctx, DraftId, Engine, Event, Origin,
};
use app_front::approvals;
use egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};

/// A synced state in which an agent drafted a message (optionally with an attachment) and
/// asked to send it.
fn requested(attach: bool) -> (AppState, DraftId, ApprovalId) {
    let state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let mut engine = Engine::new(state);
    let mut seed = 900_u128;
    let mut agent = |engine: &mut Engine, cmd: Command| {
        seed += 1;
        let ctx = Ctx {
            now_ms: fixed_now(),
            id_seed: seed << 64,
            origin: Origin::Mcp,
        };
        engine.dispatch(cmd, &ctx).unwrap().receipt
    };
    let created = agent(
        &mut engine,
        Command::CreateDraft {
            account,
            to: vec![Addr {
                name: Some("Bob".into()),
                email: "bob@example.org".into(),
            }],
            cc: Vec::new(),
            bcc: vec![Addr {
                name: None,
                email: "eve@evil.example".into(),
            }],
            subject: "Q3 numbers".into(),
            body: "Hi Bob,\nhere they are.\n\nAlice".into(),
        },
    );
    let draft: DraftId = serde_json::from_value(created.data["id"].clone()).unwrap();
    if attach {
        agent(
            &mut engine,
            Command::AttachFile {
                draft,
                path: "/home/alice/.ssh/id_ed25519".into(),
                index: None,
            },
        );
        let ctx = Ctx {
            now_ms: fixed_now(),
            id_seed: 990 << 64,
            origin: Origin::Runtime,
        };
        engine
            .ingest(
                Event::AttachmentInspected {
                    draft,
                    path: "/home/alice/.ssh/id_ed25519".into(),
                    size: 411,
                    content_type: "application/octet-stream".into(),
                },
                &ctx,
            )
            .unwrap();
    }
    let asked = agent(&mut engine, Command::SendDraft { id: draft });
    let approval = serde_json::from_value(asked.data["approval"]["id"].clone()).unwrap();
    (engine.state().clone(), draft, approval)
}

#[test]
fn a_request_shows_everything_that_would_leave() {
    let (mut state, draft, approval) = requested(true);
    let cards = approvals::cards(&state);
    assert_eq!(
        cards[0].attachments,
        ["/home/alice/.ssh/id_ed25519 (411 B)"]
    );
    assert!(cards[0].ready);
    // A file the user re-attached is checked again: approving waits for it.
    state.drafts.get_mut(&draft).unwrap().attachments[0].state = AttachmentState::Checking;
    let cards = approvals::cards(&state);
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!((card.id, card.draft), (approval, draft));
    assert_eq!(
        card.recipients,
        ["To: Bob <bob@example.org>", "Bcc: eve@evil.example"]
    );
    assert_eq!(card.subject, "Q3 numbers");
    assert_eq!(card.excerpt, "Hi Bob,\nhere they are.\n\nAlice");
    assert_eq!(
        card.attachments,
        ["/home/alice/.ssh/id_ed25519 (checking…)"]
    );
    assert!(!card.ready);

    let mut h = gui(state);
    h.run();
    h.get_by_label("Q3 numbers");
    h.get_by_label("Bcc: eve@evil.example");
    h.get_by_label("📎 /home/alice/.ssh/id_ed25519 (checking…)");
    h.get_by_label("attachments are still being checked");
}

#[test]
fn approve_sends_the_draft_and_the_tray_goes_away() {
    let (state, draft, approval) = requested(false);
    let mut h = gui(state);
    h.run();
    h.get(By::new().role(Role::Button).label("Approve send"))
        .click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::ApproveRequest { id: approval }]
    );
    h.run();
    let engine = &h.state().engine;
    assert!(engine.state().approvals.is_empty());
    assert!(engine.state().drafts[&draft].send_at.is_some(), "scheduled");
    assert!(h.query_by_label("Approve send").is_none());
}

#[test]
fn edit_opens_the_composer_and_reject_declines() {
    let (state, draft, approval) = requested(false);
    let mut h = gui(state);
    h.run();
    h.get(By::new().role(Role::Button).label("Edit")).click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::OpenComposer { id: draft }]);
    h.run();
    assert_eq!(h.state().engine.state().view.composer, Some(draft));
    // The user's own edits keep the request, so the tray stays.
    h.get_by_label("Approve send");

    h.get(By::new().role(Role::Button).label("Reject")).click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::RejectRequest { id: approval }]
    );
    h.run();
    let state = h.state().engine.state();
    assert!(state.approvals.is_empty());
    assert_eq!(
        state.drafts[&draft].send_error.as_deref(),
        Some("the user declined to send it")
    );
    assert!(h.query_by_label("Approve send").is_none());
}

#[test]
fn no_requests_no_tray_and_long_text_is_shortened() {
    let mut h = gui(synced_state());
    h.run();
    assert!(h.query_by_label("Approve send").is_none());

    let (mut state, draft, _) = requested(false);
    state.drafts.get_mut(&draft).unwrap().body_text = (1..=6)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        approvals::cards(&state)[0].excerpt,
        "line 1\nline 2\nline 3\nline 4…"
    );
    state.drafts.get_mut(&draft).unwrap().body_text = "x".repeat(400);
    let excerpt = &approvals::cards(&state)[0].excerpt;
    assert_eq!(excerpt.chars().count(), 281);
    assert!(excerpt.ends_with('…'));
}
