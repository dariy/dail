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
