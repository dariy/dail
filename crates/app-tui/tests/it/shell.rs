//! The shell: frames drawn from the state, quitting.

use app_core::Command;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Term, inbox_of, synced_state};

#[test]
fn a_frame_shows_the_open_mailbox_and_the_sync_state() {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let mut t = Term::new(state, 80, 24);
    t.apply(Command::OpenMailbox { id: inbox });
    insta::assert_snapshot!(t.screen());
}

#[test]
fn q_and_ctrl_c_quit() {
    let mut t = Term::new(synced_state(), 80, 24);
    t.key(KeyCode::Char('q'));
    assert!(t.tui.quit_requested());
    let mut t = Term::new(synced_state(), 80, 24);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('c'));
    assert!(t.tui.quit_requested());
    assert!(t.take().is_empty());
}

#[test]
fn without_accounts_it_shows_how_to_add_one_from_another_shell() {
    let mut t = Term::new(app_core::AppState::default(), 40, 20);
    t.tui = std::mem::take(&mut t.tui).with_setup_hint("dail account add --profile work");
    insta::assert_snapshot!(t.screen());
}
