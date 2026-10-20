//! `send_draft` / `cancel_send`: the undo-send window, tick-driven dispatch and results. The
//! approval rule for agents is in `approvals.rs`.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{self, Mail, T0, of_kind};
use app_core::{
    Addr, Command, CommandKind, CoreError, Ctx, DraftId, Effect, Engine, Event, MailboxRole,
    Origin, Outcome, ProviderPreset, SettingKey,
};

const WINDOW: i64 = 10_000;

fn account(preset: ProviderPreset) -> Mail {
    Mail::new(
        preset,
        &[
            ("INBOX", None),
            ("Sent", Some(MailboxRole::Sent)),
            ("Drafts", Some(MailboxRole::Drafts)),
        ],
    )
}

fn at(m: &Mail, now_ms: i64, origin: Origin) -> Ctx {
    Ctx {
        now_ms,
        origin,
        ..support::ctx(m.seed + 100)
    }
}

fn dispatch(m: &mut Mail, cmd: Command, now_ms: i64) -> Result<Outcome, CoreError> {
    let ctx = at(m, now_ms, Origin::Gui);
    m.engine.dispatch(cmd, &ctx)
}

fn ingest(m: &mut Mail, event: Event, now_ms: i64) -> Outcome {
    let ctx = at(m, now_ms, Origin::Runtime);
    m.engine.ingest(event, &ctx).unwrap()
}

fn draft(m: &mut Mail) -> DraftId {
    let account = m.account;
    let outcome = dispatch(
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
    )
    .unwrap();
    serde_json::from_value(outcome.receipt.data["id"].clone()).unwrap()
}

fn send(m: &mut Mail, id: DraftId, now_ms: i64) -> Outcome {
    dispatch(m, Command::SendDraft { id }, now_ms).unwrap()
}

fn tick(m: &mut Mail, now_ms: i64) -> Outcome {
    ingest(m, Event::Tick { now_ms }, now_ms)
}

fn smtp(outcome: &Outcome) -> Vec<&Effect> {
    of_kind(&outcome.effects, "smtp_send")
}

fn not_sendable(result: Result<Outcome, CoreError>, needle: &str) {
    match result {
        Err(CoreError::NotSendable { reason, .. }) => {
            assert!(reason.contains(needle), "{reason}");
        }
        other => panic!("expected not_sendable, got {other:?}"),
    }
}

#[test]
fn send_draft_schedules_the_send_after_the_undo_window() {
    let mut m = account(ProviderPreset::Hostinger);
    let id = draft(&mut m);
    let outcome = send(&mut m, id, T0 + 1_000);
    assert_eq!(outcome.receipt.kind, Some(CommandKind::Outbound));
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Send"));
    assert_eq!(outcome.receipt.data["send_at"], T0 + 1_000 + WINDOW);
    assert!(outcome.effects.contains(&Effect::ScheduleTick {
        at_ms: T0 + 1_000 + WINDOW
    }));
    assert!(smtp(&outcome).is_empty());
    let draft = &m.engine.state().drafts[&id];
    assert_eq!(draft.send_at, Some(T0 + 1_000 + WINDOW));
    assert!(!draft.sending);

    // Scheduled drafts can't be changed.
    not_sendable(
        dispatch(
            &mut m,
            Command::UpdateDraft {
                id,
                to: None,
                cc: None,
                bcc: None,
                subject: Some("x".into()),
                body: None,
            },
            T0 + 2_000,
        ),
        "cancel_send first",
    );
    not_sendable(
        dispatch(&mut m, Command::DiscardDraft { id }, T0 + 2_000),
        "cancel_send first",
    );
    not_sendable(
        dispatch(
            &mut m,
            Command::AttachFile {
                draft: id,
                path: "/a.pdf".into(),
                index: None,
            },
            T0 + 2_000,
        ),
        "cancel_send first",
    );
    not_sendable(
        dispatch(&mut m, Command::SendDraft { id }, T0 + 2_000),
        "scheduled",
    );
    // A scheduled draft is not saved to Drafts meanwhile.
    assert!(of_kind(&tick(&mut m, T0 + 9_000).effects, "save_draft").is_empty());
}

#[test]
fn send_draft_checks_recipients_and_attachments() {
    let mut m = account(ProviderPreset::Hostinger);
    let account_id = m.account;
    let empty = dispatch(
        &mut m,
        Command::CreateDraft {
            account: account_id,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
        },
        T0,
    )
    .unwrap();
    let empty: DraftId = serde_json::from_value(empty.receipt.data["id"].clone()).unwrap();
    not_sendable(
        dispatch(&mut m, Command::SendDraft { id: empty }, T0),
        "no recipients",
    );

    let id = draft(&mut m);
    dispatch(
        &mut m,
        Command::AttachFile {
            draft: id,
            path: "/q3.pdf".into(),
            index: None,
        },
        T0,
    )
    .unwrap();
    not_sendable(
        dispatch(&mut m, Command::SendDraft { id }, T0),
        "still being checked",
    );
    ingest(
        &mut m,
        Event::AttachmentFailed {
            draft: id,
            path: "/q3.pdf".into(),
            error: "gone".into(),
        },
        T0,
    );
    not_sendable(
        dispatch(&mut m, Command::SendDraft { id }, T0),
        "remove it with remove_attachment",
    );
    let missing = DraftId::from_uuid(uuid::Uuid::from_u128(4));
    let err = dispatch(&mut m, Command::SendDraft { id: missing }, T0).unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "draft"));
}

#[test]
fn undo_inside_the_window_cancels_and_redo_schedules_again() {
    let mut m = account(ProviderPreset::Hostinger);
    let id = draft(&mut m);
    send(&mut m, id, T0 + 1_000);
    let undo = dispatch(&mut m, Command::Undo {}, T0 + 5_000).unwrap();
    assert_eq!(undo.receipt.data["undone"], "Send");
    assert_eq!(m.engine.state().drafts[&id].send_at, None);
    assert!(
        smtp(&tick(&mut m, T0 + 1_000 + WINDOW)).is_empty(),
        "cancelled"
    );
    let redo = dispatch(&mut m, Command::Redo {}, T0 + 20_000).unwrap();
    assert!(redo.effects.contains(&Effect::ScheduleTick {
        at_ms: T0 + 20_000 + WINDOW
    }));
    assert_eq!(
        m.engine.state().drafts[&id].send_at,
        Some(T0 + 20_000 + WINDOW)
    );
}

#[test]
fn the_tick_at_send_at_hands_it_to_smtp_and_makes_it_irreversible() {
    let mut m = account(ProviderPreset::Hostinger);
    m.run(Command::SetSetting {
        key: SettingKey::PollSecs,
        value: 600.into(),
    })
    .unwrap();
    let id = draft(&mut m);
    send(&mut m, id, T0 + 1_000);
    assert!(smtp(&tick(&mut m, T0 + WINDOW)).is_empty(), "too early");
    let due = tick(&mut m, T0 + 1_000 + WINDOW);
    let [
        Effect::SmtpSend {
            draft,
            smtp: login,
            message,
            sent_copy,
            ..
        },
    ] = smtp(&due)[..]
    else {
        panic!("{:?}", due.effects);
    };
    assert_eq!(*draft, id);
    assert_eq!(login.login, "me@example.com");
    assert_eq!(login.server.host, "smtp.example.com");
    assert_eq!(message.to[0].email, "bob@example.org");
    assert_eq!(message.date_ms, T0 + 1_000 + WINDOW);
    assert_eq!(
        sent_copy.as_ref().map(|c| c.path.as_str()),
        Some("Sent"),
        "Hostinger: the client stores the copy"
    );
    assert!(m.engine.state().drafts[&id].sending);
    not_sendable(
        dispatch(&mut m, Command::CancelSend { id }, T0 + 12_000),
        "can't be stopped",
    );
    // Every step about the draft is gone; the setting change before it can still be undone.
    let undo = dispatch(&mut m, Command::Undo {}, T0 + 12_000).unwrap();
    assert_eq!(undo.receipt.data["undone"], "Set poll_secs");
    assert_eq!(
        m.engine.history().undo_label(),
        Some("Add account me@example.com"),
        "only the step from before the draft is left"
    );
    assert!(smtp(&tick(&mut m, T0 + 13_000)).is_empty(), "sent once");
}

#[test]
fn gmail_saves_the_sent_copy_itself() {
    let mut m = account(ProviderPreset::Gmail);
    let id = draft(&mut m);
    send(&mut m, id, T0);
    let due = tick(&mut m, T0 + WINDOW);
    assert!(matches!(
        smtp(&due)[..],
        [Effect::SmtpSend {
            sent_copy: None,
            ..
        }]
    ));
    let _ = id;
}

#[test]
fn a_zero_window_sends_at_once_without_undo() {
    let mut m = account(ProviderPreset::Hostinger);
    m.run(Command::SetSetting {
        key: SettingKey::UndoSendMs,
        value: 0.into(),
    })
    .unwrap();
    let id = draft(&mut m);
    let outcome = send(&mut m, id, T0 + 1_000);
    assert_eq!(smtp(&outcome).len(), 1);
    assert_eq!(outcome.receipt.undo_label, None);
    assert!(m.engine.state().drafts[&id].sending);
}

#[test]
fn message_sent_removes_the_draft_and_its_server_copy() {
    let mut m = account(ProviderPreset::Hostinger);
    let id = draft(&mut m);
    // Saved to Drafts first.
    tick(&mut m, T0 + 5_000);
    ingest(
        &mut m,
        Event::DraftSaved {
            draft: id,
            revision: T0,
        },
        T0 + 5_500,
    );
    send(&mut m, id, T0 + 6_000);
    tick(&mut m, T0 + 6_000 + WINDOW);
    let sent = ingest(
        &mut m,
        Event::MessageSent {
            draft: id,
            sent_copy_error: Some("IMAP APPEND to Sent failed: NO quota".into()),
        },
        T0 + 17_000,
    );
    assert!(m.engine.state().drafts.is_empty());
    assert_eq!(of_kind(&sent.effects, "delete_draft").len(), 1);
    let account = m.account;
    let issue = m.engine.state().status.sync[&account]
        .last_error
        .clone()
        .unwrap();
    assert!(
        issue
            .error
            .starts_with("sent, but saving the copy in Sent failed")
    );
}

#[test]
fn a_failed_send_gives_the_draft_back_with_the_error() {
    let mut m = account(ProviderPreset::Hostinger);
    let id = draft(&mut m);
    send(&mut m, id, T0);
    tick(&mut m, T0 + WINDOW);
    ingest(
        &mut m,
        Event::SendFailed {
            draft: id,
            error: "SMTP smtp.example.com:465: 550 relay denied".into(),
        },
        T0 + 11_000,
    );
    let draft = &m.engine.state().drafts[&id];
    assert!(!draft.is_outgoing());
    assert!(
        draft
            .send_error
            .as_deref()
            .unwrap()
            .contains("relay denied")
    );
    // Editable and sendable again.
    send(&mut m, id, T0 + 12_000);
    assert_eq!(m.engine.state().drafts[&id].send_error, None);
}

#[test]
fn cancel_send_needs_a_scheduled_send_and_is_undoable() {
    let mut m = account(ProviderPreset::Hostinger);
    let id = draft(&mut m);
    not_sendable(
        dispatch(&mut m, Command::CancelSend { id }, T0),
        "no send is scheduled",
    );
    send(&mut m, id, T0);
    let cancel = dispatch(&mut m, Command::CancelSend { id }, T0 + 1_000).unwrap();
    assert_eq!(cancel.receipt.undo_label.as_deref(), Some("Cancel send"));
    assert_eq!(m.engine.state().drafts[&id].send_at, None);
    dispatch(&mut m, Command::Undo {}, T0 + 2_000).unwrap();
    assert_eq!(
        m.engine.state().drafts[&id].send_at,
        Some(T0 + 2_000 + WINDOW)
    );
}

#[test]
fn a_restart_never_sends_behind_the_users_back() {
    let mut m = account(ProviderPreset::Hostinger);
    let scheduled = draft(&mut m);
    let running = draft(&mut m);
    send(&mut m, running, T0);
    tick(&mut m, T0 + WINDOW);
    send(&mut m, scheduled, T0 + WINDOW);
    let mut engine = Engine::new(m.engine.state().clone());
    for id in [scheduled, running] {
        let draft = &engine.state().drafts[&id];
        assert!(!draft.is_outgoing());
        assert!(draft.send_error.is_some());
    }
    let tick = engine
        .ingest(
            Event::Tick {
                now_ms: T0 + 100_000,
            },
            &support::ctx(400),
        )
        .unwrap();
    assert!(smtp(&tick).is_empty());
}
