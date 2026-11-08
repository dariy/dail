//! The theme and the shared preferences follow the synced settings document (ADR 0015, SET-7):
//! a local change stamps the document, a pull applies a newer field to the local settings.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, T0};
use app_core::{
    Command, Effect, Event, MailboxRole, PersistDelta, ProviderPreset, SettingKey, SettingsCopy,
    Stamped, SyncedSettings, SyncedTheme, ThemeMode,
};
use serde_json::json;

fn mail() -> Mail {
    Mail::new(
        ProviderPreset::Custom,
        &[
            ("INBOX", Some(MailboxRole::Inbox)),
            ("dail", None),
            ("dail/Settings", None),
        ],
    )
}

fn set(key: SettingKey, value: u64) -> Command {
    Command::SetSetting {
        key,
        value: json!(value),
    }
}

/// Answers the pull in flight with `doc`.
fn pull(m: &mut Mail, doc: &SyncedSettings) -> app_core::Outcome {
    if !m.engine.state().synced.pulling {
        m.run(Command::PullSettings { account: None }).unwrap();
    }
    let account = m.account;
    m.feed(Event::SettingsRead {
        account,
        copy: Some(SettingsCopy {
            uid_validity: 9,
            uid: 4,
        }),
        document: Some(String::from_utf8(doc.to_json().unwrap()).unwrap()),
    })
}

fn theme_doc(updated_ms: i64, mode: u64, dark: &str) -> SyncedSettings {
    let mut doc = SyncedSettings::new();
    doc.updated_ms = updated_ms;
    doc.theme = Some(Stamped {
        updated_ms,
        value: SyncedTheme {
            mode,
            light: "dail-light".into(),
            dark: dark.into(),
        },
    });
    doc
}

fn persists_synced(effects: &[Effect]) -> bool {
    effects.iter().any(|e| {
        matches!(
            e,
            Effect::PersistState {
                delta: PersistDelta::Synced(_)
            }
        )
    })
}

fn persists_settings(effects: &[Effect]) -> bool {
    effects.iter().any(|e| {
        matches!(
            e,
            Effect::PersistState {
                delta: PersistDelta::Settings(_)
            }
        )
    })
}

#[test]
fn a_shared_setting_is_stamped_and_dirty() {
    let mut m = mail();
    let out = m.run(set(SettingKey::ListDensity, 1)).unwrap();
    let synced = &m.engine.state().synced;
    assert!(synced.dirty);
    let pref = &synced.settings.prefs["list_density"];
    assert_eq!(pref.value, json!(1));
    assert_eq!(pref.updated_ms, T0 + 3_000);
    assert_eq!(synced.settings.updated_ms, T0 + 3_000);
    assert!(persists_synced(&out.effects) && persists_settings(&out.effects));
}

#[test]
fn a_second_change_in_the_same_millisecond_still_wins() {
    let mut m = mail();
    let now = app_core::Ctx {
        now_ms: T0,
        id_seed: 0,
        origin: app_core::Origin::Gui,
    };
    m.engine
        .dispatch(set(SettingKey::Language, 2), &now)
        .unwrap();
    m.engine
        .dispatch(set(SettingKey::Language, 1), &now)
        .unwrap();
    let pref = &m.engine.state().synced.settings.prefs["language"];
    assert_eq!((pref.updated_ms, pref.value.clone()), (T0 + 1, json!(1)));
}

#[test]
fn set_theme_stamps_the_theme() {
    let mut m = mail();
    m.run(Command::SetTheme {
        mode: Some(ThemeMode::Dark),
        light: None,
        dark: Some("dail-dark-hc".into()),
    })
    .unwrap();
    let theme = m.engine.state().synced.settings.theme.clone().unwrap();
    assert_eq!(theme.value.mode, 2);
    assert_eq!(theme.value.dark, "dail-dark-hc");
    assert!(m.engine.state().synced.dirty);
    // The legacy `set_setting theme` is the theme too.
    m.run(set(SettingKey::Theme, 1)).unwrap();
    let theme = m.engine.state().synced.settings.theme.clone().unwrap();
    assert_eq!(theme.value.mode, 1);
}

#[test]
fn a_local_only_setting_never_enters_the_document() {
    let mut m = mail();
    for (key, value) in [
        (SettingKey::TextScale, 150),
        (SettingKey::PollSecs, 600),
        (SettingKey::TuiColors, 1),
        (SettingKey::BackgroundSync, 1),
        (SettingKey::HistoryCap, 50),
        (SettingKey::Notifications, 0),
    ] {
        let out = m.run(set(key, value)).unwrap();
        assert!(!persists_synced(&out.effects), "{key:?}");
    }
    m.run(Command::SetKeyPreset {
        preset: app_core::KeyPreset::Vim,
    })
    .unwrap();
    let synced = &m.engine.state().synced;
    assert_eq!(synced.settings, SyncedSettings::default());
    assert!(!synced.dirty);
}

#[test]
fn undo_and_redo_restamp_the_shared_value() {
    let mut m = mail();
    m.run(set(SettingKey::MarkReadAfterMs, 5_000)).unwrap();
    m.run(Command::Undo {}).unwrap();
    let pref = m.engine.state().synced.settings.prefs["mark_read_after_ms"].clone();
    assert_eq!(pref.value, json!(2_000));
    assert_eq!(pref.updated_ms, T0 + 4_000);
    assert_eq!(m.engine.state().settings.mark_read_after_ms, 2_000);
    m.run(Command::Redo {}).unwrap();
    let pref = m.engine.state().synced.settings.prefs["mark_read_after_ms"].clone();
    assert_eq!((pref.value, pref.updated_ms), (json!(5_000), T0 + 5_000));
    assert_eq!(m.engine.state().settings.mark_read_after_ms, 5_000);
}

#[test]
fn a_newer_remote_theme_changes_the_local_theme() {
    let mut m = mail();
    let out = pull(&mut m, &theme_doc(T0 + 1_000_000, 2, "dail-dark-hc"));
    let s = &m.engine.state().settings;
    assert_eq!((s.theme, s.theme_dark.as_str()), (2, "dail-dark-hc"));
    assert!(persists_settings(&out.effects));
    assert!(
        !m.engine.state().synced.dirty,
        "a pulled change is not a local one"
    );
}

#[test]
fn an_older_remote_theme_is_ignored() {
    let mut m = mail();
    m.run(Command::SetTheme {
        mode: Some(ThemeMode::Light),
        light: Some("dail-light-hc".into()),
        dark: None,
    })
    .unwrap();
    let out = pull(&mut m, &theme_doc(T0 - 1_000, 2, "dail-dark-hc"));
    let s = &m.engine.state().settings;
    assert_eq!((s.theme, s.theme_light.as_str()), (1, "dail-light-hc"));
    assert_eq!(s.theme_dark, "dail-dark");
    assert!(!persists_settings(&out.effects));
}

#[test]
fn a_pull_applies_shared_prefs_only_and_leaves_unset_fields_alone() {
    let mut m = mail();
    m.run(set(SettingKey::UndoSendMs, 7_000)).unwrap();
    let mut doc = SyncedSettings::new();
    for (key, value) in [
        ("list_density", json!(2)),
        ("poll_secs", json!(30)),
        ("text_scale", json!(200)),
        ("after_triage", json!(99)),
        ("language", json!("fr")),
    ] {
        doc.prefs.insert(
            key.into(),
            Stamped {
                updated_ms: T0 + 1_000_000,
                value,
            },
        );
    }
    let before = m.engine.state().settings.clone();
    pull(&mut m, &doc);
    let s = &m.engine.state().settings;
    assert_eq!(s.list_density, 2, "a shared preference follows");
    assert_eq!(s.poll_secs, before.poll_secs, "a local-only key is ignored");
    assert_eq!(s.text_scale, before.text_scale);
    assert_eq!(s.after_triage, before.after_triage, "out of range");
    assert_eq!(s.language, before.language, "not a number");
    assert_eq!(s.undo_send_ms, 7_000, "absent from the server copy");
    assert_eq!(s.theme, before.theme, "no theme in the document");
}
