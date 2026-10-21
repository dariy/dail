//! The settings page (UI-12): opened from the sidebar or with Ctrl+,; its controls send
//! commands; the keyboard tab lists every bound command with its keys.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{apply, gui, synced_state};
use app_core::{Command, SettingKey, SettingsSection, ThemeMode};
use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::kittest::{By, Queryable};

#[test]
fn ctrl_comma_opens_it_and_escape_closes_it() {
    let mut h = gui(synced_state());
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::OpenSettings { section: None }]
    );
    h.run();
    h.get_by_label("Appearance");
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::CloseSettings {}]);
}

#[test]
fn appearance_controls_send_commands() {
    let mut h = gui(synced_state());
    apply(&mut h, Command::OpenSettings { section: None });
    h.run();
    h.get(By::new().role(Role::Button).label("Dark")).click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::SetTheme {
            mode: Some(ThemeMode::Dark),
            light: None,
            dark: None
        }]
    );
    h.get(
        By::new()
            .role(Role::Button)
            .label("Dail Dark (high contrast)"),
    )
    .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::SetTheme {
            mode: None,
            light: None,
            dark: Some("dail-dark-hc".into())
        }]
    );
    h.get(By::new().role(Role::Button).label("Compact 40"))
        .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::SetSetting {
            key: SettingKey::ListDensity,
            value: 1.into()
        }]
    );
}

#[test]
fn the_keyboard_tab_lists_every_bound_command() {
    let mut h = gui(synced_state());
    apply(
        &mut h,
        Command::OpenSettings {
            section: Some(SettingsSection::Keys),
        },
    );
    h.run();
    let settings = h.state().engine.state().settings.clone();
    for &name in app_core::CommandName::ALL {
        let keys = app_front::keymap::keys_for(&settings, name);
        if keys.is_empty() {
            continue;
        }
        h.get(
            By::new()
                .role(Role::Button)
                .label(&format!("Change the keys of {}", name.spec().title)),
        );
    }
    assert!(h.get_all_by_label("Ctrl+K").next().is_some());
}
