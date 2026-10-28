//! The command palette window: keys, second steps and query results. The registry parity walk
//! and the ranking are tested in `app-front` (`tests/it/palette.rs`).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{self, apply, gui, inbox_of, synced_state};
use app_core::{Command, MailboxRole};
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;

fn typed(h: &mut egui_kittest::Harness<'static, support::Gui>, text: &str) {
    h.event(egui::Event::Text(text.to_owned()));
    h.run();
}

#[test]
fn ctrl_k_opens_the_palette_and_enter_runs_the_best_match() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::OpenPalette {}]);
    h.run();
    typed(&mut h, "archive");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [
            Command::ArchiveThreads {
                ids: vec![threads[0]],
                messages: Vec::new()
            },
            // UX-12: the next conversation opens.
            Command::OpenThread { id: threads[1] },
            Command::ClosePalette {},
        ]
    );
}

#[test]
fn move_to_asks_for_the_mailbox_and_escape_steps_back() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let archive = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Archive))
        .unwrap()
        .id;
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    apply(&mut h, Command::OpenPalette {});
    h.state_mut().out.clear();
    typed(&mut h, "move to");
    h.key_press(Key::Enter);
    h.run();
    let got = h.state_mut().take();
    assert!(
        got.is_empty(),
        "a second step, nothing submitted yet: {got:?}"
    );
    h.get_by_label("MOVE TO…");
    // Escape goes back to the command list, a second Escape closes the palette.
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state_mut().take().is_empty());
    typed(&mut h, "move to");
    h.key_press(Key::Enter);
    h.run();
    typed(&mut h, "archive");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [
            Command::MoveThreads {
                ids: vec![threads[0]],
                messages: Vec::new(),
                to: archive
            },
            Command::OpenThread { id: threads[1] },
            Command::ClosePalette {},
        ]
    );
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state_mut().take(), [Command::ClosePalette {}]);
}

#[test]
fn typed_text_can_become_a_search() {
    let mut h = gui(synced_state());
    apply(&mut h, Command::OpenPalette {});
    h.state_mut().out.clear();
    typed(&mut h, "from:carol");
    h.get_by_label("Search mail for “from:carol”").click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [
            Command::SetSearch {
                query: "from:carol".into()
            },
            Command::ClosePalette {}
        ]
    );
}

#[test]
fn query_results_open_in_a_window() {
    let mut h = gui(synced_state());
    apply(&mut h, Command::OpenPalette {});
    typed(&mut h, "show settings");
    h.key_press(Key::Enter);
    h.run();
    let sent = h.state_mut().pump();
    assert_eq!(sent, [Command::GetSettings {}, Command::ClosePalette {}]);
    h.run();
    h.get_by_label("Show settings");
    // A view, not JSON (MISC-22); *Raw data* shows the JSON.
    h.get_by_label("Undo send ms");
    assert!(h.query_by_label_contains("\"undo_send_ms\"").is_none());
    h.get_by_label("Raw data").click();
    h.run();
    h.get_by_label_contains("\"undo_send_ms\": 10000");
}

#[test]
fn an_empty_query_shows_section_captions_and_down_reaches_every_row() {
    let mut h = gui(synced_state());
    apply(&mut h, Command::OpenPalette {});
    h.run();
    h.get_by_label("MAIL");
    h.get_by_label("MAILBOXES");
    // ↓ walks past every caption to the last row and stops there.
    for _ in 0..200 {
        h.key_press(egui::Key::ArrowDown);
    }
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(!h.state_mut().take().is_empty(), "the last row is chosen");
}

#[test]
fn a_palette_open_at_start_stays_open() {
    let mut state = synced_state();
    state.view.palette_open = true;
    let mut h = gui(state);
    h.run();
    h.run();
    assert_eq!(h.state_mut().take(), [], "nothing closes it");
    h.get_by_label("MAIL");
}
