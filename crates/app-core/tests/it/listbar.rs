//! The list toolbar's commands (UX-7): `mark_mailbox_read` and `set_list_filter`.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, of_kind};
use app_core::{Command, CoreError, Effect, Event, MailboxId, RemoteOp, RemoteStep, listing};
use serde_json::json;

/// INBOX with two unread messages (UIDs 1, 2) and one read (UID 3); the server counts
/// `server_unread` unread.
fn inbox(server_unread: u32) -> (Mail, MailboxId) {
    let mut m = Mail::hostinger();
    let inbox = m.mailbox("INBOX");
    m.feed(Event::EnvelopesFetched {
        account: m.account,
        mailbox: inbox,
        uid_validity: 7,
        uid_next: Some(4),
        total: 40,
        unread: server_unread,
        envelopes: vec![
            mail(1, "a@x", None, "One", false),
            mail(2, "b@x", None, "Two", false),
            mail(3, "c@x", None, "Three", true),
        ],
    });
    (m, inbox)
}

#[test]
fn marks_the_cached_unread_and_undo_restores_them() {
    let (mut m, inbox) = inbox(2);
    let outcome = m.run(Command::MarkMailboxRead { id: inbox }).unwrap();
    let data = &outcome.receipt.data;
    assert_eq!(data["uncached_affected"], false);
    assert_eq!(data["changed"].as_array().unwrap().len(), 2);
    assert!(of_kind(&outcome.effects, "run_remote_op").len() == 1);
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Mark all read (2 messages)")
    );
    let state = m.engine.state();
    assert!(state.messages.values().all(|e| e.flags.seen));
    assert_eq!(state.mailboxes[&inbox].unread, 0);
    m.run(Command::Undo {}).unwrap();
    let unread = m
        .engine
        .state()
        .messages
        .values()
        .filter(|e| !e.flags.seen)
        .count();
    assert_eq!(unread, 2);
}

#[test]
fn uncached_unread_mail_is_marked_on_the_server_too() {
    let (mut m, inbox) = inbox(30);
    let outcome = m.run(Command::MarkMailboxRead { id: inbox }).unwrap();
    assert_eq!(outcome.receipt.data["uncached_affected"], true);
    assert_eq!(outcome.receipt.data["ops"].as_array().unwrap().len(), 2);
    let ops: Vec<&RemoteOp> = m.engine.state().outbox.values().map(|o| &o.op).collect();
    assert!(ops.iter().any(
        |op| matches!(op, RemoteOp::FlagMailbox { mailbox, value: true, .. } if *mailbox == inbox)
    ));
    // Once the first op is done, the second runs as `UID STORE 1:*`.
    let first = of_kind(&outcome.effects, "run_remote_op")[0].clone();
    let Effect::RunRemoteOp { op, .. } = first else {
        panic!()
    };
    let next = m.feed(Event::RemoteOpSucceeded { op });
    let run = of_kind(&next.effects, "run_remote_op");
    let Effect::RunRemoteOp { steps, .. } = run[0] else {
        panic!()
    };
    assert!(
        matches!(&steps[..], [RemoteStep::StoreAll { path, value: true, .. }] if path == "INBOX")
    );
}

#[test]
fn nothing_unread_anywhere_changes_nothing() {
    let (mut m, inbox) = inbox(0);
    m.run(Command::MarkMailboxRead { id: inbox }).unwrap();
    let again = m.run(Command::MarkMailboxRead { id: inbox }).unwrap();
    assert_eq!(again.receipt.data["changed"], json!([]));
    assert_eq!(again.receipt.data["uncached_affected"], false);
    assert!(again.receipt.undo_label.is_none());
    let sent = m.mailbox("Sent");
    m.run(Command::MarkMailboxRead { id: sent }).unwrap();
    let gone = app_core::IdGen::new(99).next_id();
    let err = m.run(Command::MarkMailboxRead { id: gone }).unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }));
}

#[test]
fn list_filters_narrow_the_visible_threads_and_are_saved() {
    let (mut m, inbox) = inbox(2);
    m.run(Command::OpenMailbox { id: inbox }).unwrap();
    m.run(Command::CheckThreads { ids: Vec::new() }).unwrap();
    assert_eq!(listing::visible_threads(m.engine.state()).len(), 3);
    let outcome = m
        .run(Command::SetListFilter {
            unread: Some(true),
            starred: None,
            attachments: None,
        })
        .unwrap();
    assert_eq!(outcome.receipt.data["threads"], 2);
    assert!(outcome.effects.iter().any(|e| matches!(
        e,
        Effect::PersistState { delta: app_core::PersistDelta::View(prefs) } if prefs.list_filter.unread
    )));
    assert_eq!(
        m.engine.state().view.checked.len(),
        2,
        "hidden threads are unchecked"
    );
    m.run(Command::SetListFilter {
        unread: None,
        starred: Some(true),
        attachments: None,
    })
    .unwrap();
    assert!(listing::visible_threads(m.engine.state()).is_empty());
    m.run(Command::SetListFilter {
        unread: Some(false),
        starred: Some(false),
        attachments: Some(false),
    })
    .unwrap();
    assert_eq!(listing::visible_threads(m.engine.state()).len(), 3);
}
