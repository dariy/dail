//! `expand_messages` / `collapse_messages`: which messages of the open thread the reader shows
//! expanded.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, mail};
use app_core::{Command, CoreError, listing};
use serde_json::json;

/// INBOX: a read message and its unread reply (one thread), and a read single.
fn opened() -> (Mail, [app_core::MessageId; 2]) {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "Plans", true),
            mail(2, "b@x", Some("a@x"), "Re: Plans", true),
            mail(3, "c@x", None, "Lunch", true),
        ],
    );
    let (first, reply) = (m.message("INBOX", 1), m.message("INBOX", 2));
    m.run(Command::OpenThread {
        id: m.thread_of(first),
    })
    .unwrap();
    (m, [first, reply])
}

fn expanded(m: &Mail, message: app_core::MessageId) -> bool {
    let state = m.engine.state();
    let envelope = &state.messages[&message];
    listing::is_expanded(state, &state.threads[&envelope.thread], envelope)
}

#[test]
fn by_default_unread_messages_and_the_newest_are_expanded() {
    let (mut m, [first, reply]) = opened();
    assert!(!expanded(&m, first), "read and older");
    assert!(expanded(&m, reply), "newest");
    m.run(Command::MarkUnread {
        ids: vec![],
        messages: vec![first],
    })
    .unwrap();
    assert!(expanded(&m, first), "unread");
}

#[test]
fn messages_expand_and_collapse_one_by_one_or_all_at_once() {
    let (mut m, [first, reply]) = opened();
    let outcome = m.run(Command::ExpandMessages { ids: vec![first] }).unwrap();
    assert_eq!(
        outcome.receipt.data,
        json!({"messages": [first], "expanded": true})
    );
    assert!(outcome.effects.is_empty());
    assert!(expanded(&m, first));
    m.run(Command::CollapseMessages { ids: vec![] }).unwrap();
    assert!(
        !expanded(&m, first) && !expanded(&m, reply),
        "all collapsed"
    );
    m.run(Command::ExpandMessages { ids: vec![] }).unwrap();
    assert!(expanded(&m, first) && expanded(&m, reply));
    // View commands: not recorded in history.
    assert_eq!(
        app_core::CommandName::ExpandMessages.kind(),
        app_core::CommandKind::View
    );
    assert!(outcome.receipt.undo_label.is_none());
}

#[test]
fn another_thread_or_closing_starts_over() {
    let (mut m, [first, _]) = opened();
    m.run(Command::ExpandMessages { ids: vec![first] }).unwrap();
    // Opening the same thread again keeps it.
    m.run(Command::OpenThread {
        id: m.thread_of(first),
    })
    .unwrap();
    assert!(expanded(&m, first));
    let lunch = m.message("INBOX", 3);
    m.run(Command::OpenThread {
        id: m.thread_of(lunch),
    })
    .unwrap();
    assert!(m.engine.state().view.expanded.is_empty());
    m.run(Command::OpenThread {
        id: m.thread_of(first),
    })
    .unwrap();
    m.run(Command::ExpandMessages { ids: vec![first] }).unwrap();
    m.run(Command::CloseThread {}).unwrap();
    assert!(m.engine.state().view.expanded.is_empty());
}

#[test]
fn only_messages_of_the_open_thread() {
    let (mut m, [first, _]) = opened();
    let lunch = m.message("INBOX", 3);
    let err = m
        .run(Command::CollapseMessages { ids: vec![lunch] })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "ids"));
    m.run(Command::CloseThread {}).unwrap();
    let err = m
        .run(Command::ExpandMessages { ids: vec![first] })
        .unwrap_err();
    assert!(
        matches!(&err, CoreError::InvalidArgument { reason, .. } if reason.contains("no conversation")),
        "{err:?}"
    );
}
