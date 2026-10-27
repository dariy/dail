//! Wave 7 F1/F3: the first-run question, the top bar's update badge and the update panel of the
//! General settings.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{
    Command, Event, InstallKind, SettingsSection, UpdateArtifact, UpdateChecks, UpdateHost,
    UpdateManifest,
};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::support::{Gui, apply, gui, ingest, synced_state};

const TARGET: &str = "x86_64-unknown-linux-gnu";

fn with_host() -> Harness<'static, Gui> {
    let mut h = gui(synced_state());
    ingest(
        &mut h,
        Event::UpdateHostDetected {
            host: UpdateHost {
                version: "0.1.0".to_owned(),
                target: TARGET.to_owned(),
                install: InstallKind::Appimage,
            },
        },
    );
    h
}

fn found_release(h: &mut Harness<'static, Gui>) {
    apply(h, Command::CheckForUpdates {});
    ingest(
        h,
        Event::UpdateChecked {
            manifest: UpdateManifest {
                version: "9.0.0".to_owned(),
                published_ms: 0,
                notes_url: "https://github.com/dariy/dail/releases/tag/v9.0.0".to_owned(),
                artifacts: vec![UpdateArtifact {
                    target: TARGET.to_owned(),
                    kind: "appimage".to_owned(),
                    url: "https://github.com/dariy/dail/releases/download/v9.0.0/a".to_owned(),
                    sha256: "0".repeat(64),
                    size: 1,
                }],
            },
        },
    );
}

fn button<'a>(h: &'a Harness<'static, Gui>, label: &'a str) -> egui_kittest::Node<'a> {
    h.get(By::new().role(Role::Button).label(label))
}

#[test]
fn the_first_run_question_sets_the_policy() {
    let mut h = gui(synced_state());
    h.run();
    assert!(h.query_by_label(app_front::updates::QUESTION).is_none());
    let mut h = with_host();
    h.get_by_label(app_front::updates::QUESTION);
    button(&h, "Check for updates").click();
    h.run();
    assert_eq!(h.state_mut().pump(), [app_front::updates::answer(true)]);
    h.run();
    assert_eq!(
        h.state().engine.state().settings.updates.checks,
        UpdateChecks::On
    );
    assert!(h.query_by_label(app_front::updates::QUESTION).is_none());
}

#[test]
fn the_badge_opens_the_update_panel() {
    let mut h = with_host();
    apply(&mut h, app_front::updates::answer(false));
    found_release(&mut h);
    button(&h, "Update 9.0.0").click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::OpenSettings {
            section: Some(SettingsSection::General)
        }]
    );
}

#[test]
fn the_panel_downloads_and_asks_before_installing() {
    let mut h = with_host();
    apply(
        &mut h,
        Command::OpenSettings {
            section: Some(SettingsSection::General),
        },
    );
    h.get_by_label("dail 0.1.0");
    button(&h, "Check now").click();
    h.run();
    assert_eq!(h.state_mut().take(), [Command::CheckForUpdates {}]);

    found_release(&mut h);
    button(&h, "Download").click();
    h.run();
    assert_eq!(h.state_mut().take(), [Command::DownloadUpdate {}]);
    button(&h, "Skip this version").click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::SkipUpdateVersion {
            version: Some("9.0.0".to_owned())
        }]
    );

    apply(&mut h, Command::DownloadUpdate {});
    ingest(
        &mut h,
        Event::UpdateDownloaded {
            version: "9.0.0".to_owned(),
            path: "/tmp/dail-9.0.0.AppImage".to_owned(),
        },
    );
    button(&h, "Install…").click();
    h.run();
    // The confirmation step opens in the palette; nothing is installed yet.
    assert_eq!(h.state_mut().take(), [Command::OpenPalette {}]);
}

#[test]
fn the_interval_and_the_switch_send_the_policy() {
    let mut h = with_host();
    apply(
        &mut h,
        Command::OpenSettings {
            section: Some(SettingsSection::General),
        },
    );
    button(&h, "Weekly").click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::SetUpdatePolicy {
            checks: None,
            interval_hours: Some(168)
        }]
    );
    h.get(By::new().role(Role::CheckBox).label("Check automatically"))
        .click();
    h.run();
    assert_eq!(h.state_mut().take(), [app_front::updates::answer(true)]);
}
