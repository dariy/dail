//! The mailboxes and the thread list: flags, the cursor (the selection, as in the GUI), and
//! scrolling that keeps the cursor visible.

use app_core::Command;

use crate::support::{Term, inbox_of, synced_state, synced_with};

#[test]
fn rows_show_unread_starred_and_the_cursor() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 80, 24);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::Star {
        ids: vec![threads[1]],
        messages: Vec::new(),
    });
    t.apply(Command::SelectThreads {
        ids: vec![threads[1]],
    });
    insta::assert_snapshot!(t.screen());
}

#[test]
fn the_list_scrolls_to_keep_the_cursor_visible() {
    let state = synced_with(40);
    let (inbox, threads) = inbox_of(&state);
    assert_eq!(threads.len(), 42);
    let mut t = Term::new(state, 40, 14);
    t.apply(Command::OpenMailbox { id: inbox });
    let screen = t.screen();
    assert!(screen.contains("Lunch?"), "the newest first:\n{screen}");
    t.apply(Command::SelectThreads {
        ids: vec![threads[30]],
    });
    let screen = t.screen();
    let cursor = screen.lines().find(|l| l.starts_with('›')).unwrap_or("");
    assert!(
        cursor.contains("Note"),
        "the cursor's row is on screen:\n{screen}"
    );
    assert!(!screen.contains("Lunch?"), "scrolled down:\n{screen}");
    t.apply(Command::SelectThreads {
        ids: vec![threads[0]],
    });
    assert!(t.screen().contains("Lunch?"), "and back up");
}
