//! The adaptive layout at the three test sizes, the sidebar's focus, and a too-small terminal.

use app_core::{Command, Pane};

use crate::support::{Term, inbox_of, synced_state};

/// The inbox open, its newest thread open in the reader.
fn reading(width: u16, height: u16) -> Term {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, width, height);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[0] });
    t
}

#[test]
fn a_phone_shows_one_pane() {
    let mut t = reading(40, 20);
    insta::assert_snapshot!("phone_reader", t.screen());
    t.apply(Command::CloseThread {});
    insta::assert_snapshot!("phone_list", t.screen());
    t.apply(Command::FocusPane {
        pane: Pane::Sidebar,
    });
    insta::assert_snapshot!("phone_mailboxes", t.screen());
}

#[test]
fn eighty_columns_show_the_list_and_the_reader() {
    let mut t = reading(80, 24);
    insta::assert_snapshot!("medium_reader", t.screen());
    t.apply(Command::FocusPane {
        pane: Pane::Sidebar,
    });
    let screen = t.screen();
    assert!(screen.contains("Mailboxes"), "{screen}");
    assert!(
        !screen.contains("Inbox ─"),
        "the mailboxes take the list's place"
    );
}

#[test]
fn wide_terminals_show_three_panes_unless_the_sidebar_is_collapsed() {
    let mut t = reading(120, 40);
    insta::assert_snapshot!("wide_reader", t.screen());
    t.apply(Command::ToggleSidebar {});
    assert!(!t.screen().contains("Mailboxes"));
}

#[test]
fn a_too_small_terminal_says_so_and_a_resize_dispatches_nothing() {
    let mut t = reading(39, 20);
    assert!(t.screen().contains("Terminal too small"));
    let mut t = reading(80, 11);
    assert!(t.screen().contains("Terminal too small"));
    t.input(app_tui::Input::Resize(120, 40));
    assert!(t.take().is_empty());
}
