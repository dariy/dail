//! `list_pending_ops`, `retry_pending_ops`, `discard_pending_op`.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{self, Mail, mail, names, of_kind};
use app_core::ops::MAX_ATTEMPTS;
use app_core::{
    AccountId, Command, CommandKind, CoreError, Ctx, Effect, Event, MailboxRole, MessageId, OpId,
    OpState, ProviderPreset,
};
use serde_json::json;

/// INBOX with UIDs 1–3 (all unread) and a fetched Archive.
fn mailbox() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "One", false),
            mail(2, "b@x", None, "Two", false),
            mail(3, "c@x", None, "Three", false),
        ],
    );
    m.fetch("Archive", vec![]);
    m
}

fn read(m: &mut Mail, uid: u32) -> OpId {
    let id = m.message("INBOX", uid);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![id],
        })
        .unwrap();
    serde_json::from_value(outcome.receipt.data["ops"][0].clone()).unwrap()
}

fn fail(m: &mut Mail, op: OpId, now_ms: i64) {
    m.engine
        .ingest(
            Event::RemoteOpFailed {
                op,
                error: "IMAP UID STORE in INBOX failed: NO [SERVERBUG]".into(),
                permanent: true,
            },
            &Ctx {
                now_ms,
                ..support::ctx(90)
            },
        )
        .unwrap();
}

/// Makes `op` (the running one) fail until it is given up.
fn give_up(m: &mut Mail, op: OpId) {
    let mut now = support::T0 + 100_000;
    for attempt in 1..=MAX_ATTEMPTS {
        fail(m, op, now);
        if attempt < MAX_ATTEMPTS {
            now += app_core::ops::retry_delay_ms(attempt);
            m.engine
                .ingest(Event::Tick { now_ms: now }, &support::ctx(91))
                .unwrap();
        }
    }
    assert_eq!(m.engine.state().outbox[&op].state, OpState::Failed);
}

#[test]
fn list_pending_ops_shows_state_attempts_and_errors() {
    let mut m = mailbox();
    let first = read(&mut m, 1);
    let second = read(&mut m, 2);
    fail(&mut m, first, support::T0 + 50_000);
    let outcome = m.run(Command::ListPendingOps { account: None }).unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::Query));
    let ops = outcome.receipt.data["ops"].as_array().unwrap();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0]["id"], json!(first));
    assert_eq!(ops[0]["action"], "set_flag");
    assert_eq!(ops[0]["detail"]["flag"], "seen");
    assert_eq!(ops[0]["state"], "queued");
    assert_eq!(ops[0]["attempts"], 1);
    assert_eq!(ops[0]["next_attempt_ms"], support::T0 + 52_000);
    assert!(ops[0]["last_error"].as_str().unwrap().contains("SERVERBUG"));
    assert_eq!(ops[1]["id"], json!(second));
    assert_eq!(ops[1]["last_error"], json!(null));

    let account = m.account;
    let mine = m
        .run(Command::ListPendingOps {
            account: Some(account),
        })
        .unwrap();
    assert_eq!(mine.receipt.data["ops"].as_array().unwrap().len(), 2);
    let err = m
        .run(Command::ListPendingOps {
            account: Some(AccountId::from_uuid(uuid::Uuid::from_u128(5))),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "account"));
}

#[test]
fn retry_runs_waiting_ops_now_and_gives_failed_ops_one_more_attempt() {
    let mut m = mailbox();
    let first = read(&mut m, 1);
    fail(&mut m, first, support::T0 + 50_000);
    let retry = m.run(Command::RetryPendingOps { id: None }).unwrap();
    assert_eq!(retry.receipt.data["retried"], json!([first]));
    assert_eq!(retry.receipt.undo_label, None, "not recorded");
    assert!(matches!(
        of_kind(&retry.effects, "run_remote_op")[..],
        [Effect::RunRemoteOp { op, .. }] if *op == first
    ));

    give_up(&mut m, first);
    let retry = m.run(Command::RetryPendingOps { id: Some(first) }).unwrap();
    assert_eq!(retry.receipt.data["retried"], json!([first]));
    assert_eq!(m.engine.state().outbox[&first].state, OpState::Running);
    // One more failure gives it up again.
    fail(&mut m, first, support::T0 + 900_000);
    assert_eq!(m.engine.state().outbox[&first].state, OpState::Failed);

    let all = m.run(Command::RetryPendingOps { id: None }).unwrap();
    assert_eq!(all.receipt.data["retried"], json!([first]));
    let err = m
        .run(Command::RetryPendingOps {
            id: Some(OpId::from_uuid(uuid::Uuid::from_u128(5))),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "op"));
}

#[test]
fn discard_reverts_a_flag_change_and_resyncs() {
    let mut m = mailbox();
    let first = read(&mut m, 1);
    let second = read(&mut m, 2);
    let two = m.message("INBOX", 2);
    assert!(m.engine.state().messages[&two].flags.seen);
    let discard = m.run(Command::DiscardPendingOp { id: second }).unwrap();
    assert_eq!(discard.receipt.data["reverted"], json!([two]));
    assert_eq!(discard.receipt.undo_label, None);
    assert!(!m.engine.state().messages[&two].flags.seen);
    assert!(!m.engine.state().outbox.contains_key(&second));
    assert_eq!(
        names(&discard.effects),
        ["persist_state", "fetch_envelopes"]
    );
    assert!(m.engine.state().outbox.contains_key(&first));

    let err = m.run(Command::DiscardPendingOp { id: first }).unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "id"));
    let err = m
        .run(Command::DiscardPendingOp {
            id: OpId::from_uuid(uuid::Uuid::from_u128(5)),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "op"));
}

#[test]
fn discarding_a_failed_head_op_lets_the_queue_continue() {
    let mut m = mailbox();
    let first = read(&mut m, 1);
    let second = read(&mut m, 2);
    fail(&mut m, first, support::T0 + 50_000);
    let discard = m.run(Command::DiscardPendingOp { id: first }).unwrap();
    assert!(matches!(
        of_kind(&discard.effects, "run_remote_op")[..],
        [Effect::RunRemoteOp { op, .. }] if *op == second
    ));
}

#[test]
fn discard_reverts_moves_and_drops_copy_placeholders() {
    let mut m = mailbox();
    read(&mut m, 3); // keeps the queue busy, so the archive op stays unsent
    let one = m.message("INBOX", 1);
    let thread = m.thread_of(one);
    let archived = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    let op: OpId = serde_json::from_value(archived.receipt.data["ops"][0].clone()).unwrap();
    let discard = m.run(Command::DiscardPendingOp { id: op }).unwrap();
    assert_eq!(discard.receipt.data["reverted"], json!([one]));
    assert_eq!(m.engine.state().messages[&one].moved_to, None);

    let mut g = Mail::new(
        ProviderPreset::Gmail,
        &[
            ("INBOX", None),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
        ],
    );
    g.fetch("INBOX", vec![mail(1, "z@x", None, "Busy", false)]);
    g.fetch(
        "[Gmail]/All Mail",
        vec![mail(40, "a@x", None, "Plans", true)],
    );
    let busy = g.message("INBOX", 1);
    g.run(Command::Star {
        ids: vec![],
        messages: vec![busy],
    })
    .unwrap();
    let source = g.message("[Gmail]/All Mail", 40);
    let inbox = g.mailbox("INBOX");
    let copied = g
        .run(Command::MoveThreads {
            ids: vec![],
            messages: vec![source],
            to: inbox,
        })
        .unwrap();
    let copy: MessageId = serde_json::from_value(copied.receipt.data["copied"][0].clone()).unwrap();
    let op: OpId = serde_json::from_value(copied.receipt.data["ops"][0].clone()).unwrap();
    g.run(Command::DiscardPendingOp { id: op }).unwrap();
    assert!(!g.engine.state().messages.contains_key(&copy));
    assert!(g.engine.state().messages.contains_key(&source));
}
