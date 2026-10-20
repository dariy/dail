//! Mouse and touch: a tap selects (a second tap opens), a swipe scrolls, `m` turns capture off.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{Command, MailboxRole};
use app_tui::{External, Input};
use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::support::{Term, inbox_of, synced_state, synced_with};

fn at(kind: MouseEventKind, column: u16, row: u16) -> Input {
    Input::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn tap(column: u16, row: u16) -> Input {
    at(MouseEventKind::Down(MouseButton::Left), column, row)
}

#[test]
fn a_tap_selects_a_row_and_a_second_tap_opens_it() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 40, 20);
    t.apply(Command::OpenMailbox { id: inbox });
    t.out.clear();
    // Row 0 is the header; rows 1 and 2 are the threads.
    t.input(tap(10, 2));
    assert_eq!(
        t.pump(),
        [Command::SelectThreads {
            ids: vec![threads[1]]
        }]
    );
    t.input(tap(10, 2));
    assert_eq!(t.pump(), [Command::OpenThread { id: threads[1] }]);
}

#[test]
fn a_swipe_scrolls_the_list_without_moving_the_selection() {
    let state = synced_with(40);
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 40, 14);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::SelectThreads {
        ids: vec![threads[0]],
    });
    assert!(t.screen().contains("Lunch?"));
    for _ in 0..5 {
        t.input(at(MouseEventKind::ScrollDown, 10, 5));
    }
    let screen = t.screen();
    assert!(!screen.contains("Lunch?"), "{screen}");
    assert!(t.take().is_empty(), "scrolling dispatches nothing");
    for _ in 0..5 {
        t.input(at(MouseEventKind::ScrollUp, 10, 5));
    }
    assert!(t.screen().contains("Lunch?"));
}

#[test]
fn a_tap_on_a_mailbox_opens_it_and_m_turns_capture_off() {
    let state = synced_state();
    let sent = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Sent))
        .unwrap()
        .id;
    let mut t = Term::new(state, 120, 30);
    let screen = t.screen();
    let row = screen.lines().position(|l| l.contains("│  Sent")).unwrap();
    t.input(tap(5, u16::try_from(row).unwrap()));
    assert_eq!(t.pump(), [Command::OpenMailbox { id: sent }]);
    t.key(KeyCode::Char('m'));
    assert_eq!(t.tui.take_external(), Some(External::MouseCapture(false)));
    assert!(t.screen().contains("Mouse off"));
    t.key(KeyCode::Char('m'));
    assert_eq!(t.tui.take_external(), Some(External::MouseCapture(true)));
}
