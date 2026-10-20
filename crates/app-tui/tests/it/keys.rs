//! The registry's keys in a plain terminal.

use app_core::CommandName;
use app_front::keymap::parse_sequence;
use app_tui::keys::{delivers, hint};

#[test]
fn every_command_with_default_keys_has_one_a_plain_terminal_delivers() {
    let gmail = app_core::Settings::default();
    let vim = app_core::Settings {
        key_preset: app_core::KeyPreset::Vim,
        ..app_core::Settings::default()
    };
    for &name in CommandName::ALL {
        let keys = name.spec().default_keys;
        if keys.is_empty() {
            continue;
        }
        let deliverable = keys.iter().any(|text| {
            parse_sequence(text).is_some_and(|chords| chords.into_iter().all(delivers))
        });
        assert!(deliverable, "{name:?}: none of {keys:?} reaches the TUI");
        assert!(hint(&gmail, name).is_some(), "{name:?}");
        assert!(hint(&vim, name).is_some(), "{name:?} with the Vim keys");
    }
    assert_eq!(
        hint(&gmail, CommandName::SendDraft).as_deref(),
        Some("Ctrl+S")
    );
    assert_eq!(hint(&gmail, CommandName::Redo).as_deref(), Some("Shift+Z"));
    assert_eq!(
        hint(&gmail, CommandName::OpenPalette).as_deref(),
        Some("Ctrl+K")
    );
    assert_eq!(
        hint(&gmail, CommandName::ToggleSidebar).as_deref(),
        Some("[")
    );
    assert_eq!(
        hint(&gmail, CommandName::SyncNow).as_deref(),
        Some("Ctrl+R")
    );
}

mod keymap {
    use app_core::{Command, MailboxRole, Pane};
    use app_tui::Input;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    use crate::support::{Term, inbox_of, synced_state};

    fn inbox() -> (Term, Vec<app_core::ThreadId>) {
        let state = synced_state();
        let (inbox, threads) = inbox_of(&state);
        let mut t = Term::new(state, 80, 24);
        t.apply(Command::OpenMailbox { id: inbox });
        t.out.clear();
        (t, threads)
    }

    #[test]
    fn j_k_and_enter_move_and_open_like_the_gui() {
        let (mut t, threads) = inbox();
        t.key(KeyCode::Char('j'));
        assert_eq!(
            t.pump(),
            [Command::SelectThreads {
                ids: vec![threads[0]]
            }]
        );
        t.key(KeyCode::Char('j'));
        t.pump();
        t.key(KeyCode::Char('k'));
        t.pump();
        t.key(KeyCode::Enter);
        assert_eq!(t.pump(), [Command::OpenThread { id: threads[0] }]);
        t.key(KeyCode::Char('e'));
        assert_eq!(
            t.pump(),
            [Command::ArchiveThreads {
                ids: vec![threads[0]],
                messages: Vec::new()
            }]
        );
        t.key(KeyCode::Char('z'));
        assert_eq!(t.pump(), [Command::Undo {}]);
        t.chord(KeyModifiers::SHIFT, KeyCode::Char('Z'));
        assert_eq!(t.pump(), [Command::Redo {}]);
    }

    #[test]
    fn symbols_sequences_and_keys_that_ask() {
        let (mut t, threads) = inbox();
        let sent = t
            .engine
            .state()
            .mailboxes
            .values()
            .find(|m| m.role == Some(MailboxRole::Sent))
            .unwrap()
            .id;
        t.apply(Command::OpenThread { id: threads[1] });
        t.chord(KeyModifiers::SHIFT, KeyCode::Char('#'));
        assert_eq!(
            t.take(),
            [Command::TrashThreads {
                ids: vec![threads[1]],
                messages: Vec::new()
            }]
        );
        t.key(KeyCode::Char('g'));
        assert!(t.take().is_empty(), "g waits");
        t.key(KeyCode::Char('s'));
        assert_eq!(t.take(), [Command::OpenMailbox { id: sent }]);
        t.key(KeyCode::Char('v'));
        assert_eq!(
            t.take(),
            [Command::OpenPalette {}],
            "v asks for the mailbox"
        );
        t.key(KeyCode::Char(':'));
        assert_eq!(t.take(), [Command::OpenPalette {}]);
        t.key(KeyCode::Tab);
        assert_eq!(t.take(), [Command::FocusPane { pane: Pane::Reader }]);
    }

    #[test]
    fn compose_opens_the_new_draft_in_the_composer() {
        let (mut t, _) = inbox();
        t.key(KeyCode::Char('c'));
        let sent = t.pump();
        assert!(matches!(sent[0], Command::CreateDraft { .. }), "{sent:?}");
        assert!(matches!(sent[1], Command::OpenComposer { .. }), "{sent:?}");
    }

    #[test]
    fn q_and_backspace_stand_in_for_escape_and_q_quits_on_top() {
        let (mut t, threads) = inbox();
        t.apply(Command::OpenThread { id: threads[0] });
        t.key(KeyCode::Backspace);
        assert_eq!(t.pump(), [Command::CloseThread {}]);
        t.key(KeyCode::Backspace);
        assert!(t.pump().is_empty());
        assert!(!t.tui.quit_requested(), "Backspace never quits");
        t.apply(Command::FocusPane {
            pane: Pane::Sidebar,
        });
        t.key(KeyCode::Char('q'));
        assert_eq!(t.pump(), [Command::FocusPane { pane: Pane::List }]);
        t.key(KeyCode::Char('q'));
        assert!(t.tui.quit_requested());
    }

    #[test]
    fn a_paste_outside_a_text_field_is_dropped() {
        let (mut t, _) = inbox();
        t.input(Input::Paste("e#zz".into()));
        assert!(t.take().is_empty());
        assert!(!t.tui.quit_requested());
    }
}
