//! `set_keybinding` / `set_key_preset`: key bindings kept in the settings.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::ctx;
use app_core::{Command, CoreError, Effect, Engine, KeyPreset};

#[test]
fn bindings_are_saved_validated_and_undone() {
    let mut engine = Engine::default();
    let outcome = engine
        .dispatch(
            Command::SetKeybinding {
                command: "archive_threads".into(),
                keys: Some(vec!["y".into(), "  g   a ".into()]),
            },
            &ctx(1),
        )
        .unwrap();
    assert!(matches!(outcome.effects[..], [Effect::PersistState { .. }]));
    assert_eq!(
        engine.state().settings.keys["archive_threads"],
        ["y", "g a"],
        "spaces normalized"
    );
    engine
        .dispatch(
            Command::SetKeyPreset {
                preset: KeyPreset::Vim,
            },
            &ctx(2),
        )
        .unwrap();
    assert_eq!(engine.state().settings.key_preset, KeyPreset::Vim);
    engine.dispatch(Command::Undo {}, &ctx(3)).unwrap();
    assert_eq!(engine.state().settings.key_preset, KeyPreset::Gmail);
    engine.dispatch(Command::Undo {}, &ctx(4)).unwrap();
    assert!(engine.state().settings.keys.is_empty());

    for (command, keys) in [
        ("no_such_command", Some(vec!["y".to_owned()])),
        ("archive_threads", Some(vec![String::new()])),
        ("archive_threads", Some(vec!["a b c d".to_owned()])),
        ("archive_threads", Some(vec!["a\u{1b}".to_owned()])),
    ] {
        let err = engine
            .dispatch(
                Command::SetKeybinding {
                    command: command.into(),
                    keys,
                },
                &ctx(5),
            )
            .unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidArgument { .. }),
            "{command}"
        );
    }
}
