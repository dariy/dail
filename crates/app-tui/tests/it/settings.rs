//! The settings view (UI-12): lists of keys and values; `g ,` opens it, Tab moves between
//! sections, Enter opens the palette at the step that changes the row, Esc closes it.

use app_core::{Command, SettingsSection};
use ratatui::crossterm::event::KeyCode;

use crate::support::{Term, synced_state};

#[test]
fn the_settings_list_at_80_by_24() {
    let mut t = Term::new(synced_state(), 80, 24);
    t.typed("g,");
    assert_eq!(t.pump(), [Command::OpenSettings { section: None }]);
    insta::assert_snapshot!(t.screen());
}

#[test]
fn tab_enter_and_escape() {
    let mut t = Term::new(synced_state(), 80, 24);
    t.apply(Command::OpenSettings {
        section: Some(SettingsSection::General),
    });
    t.key(KeyCode::Tab);
    assert_eq!(
        t.pump(),
        [Command::OpenSettings {
            section: Some(SettingsSection::Keys)
        }]
    );
    // The first row of Keys is the preset: Enter asks for it in the palette.
    t.key(KeyCode::Enter);
    assert_eq!(t.pump(), [Command::OpenPalette {}]);
    assert!(t.screen().contains("Which keys?"), "{}", t.screen());
    // Esc leaves the palette's step, then the palette, then the settings.
    let mut sent = Vec::new();
    for _ in 0..3 {
        t.key(KeyCode::Esc);
        sent.extend(t.pump());
    }
    assert_eq!(sent.last(), Some(&Command::CloseSettings {}), "{sent:?}");
}
