//! The keymap from `CommandSpec.default_keys`: parsing, collisions and what each key does.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;

use crate::support::{Gui, apply, gui, inbox_of, synced_state};
use app_core::{AppState, Command, CommandName, MailboxRole, Pane};
use app_gui::keymap;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[test]
fn every_default_key_parses() {
    let total: usize = CommandName::ALL
        .iter()
        .map(|n| n.spec().default_keys.len())
        .sum();
    assert_eq!(
        keymap::bindings(&app_core::Settings::default()).len(),
        total
    );
    for keys in [
        "j", "k", "e", "#", "s", "u", "r", "a", "f", "c", "/", "z", "g i",
    ] {
        assert!(
            keymap::bindings(&app_core::Settings::default())
                .iter()
                .any(|b| b.text == keys),
            "{keys} is not bound"
        );
    }
}

#[test]
fn keys_are_shared_only_where_the_view_decides() {
    let mut by_keys: BTreeMap<String, Vec<CommandName>> = BTreeMap::new();
    for binding in keymap::bindings(&app_core::Settings::default()) {
        by_keys.entry(binding.text).or_default().push(binding.name);
    }
    let shared: BTreeMap<String, Vec<CommandName>> = by_keys
        .into_iter()
        .filter(|(_, names)| names.len() > 1)
        .collect();
    let expected: BTreeMap<String, Vec<CommandName>> = [
        (
            "Escape",
            vec![
                CommandName::ClosePalette,
                CommandName::CloseThread,
                CommandName::ClearSearch,
                CommandName::CloseComposer,
            ],
        ),
        ("u", vec![CommandName::MarkRead, CommandName::MarkUnread]),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    assert_eq!(shared, expected);
    // No sequence starts with a key that is bound on its own (it could never be typed).
    let bindings = keymap::bindings(&app_core::Settings::default());
    for long in bindings.iter().filter(|b| b.keys.len() > 1) {
        let shadow = bindings.iter().find(|b| b.keys == long.keys[..1]);
        assert!(shadow.is_none(), "{shadow:?} shadows {}", long.text);
    }
}

fn inbox_gui() -> (Harness<'static, Gui>, Vec<app_core::ThreadId>, AppState) {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state.clone());
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.state_mut().out.clear();
    (h, threads, state)
}

fn press(h: &mut Harness<'static, Gui>, key: Key) -> Vec<Command> {
    h.key_press(key);
    h.run();
    h.state_mut().pump()
}

#[test]
fn j_and_k_move_the_selection_and_enter_opens() {
    let (mut h, threads, _) = inbox_gui();
    assert_eq!(
        press(&mut h, Key::J),
        [Command::SelectThreads {
            ids: vec![threads[0]]
        }]
    );
    h.run();
    assert_eq!(
        press(&mut h, Key::J),
        [Command::SelectThreads {
            ids: vec![threads[1]]
        }]
    );
    h.run();
    assert!(press(&mut h, Key::J).is_empty(), "the last row stays");
    assert_eq!(
        press(&mut h, Key::K),
        [Command::SelectThreads {
            ids: vec![threads[0]]
        }]
    );
    h.run();
    assert_eq!(
        press(&mut h, Key::Enter),
        [Command::OpenThread { id: threads[0] }]
    );
    h.run();
    // With the reader open, j/k open the neighbours.
    assert_eq!(
        press(&mut h, Key::J),
        [Command::OpenThread { id: threads[1] }]
    );
}

#[test]
fn triage_keys_act_on_the_open_thread() {
    let (mut h, threads, _) = inbox_gui();
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    let ids = vec![threads[0]];
    // "Lunch?" is unread: u marks it read, then u marks it unread again.
    assert_eq!(
        press(&mut h, Key::U),
        [Command::MarkRead {
            ids: ids.clone(),
            messages: Vec::new()
        }]
    );
    h.run();
    assert_eq!(
        press(&mut h, Key::U),
        [Command::MarkUnread {
            ids: ids.clone(),
            messages: Vec::new()
        }]
    );
    h.run();
    assert_eq!(
        press(&mut h, Key::S),
        [Command::Star {
            ids: ids.clone(),
            messages: Vec::new()
        }]
    );
    h.run();
    let newest = h.state().engine.state().threads[&threads[0]].messages[0];
    assert_eq!(
        press(&mut h, Key::R),
        [Command::Reply {
            message: newest,
            body: None
        }]
    );
    h.run();
    let follow = h.state_mut().pump();
    assert!(
        matches!(follow[..], [Command::OpenComposer { .. }]),
        "{follow:?}"
    );
    h.run();
    // The composer has the focus: Escape closes it first, then the thread.
    assert_eq!(press(&mut h, Key::Escape), [Command::CloseComposer {}]);
    h.run();
    assert_eq!(press(&mut h, Key::Z), [Command::Undo {}], "undo the reply");
    h.run();
    h.event(egui::Event::Text("#".into()));
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::TrashThreads {
            ids: ids.clone(),
            messages: Vec::new()
        }]
    );
    h.run();
    assert_eq!(press(&mut h, Key::Z), [Command::Undo {}]);
    h.run();
    assert_eq!(
        press(&mut h, Key::E),
        [Command::ArchiveThreads {
            ids,
            messages: Vec::new()
        }]
    );
}

#[test]
fn sequences_tab_and_escape() {
    let (mut h, threads, state) = inbox_gui();
    let sent = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Sent))
        .unwrap()
        .id;
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Inbox))
        .unwrap()
        .id;
    h.key_press(Key::G);
    h.run();
    assert!(h.state_mut().pump().is_empty(), "g waits for the next key");
    assert_eq!(press(&mut h, Key::S), [Command::OpenMailbox { id: sent }]);
    h.run();
    h.key_press(Key::G);
    h.key_press(Key::I);
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::OpenMailbox { id: inbox }]);
    h.run();
    assert_eq!(
        press(&mut h, Key::Tab),
        [Command::FocusPane {
            pane: Pane::Sidebar
        }]
    );
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::FocusPane { pane: Pane::List }]
    );
    h.run();
    apply(&mut h, Command::OpenThread { id: threads[1] });
    apply(
        &mut h,
        Command::SetSearch {
            query: "bob".into(),
        },
    );
    apply(&mut h, Command::OpenThread { id: threads[1] });
    h.state_mut().out.clear();
    assert_eq!(press(&mut h, Key::Escape), [Command::CloseThread {}]);
    h.run();
    assert_eq!(press(&mut h, Key::Escape), [Command::ClearSearch {}]);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::B);
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::ToggleSidebar {}]);
}

#[test]
fn keys_that_need_an_argument_open_the_palette_at_that_step() {
    let (mut h, threads, _) = inbox_gui();
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    assert_eq!(press(&mut h, Key::V), [Command::OpenPalette {}]);
    h.run();
    h.get_by_label("MOVE TO…");
    assert_eq!(press(&mut h, Key::Escape), [], "back to the command list");
    h.run();
    assert_eq!(press(&mut h, Key::Escape), [Command::ClosePalette {}]);
    h.run();
    assert_eq!(press(&mut h, Key::Slash), [Command::OpenPalette {}]);
    h.run();
    h.event(egui::Event::Text("from:carol".into()));
    h.run();
    assert_eq!(
        press(&mut h, Key::Enter),
        [
            Command::SetSearch {
                query: "from:carol".into()
            },
            Command::ClosePalette {}
        ]
    );
}

#[test]
fn typing_in_a_text_field_does_not_trigger_single_keys() {
    let (mut h, _, state) = inbox_gui();
    // A text field: the composer's subject.
    let account = *state.accounts.keys().next().unwrap();
    apply(
        &mut h,
        Command::CreateDraft {
            account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
        },
    );
    let draft = *h.state().engine.state().drafts.keys().next().unwrap();
    apply(&mut h, Command::OpenComposer { id: draft });
    h.state_mut().out.clear();
    h.get_by_label("Subject").click();
    h.run();
    h.key_press(Key::E);
    h.event(egui::Event::Text("e".into()));
    h.run();
    assert!(h.state_mut().pump().is_empty(), "e typed into the subject");
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    h.run();
    // The typed text is written back first (the composer flushes before a key command).
    assert_eq!(
        h.state_mut().pump(),
        [
            Command::UpdateDraft {
                id: draft,
                to: None,
                cc: None,
                bcc: None,
                subject: Some("e".into()),
                body: None
            },
            Command::OpenPalette {}
        ],
        "Ctrl chords still work"
    );
}

#[test]
fn the_palette_shows_shortcut_hints() {
    let (mut h, _, _) = inbox_gui();
    apply(&mut h, Command::OpenPalette {});
    h.event(egui::Event::Text("go to inbox".into()));
    h.run();
    h.get_by_label("g i");
}
