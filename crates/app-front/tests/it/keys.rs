//! The shared keymap: sequences with the time passed in, typing, and arguments from the view.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AppState, Command, CommandName, MailboxRole};
use app_front::keymap::{Action, Chord, Key, KeyState, Mods, Press, SEQUENCE_TIMEOUT_MS};

use crate::support::{inbox_of, synced_state};

fn opened_inbox() -> AppState {
    let mut state = synced_state();
    let (inbox, _) = inbox_of(&state);
    state.view.mailbox = Some(inbox);
    state
}

fn run(press: Press) -> Option<Command> {
    match press {
        Press::Done(Action::Run(cmd)) => Some(cmd),
        _ => None,
    }
}

#[test]
fn a_sequence_completes_within_the_timeout_only() {
    let state = opened_inbox();
    let sent = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Sent))
        .unwrap()
        .id;
    let g = Chord::plain(Key::Letter('g'));
    let s = Chord::plain(Key::Letter('s'));
    let mut keys = KeyState::default();
    assert!(matches!(keys.press(g, false, &state, 0), Press::Pending));
    assert!(keys.is_pending());
    assert_eq!(
        run(keys.press(s, false, &state, 1_000)),
        Some(Command::OpenMailbox { id: sent })
    );
    // Too late: `s` alone stars (nothing is targeted here, so it does not apply).
    assert!(matches!(
        keys.press(g, false, &state, 10_000),
        Press::Pending
    ));
    let late = keys.press(s, false, &state, 10_000 + SEQUENCE_TIMEOUT_MS + 1);
    assert!(!matches!(
        late,
        Press::Done(Action::Run(Command::OpenMailbox { .. }))
    ));
    assert!(!keys.is_pending());
}

#[test]
fn while_typing_only_escape_and_ctrl_chords_act() {
    let state = opened_inbox();
    let mut keys = KeyState::default();
    let j = Chord::plain(Key::Letter('j'));
    assert!(matches!(keys.press(j, true, &state, 0), Press::Unbound));
    assert!(run(keys.press(j, false, &state, 0)).is_some());
    let palette = Chord::Key(Mods::CTRL, Key::Letter('k'));
    assert_eq!(
        run(keys.press(palette, true, &state, 0)),
        Some(Command::OpenPalette {})
    );
    // Ctrl+Z stays the text field's undo.
    let undo = Chord::Key(Mods::CTRL, Key::Letter('z'));
    assert!(matches!(keys.press(undo, true, &state, 0), Press::Unbound));
}

#[test]
fn a_key_whose_command_needs_an_argument_asks() {
    let mut state = opened_inbox();
    let (_, threads) = inbox_of(&state);
    state.view.thread = Some(threads[0]);
    let mut keys = KeyState::default();
    let Press::Done(Action::Ask(step)) =
        keys.press(Chord::plain(Key::Letter('v')), false, &state, 0)
    else {
        panic!("v needs the target mailbox");
    };
    assert_eq!(step.prompt, "Move to…");
    assert_eq!(
        run(keys.press(Chord::Text('#'), false, &state, 0)),
        Some(Command::TrashThreads {
            ids: vec![threads[0]],
            messages: Vec::new()
        })
    );
}

#[test]
fn in_the_mailboxes_j_and_k_move_between_mailboxes_and_enter_goes_back() {
    let mut state = opened_inbox();
    state.view.focus = app_core::Pane::Sidebar;
    let order: Vec<_> = app_core::listing::mailboxes(&state, None)
        .iter()
        .map(|m| m.id)
        .collect();
    let mut keys = KeyState::default();
    assert_eq!(
        run(keys.press(Chord::plain(Key::Letter('j')), false, &state, 0)),
        Some(Command::OpenMailbox { id: order[1] })
    );
    assert_eq!(
        run(keys.press(Chord::plain(Key::Letter('k')), false, &state, 0)),
        None,
        "the inbox is the first mailbox"
    );
    assert_eq!(
        run(keys.press(Chord::plain(Key::Enter), false, &state, 0)),
        Some(Command::FocusPane {
            pane: app_core::Pane::List
        })
    );
}

#[test]
fn x_checks_the_cursor_and_star_a_checks_all() {
    let mut state = opened_inbox();
    let (_, threads) = inbox_of(&state);
    state.view.selected = vec![threads[0]];
    let mut keys = KeyState::default();
    let x = Chord::plain(Key::Letter('x'));
    assert_eq!(
        run(keys.press(x, false, &state, 0)),
        Some(Command::CheckThreads {
            ids: vec![threads[0]]
        })
    );
    state.view.checked = vec![threads[0]];
    assert_eq!(
        run(keys.press(x, false, &state, 10)),
        Some(Command::UncheckThreads {
            ids: vec![threads[0]]
        }),
        "a second x unchecks"
    );
    let star = Chord::typed('*').unwrap();
    assert!(matches!(
        keys.press(star, false, &state, 20),
        Press::Pending
    ));
    assert_eq!(
        run(keys.press(Chord::plain(Key::Letter('a')), false, &state, 30)),
        Some(Command::CheckThreads { ids: vec![] })
    );
    assert!(matches!(
        keys.press(star, false, &state, 40),
        Press::Pending
    ));
    assert_eq!(
        run(keys.press(Chord::plain(Key::Letter('n')), false, &state, 50)),
        Some(Command::UncheckThreads { ids: vec![] })
    );
    // Triage acts on the checked threads rather than the cursor.
    state.view.checked = vec![threads[1], threads[0]];
    assert_eq!(
        app_front::palette::targets(&state),
        [threads[1], threads[0]]
    );
}

#[test]
fn keys_follow_the_settings() {
    use app_front::keymap::keys_for;
    let mut state = opened_inbox();
    let (_, threads) = inbox_of(&state);
    state.view.selected = vec![threads[0]];
    // An override takes its keys away from other commands.
    state
        .settings
        .keys
        .insert("archive_threads".into(), vec!["s".into()]);
    assert_eq!(
        keys_for(&state.settings, CommandName::ArchiveThreads),
        ["s"]
    );
    assert!(keys_for(&state.settings, CommandName::Star).is_empty());
    let mut keys = KeyState::default();
    let s = Chord::plain(Key::Letter('s'));
    assert!(matches!(
        run(keys.press(s, false, &state, 0)),
        Some(Command::ArchiveThreads { .. })
    ));
    // The Vim preset: `u` undoes.
    state.settings.keys.clear();
    state.settings.key_preset = app_core::KeyPreset::Vim;
    let u = Chord::plain(Key::Letter('u'));
    assert_eq!(
        run(keys.press(u, false, &state, 10)),
        Some(Command::Undo {})
    );
    assert_eq!(
        keys_for(&state.settings, CommandName::TrashThreads),
        ["d d", "#"]
    );
}
