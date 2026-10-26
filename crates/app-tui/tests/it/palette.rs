//! The palette overlay: opening, fuzzy choice, second steps, query results, and the parity walk
//! of every registry command through the TUI's own palette, by keys.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::panic)]

use app_core::{Command, CommandName};
use app_front::palette::{AskKind, offered};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use app_tui::Purpose;

use crate::support::{Term, inbox_of, rich_term, synced_state};

fn inbox_with_open_thread(width: u16, height: u16) -> (Term, Vec<app_core::ThreadId>) {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, width, height);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[0] });
    t.out.clear();
    (t, threads)
}

#[test]
fn ctrl_k_opens_it_and_enter_runs_the_best_match() {
    let (mut t, threads) = inbox_with_open_thread(80, 24);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    assert_eq!(t.pump(), [Command::OpenPalette {}]);
    insta::assert_snapshot!("medium_palette", t.screen());
    t.typed("archive");
    t.key(KeyCode::Enter);
    assert_eq!(
        t.take(),
        [
            Command::ArchiveThreads {
                ids: vec![threads[0]],
                messages: Vec::new()
            },
            Command::ClosePalette {}
        ]
    );
}

#[test]
fn a_second_step_escape_and_a_search() {
    let (mut t, _) = inbox_with_open_thread(40, 20);
    t.key(KeyCode::Char(':'));
    t.pump();
    t.typed("move to");
    t.key(KeyCode::Enter);
    assert!(t.take().is_empty(), "asks for the mailbox");
    insta::assert_snapshot!("phone_move_to", t.screen());
    t.key(KeyCode::Esc);
    assert!(t.take().is_empty(), "back to the commands");
    t.typed("from:carol");
    let screen = t.screen();
    assert!(screen.contains("Search mail for “from:carol”"), "{screen}");
    assert!(
        !screen.contains("Navigation"),
        "narrow rows show only the keys"
    );
    // Backspace edits the query; on an empty query it closes the palette.
    for _ in 0.."from:carol".len() {
        t.key(KeyCode::Backspace);
    }
    assert!(t.take().is_empty());
    t.key(KeyCode::Backspace);
    assert_eq!(t.take(), [Command::ClosePalette {}]);
}

#[test]
fn query_results_open_read_only_and_close_with_q() {
    let (mut t, _) = inbox_with_open_thread(80, 24);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    t.pump();
    t.typed("show settings");
    t.key(KeyCode::Enter);
    assert_eq!(
        t.pump(),
        [Command::GetSettings {}, Command::ClosePalette {}]
    );
    let screen = t.screen();
    assert!(screen.contains("untrusted data"), "{screen}");
    assert!(screen.contains("\"poll_secs\": 120"), "{screen}");
    t.key(KeyCode::Char('q'));
    assert!(!t.screen().contains("untrusted data"));
    assert!(!t.tui.quit_requested(), "q closed the result first");
}

#[test]
fn every_registry_command_is_reachable_through_the_tui_palette() {
    for &name in CommandName::ALL {
        let mut t = rich_term(120, 40);
        if !offered(name) || !(name.spec().available)(t.engine.state()) {
            continue;
        }
        t.apply(Command::OpenPalette {});
        t.typed(name.spec().title);
        t.key(KeyCode::Enter);
        for _ in 0..5 {
            let user = t.out.iter().any(|r| r.purpose == Purpose::User);
            if user || t.tui.notice().is_some() || t.tui.setup_open() {
                break;
            }
            let Some(step) = t.tui.palette_step() else {
                break;
            };
            if let AskKind::Text(_) = step.kind {
                let text = if matches!(name, CommandName::SetSetting | CommandName::SetSyncWindow) {
                    "300"
                } else {
                    "plans"
                };
                t.typed(text);
            }
            t.key(KeyCode::Enter);
        }
        let got = t.take();
        match name {
            CommandName::AddAccount | CommandName::SetPassword => {
                assert_eq!(got, [Command::ClosePalette {}], "{name:?}");
                assert!(t.tui.setup_open(), "{name:?} opens the account setup");
            }
            name => {
                let expected = match name {
                    CommandName::UpdateDraft => CommandName::OpenComposer,
                    name => name,
                };
                let built = got.first().map(Command::command_name);
                assert_eq!(built, Some(expected), "{name:?} built {got:?}");
            }
        }
    }
}
