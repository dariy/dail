//! `set_theme` and the theme settings (UI-11): names, modes, the legacy values 3 and 4,
//! validation and undo.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::ctx;
use app_core::{Command, CoreError, Effect, Engine, SettingKey, ThemeMode};

fn names(engine: &Engine) -> (u64, String, String) {
    let s = &engine.state().settings;
    (s.theme, s.theme_light.clone(), s.theme_dark.clone())
}

#[test]
fn set_theme_changes_mode_and_names_in_one_undo_step() {
    let mut engine = Engine::default();
    let before = names(&engine);
    assert_eq!(before, (0, "dail-light".into(), "dail-dark".into()));
    let outcome = engine
        .dispatch(
            Command::SetTheme {
                mode: Some(ThemeMode::Dark),
                light: None,
                dark: Some("dail-dark-hc".into()),
            },
            &ctx(1),
        )
        .unwrap();
    assert!(matches!(outcome.effects[..], [Effect::PersistState { .. }]));
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Theme"));
    let after = names(&engine);
    assert_eq!(after, (2, "dail-light".into(), "dail-dark-hc".into()));
    engine.dispatch(Command::Undo {}, &ctx(2)).unwrap();
    assert_eq!(names(&engine), before);
    engine.dispatch(Command::Redo {}, &ctx(3)).unwrap();
    assert_eq!(names(&engine), after);
}

#[test]
fn legacy_values_name_the_high_contrast_themes() {
    let mut engine = Engine::default();
    for (value, expected) in [
        (3_u64, (2, "dail-light", "dail-dark-hc")),
        (4, (1, "dail-light-hc", "dail-dark-hc")),
    ] {
        engine
            .dispatch(
                Command::SetSetting {
                    key: SettingKey::Theme,
                    value: value.into(),
                },
                &ctx(u128::from(value)),
            )
            .unwrap();
        let (mode, light, dark) = names(&engine);
        assert_eq!(
            (mode, light.as_str(), dark.as_str()),
            expected,
            "theme {value}"
        );
    }
    // One undo step each.
    engine.dispatch(Command::Undo {}, &ctx(10)).unwrap();
    assert_eq!(
        names(&engine),
        (2, "dail-light".into(), "dail-dark-hc".into())
    );
}

#[test]
fn empty_and_malformed_requests_are_rejected() {
    let mut engine = Engine::default();
    let before = engine.state().clone();
    for cmd in [
        Command::SetTheme {
            mode: None,
            light: None,
            dark: None,
        },
        Command::SetTheme {
            mode: None,
            light: Some(String::new()),
            dark: None,
        },
        Command::SetTheme {
            mode: None,
            light: None,
            dark: Some("Dail Dark".into()),
        },
        Command::SetTheme {
            mode: Some(ThemeMode::Light),
            light: Some("x".repeat(65)),
            dark: None,
        },
    ] {
        let err = engine.dispatch(cmd.clone(), &ctx(1)).unwrap_err();
        assert!(matches!(err, CoreError::InvalidArgument { .. }), "{cmd:?}");
    }
    assert_eq!(engine.state(), &before, "nothing changed");
}

#[test]
fn list_density_is_a_setting() {
    let mut engine = Engine::default();
    engine
        .dispatch(
            Command::SetSetting {
                key: SettingKey::ListDensity,
                value: 1.into(),
            },
            &ctx(1),
        )
        .unwrap();
    assert_eq!(engine.state().settings.list_density, 1);
    let err = engine
        .dispatch(
            Command::SetSetting {
                key: SettingKey::ListDensity,
                value: 2.into(),
            },
            &ctx(2),
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { .. }));
}
