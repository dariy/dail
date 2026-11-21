//! The *Pair a device…* dialog (SRV-QR1): from the Accounts settings, and the palette's code.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Gui, apply, gui, synced_state};
use app_core::{AppState, Command, ServeInfo, SettingsSection};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

fn linked() -> AppState {
    let mut state = synced_state();
    state.status.serve = Some(ServeInfo {
        addresses: vec!["100.64.0.7:7443".into()],
        advertise: vec!["home.tail.ts.net".into()],
        fingerprint: FP.into(),
    });
    state
}

fn button(h: &Harness<'static, Gui>, label: &str) {
    h.get(By::new().role(Role::Button).label(label)).click();
}

#[test]
fn the_accounts_settings_pair_a_device_and_show_its_code_and_qr_code() {
    let mut h = gui(linked());
    h.run();
    apply(
        &mut h,
        Command::OpenSettings {
            section: Some(SettingsSection::Accounts),
        },
    );
    button(&h, "Pair a device…");
    h.run();
    h.get_by_label("Pair which kind of device?");
    h.get_by_label("An agent").click();
    h.run();
    h.get_by_label("Its name?").click();
    h.run();
    h.event(egui::Event::Text("Claude".to_owned()));
    h.run();
    button(&h, "Pair");
    // The waiting spinner keeps repainting: step instead of run until the reply is in.
    h.step();
    h.step();
    h.get_by_label("Opening a pairing code…");
    // Background request: not a user command, but dispatched.
    assert!(h.state_mut().pump().is_empty());
    h.run();
    h.get_by_label("Pairing code");
    h.get_by_label("Address: home.tail.ts.net:7443");
    h.get_by_label_contains("5d41 402a");
    h.get(
        By::new()
            .role(Role::Image)
            .label_contains("Scan the QR code"),
    );
    let saved = serde_json::to_string(h.state().engine.state()).unwrap();
    let code = app_core::link::pairing_code(h.state().seed << 64);
    h.get_by_label(&code);
    assert!(!saved.contains(&code), "the code never reaches the state");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_label("Pairing code").is_none());
}

#[test]
fn without_a_server_the_settings_have_no_pair_button() {
    let mut h = gui(synced_state());
    h.run();
    apply(
        &mut h,
        Command::OpenSettings {
            section: Some(SettingsSection::Accounts),
        },
    );
    assert!(h.query_by_label("Pair a device…").is_none());
}
