//! Auto-advance after triage (UX-12).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::Command;
use app_front::advance::after;

use crate::support::{inbox_of, synced_state};

fn archive(ids: Vec<app_core::ThreadId>) -> Command {
    Command::ArchiveThreads {
        ids,
        messages: Vec::new(),
    }
}

#[test]
fn the_next_row_opens_else_the_previous_or_the_list() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let [first, second] = threads[..] else {
        panic!("the basic fixture has two threads")
    };
    state.view.mailbox = Some(inbox);
    state.view.thread = Some(first);
    let open = |id| Some(Command::OpenThread { id });
    assert_eq!(after(&state, &archive(vec![first])), open(second));
    // `previous` falls back to the next row at the top.
    state.settings.after_triage = 1;
    assert_eq!(after(&state, &archive(vec![first])), open(second));
    state.view.thread = Some(second);
    assert_eq!(after(&state, &archive(vec![second])), open(first));
    // `next` falls back to the previous row at the bottom.
    state.settings.after_triage = 0;
    assert_eq!(after(&state, &archive(vec![second])), open(first));
    // Rows triaged with it are skipped; with none left, back to the list.
    assert_eq!(
        after(&state, &archive(vec![first, second])),
        Some(Command::CloseThread {})
    );
    state.settings.after_triage = 2;
    assert_eq!(
        after(&state, &archive(vec![second])),
        Some(Command::CloseThread {})
    );
}

#[test]
fn only_triage_of_the_open_thread_advances() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    state.view.mailbox = Some(inbox);
    assert_eq!(
        after(&state, &archive(vec![threads[0]])),
        None,
        "nothing open"
    );
    state.view.thread = Some(threads[0]);
    assert_eq!(
        after(&state, &archive(vec![threads[1]])),
        None,
        "another thread"
    );
    let star = Command::Star {
        ids: vec![threads[0]],
        messages: Vec::new(),
    };
    assert_eq!(after(&state, &star), None, "not a triage that removes it");
}
