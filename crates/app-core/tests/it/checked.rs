//! `check_threads` / `uncheck_threads`: the checked threads that bulk triage acts on.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, mail};
use app_core::{Command, CoreError, ThreadId};

/// INBOX with three single-message threads, open.
fn inbox() -> (Mail, Vec<ThreadId>) {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "One", true),
            mail(2, "b@x", None, "Two", true),
            mail(3, "c@x", None, "Three", true),
        ],
    );
    let inbox = m.mailbox("INBOX");
    m.run(Command::OpenMailbox { id: inbox }).unwrap();
    let threads = (1..=3)
        .map(|uid| m.thread_of(m.message("INBOX", uid)))
        .collect();
    (m, threads)
}

fn checked(m: &Mail) -> Vec<ThreadId> {
    m.engine.state().view.checked.clone()
}

#[test]
fn threads_are_checked_one_by_one_or_all_that_the_list_shows() {
    let (mut m, t) = inbox();
    let outcome = m
        .run(Command::CheckThreads {
            ids: vec![t[2], t[0], t[2]],
        })
        .unwrap();
    assert_eq!(outcome.receipt.data["checked"], 2);
    assert_eq!(checked(&m), [t[2], t[0]], "in checking order, once each");
    m.run(Command::UncheckThreads { ids: vec![t[2]] }).unwrap();
    assert_eq!(checked(&m), [t[0]]);
    m.run(Command::CheckThreads { ids: vec![] }).unwrap();
    assert_eq!(checked(&m).len(), 3, "all of the open mailbox");
    m.run(Command::UncheckThreads { ids: vec![] }).unwrap();
    assert!(checked(&m).is_empty());
}

#[test]
fn another_list_unchecks_and_unknown_threads_are_refused() {
    let (mut m, t) = inbox();
    m.run(Command::CheckThreads { ids: vec![t[0]] }).unwrap();
    m.run(Command::SetSearch {
        query: "one".into(),
    })
    .unwrap();
    assert!(checked(&m).is_empty(), "a search shows another list");
    m.run(Command::CheckThreads { ids: vec![] }).unwrap();
    assert_eq!(checked(&m), [t[0]], "all of the search results");
    m.run(Command::ClearSearch {}).unwrap();
    assert!(checked(&m).is_empty());
    let unknown = ThreadId::from_uuid(uuid::Uuid::from_u128(99));
    assert!(matches!(
        m.run(Command::CheckThreads { ids: vec![unknown] }),
        Err(CoreError::NotFound { .. })
    ));
}
