//! The reader: bodies through the shared body tracking, headers, attachments, links with their
//! full targets, and scrolling.

use app_core::Command;
use ratatui::crossterm::event::KeyCode;

use crate::support::{Term, inbox_of, synced_state};

fn opened(width: u16, height: u16) -> Term {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, width, height);
    t.apply(Command::OpenMailbox { id: inbox });
    // Bob's "Q3 numbers".
    t.apply(Command::OpenThread { id: threads[1] });
    t
}

#[test]
fn the_open_thread_asks_for_its_bodies_once_and_shows_them() {
    let mut t = opened(40, 20);
    let asked: Vec<_> = t
        .out
        .iter()
        .filter(|r| {
            matches!(
                r.cmd,
                Command::GetThread {
                    include_body: true,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(asked.len(), 1, "{:?}", t.out);
    assert!(t.screen().contains("Loading…"));
    t.deliver_bodies(&[
        "Hi Alice,\nthe numbers: https://example.org/q3?view=full&year=2023\n\nBob",
    ]);
    assert!(t.out.is_empty(), "no second request for the same bodies");
    insta::assert_snapshot!(t.screen());
}

#[test]
fn space_and_b_page_and_the_arrows_scroll_a_lone_reader() {
    let mut t = opened(40, 14);
    let long: String = (1..=60).map(|n| format!("line {n}\n")).collect();
    t.deliver_bodies(&[&long]);
    assert!(!t.screen().contains("line 17"));
    // One page is the pane's 12 rows less one of overlap.
    t.key(KeyCode::Char(' '));
    let screen = t.screen();
    assert!(
        screen.contains("line 17") && !screen.contains("From: Bob"),
        "{screen}"
    );
    t.key(KeyCode::Char('b'));
    assert!(t.screen().contains("From: Bob"));
    t.key(KeyCode::Down);
    t.key(KeyCode::Down);
    assert!(
        !t.screen().contains("Q3 numbers"),
        "scrolled past the subject"
    );
    t.key(KeyCode::PageDown);
    t.key(KeyCode::PageDown);
    t.key(KeyCode::PageDown);
    t.key(KeyCode::PageDown);
    t.key(KeyCode::PageDown);
    assert!(t.screen().contains("line 60"), "the end, and no further");
    assert!(t.take().is_empty(), "scrolling dispatches nothing");
}

#[test]
fn collapsed_messages_are_one_line() {
    // One conversation: Bob's read "Q3 numbers", then Carol's unread "Lunch?".
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let (lunch, q3) = (
        state.threads[&threads[0]].messages[0],
        state.threads[&threads[1]].messages[0],
    );
    state.threads.shift_remove(&threads[1]);
    state.threads.get_mut(&threads[0]).unwrap().messages = vec![q3, lunch];
    state.messages.get_mut(&q3).unwrap().thread = threads[0];
    let mut t = Term::new(state, 70, 20);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[0] });
    let screen = t.screen();
    assert!(screen.contains("▸ Bob <bob@example.org> ·"), "{screen}");
    assert!(!screen.contains("From: Bob"), "{screen}");
    assert!(screen.contains("From: Carol"), "{screen}");
    t.apply(Command::ExpandMessages { ids: vec![] });
    let screen = t.screen();
    assert!(screen.contains("From: Bob"), "{screen}");
    assert!(!screen.contains('▸'), "{screen}");
}

#[test]
fn x_checks_a_thread_and_the_list_marks_it() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 70, 20);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::SelectThreads {
        ids: vec![threads[0]],
    });
    t.key(KeyCode::Char('x'));
    assert_eq!(
        t.pump(),
        [Command::CheckThreads {
            ids: vec![threads[0]]
        }]
    );
    let screen = t.screen();
    assert!(screen.contains('✓'), "{screen}");
}

#[test]
fn the_message_cursor_points_the_keys_at_one_message() {
    // One conversation: Bob's read "Q3 numbers", then Carol's unread "Lunch?".
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let (lunch, q3) = (
        state.threads[&threads[0]].messages[0],
        state.threads[&threads[1]].messages[0],
    );
    state.threads.shift_remove(&threads[1]);
    state.threads.get_mut(&threads[0]).unwrap().messages = vec![q3, lunch];
    state.messages.get_mut(&q3).unwrap().thread = threads[0];
    let mut t = Term::new(state, 90, 24);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[0] });
    t.apply(Command::FocusPane {
        pane: app_core::Pane::Reader,
    });
    // Without a cursor, `r` answers the newest message.
    t.key(KeyCode::Char('r'));
    assert_eq!(
        t.take(),
        [Command::Reply {
            message: lunch,
            body: None
        }]
    );
    t.key(KeyCode::Char('n'));
    let screen = t.screen();
    assert!(screen.contains("▶ Bob <bob@example.org>"), "{screen}");
    assert!(screen.contains("r Reply · a Reply all"), "{screen}");
    assert!(t.take().is_empty(), "moving the cursor dispatches nothing");
    t.key(KeyCode::Char('r'));
    assert_eq!(
        t.take(),
        [Command::Reply {
            message: q3,
            body: None
        }]
    );
    t.key(KeyCode::Char('s'));
    assert_eq!(
        t.take(),
        [Command::Star {
            ids: Vec::new(),
            messages: vec![q3]
        }]
    );
    // `n` stops at the last message.
    t.key(KeyCode::Char('n'));
    t.key(KeyCode::Char('n'));
    t.key(KeyCode::Char('#'));
    assert_eq!(
        t.take(),
        [Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![lunch]
        }]
    );
}

#[test]
fn enter_edits_and_hash_discards_a_draft_under_the_cursor() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[1]].messages[0];
    state.messages.get_mut(&message).unwrap().flags.draft = true;
    let mut t = Term::new(state, 70, 20);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[1] });
    t.key(KeyCode::Char('n'));
    let screen = t.screen();
    assert!(screen.contains("Enter Edit"), "{screen}");
    t.key(KeyCode::Enter);
    assert_eq!(t.take(), [Command::EditDraft { message }]);
    t.key(KeyCode::Char('#'));
    assert_eq!(
        t.take(),
        [Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![message]
        }]
    );
    // `p` before the first message drops the cursor: the keys act on the thread again.
    t.key(KeyCode::Char('p'));
    assert!(!t.screen().contains('▶'));
}

#[test]
fn the_cursor_yields_to_the_list_focus_and_to_checked_threads() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 90, 24);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[1] });
    t.apply(Command::FocusPane {
        pane: app_core::Pane::Reader,
    });
    t.key(KeyCode::Char('n'));
    assert!(t.screen().contains('▶'));
    // Checked threads take the keys, even in the focused reader.
    t.apply(Command::CheckThreads {
        ids: threads.clone(),
    });
    t.take();
    t.key(KeyCode::Char('s'));
    let cmds = t.take();
    assert!(
        matches!(&cmds[..], [Command::Star { ids, messages }] if !ids.is_empty() && messages.is_empty()),
        "{cmds:?}"
    );
    // With the list focused, the keys act on the list's targets, not the cursor's message.
    t.apply(Command::UncheckThreads {
        ids: threads.clone(),
    });
    t.apply(Command::FocusPane {
        pane: app_core::Pane::List,
    });
    t.take();
    t.key(KeyCode::Char('s'));
    let cmds = t.take();
    assert!(
        matches!(&cmds[..], [Command::Star { ids, messages }] if !ids.is_empty() && messages.is_empty()),
        "{cmds:?}"
    );
}
