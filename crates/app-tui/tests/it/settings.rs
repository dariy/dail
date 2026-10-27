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

/// Wave 7 F1/F3: the question and a waiting update in the status line, the update rows in the
/// general section.
#[test]
fn updates_in_the_status_line_and_the_settings() {
    use app_core::{Event, InstallKind, UpdateArtifact, UpdateHost, UpdateManifest};

    let mut t = Term::new(synced_state(), 120, 24);
    assert!(!t.screen().contains("Check for updates?"));
    t.ingest(Event::UpdateHostDetected {
        host: UpdateHost {
            version: "0.1.0".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            install: InstallKind::Binary,
        },
    });
    assert!(t.screen().contains("Check for updates?"), "{}", t.screen());

    t.apply(Command::SetUpdatePolicy {
        checks: Some(app_core::UpdateChecks::Off),
        interval_hours: None,
    });
    t.apply(Command::OpenSettings {
        section: Some(SettingsSection::General),
    });
    let screen = t.screen();
    assert!(screen.contains("dail 0.1.0"), "{screen}");
    assert!(screen.contains("update_checks"), "{screen}");
    // The first row of General is the version; the second, *Check now*.
    t.key(KeyCode::Char('j'));
    t.key(KeyCode::Enter);
    assert_eq!(t.pump(), [Command::CheckForUpdates {}]);

    t.ingest(Event::UpdateChecked {
        manifest: UpdateManifest {
            version: "9.0.0".to_owned(),
            published_ms: 0,
            notes_url: "https://github.com/dariy/dail/releases/tag/v9.0.0".to_owned(),
            artifacts: vec![UpdateArtifact {
                target: "x86_64-unknown-linux-gnu".to_owned(),
                kind: "exe".to_owned(),
                url: "https://github.com/dariy/dail/releases/download/v9.0.0/a".to_owned(),
                sha256: "0".repeat(64),
                size: 1,
            }],
        },
    });
    let screen = t.screen();
    assert!(screen.contains("dail 9.0.0 is available"), "{screen}");
    assert!(screen.contains("Update 9.0.0 (settings)"), "{screen}");
}
