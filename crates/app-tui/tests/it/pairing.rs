//! The *Pair a device…* dialog (SRV-QR1): from the Accounts settings and from the palette.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AppState, Command, DeviceRole, ServeInfo, SettingsSection};
use app_front::pairing::Stage;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Term, synced_state};

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

fn serving() -> AppState {
    let mut state = synced_state();
    state.status.serve = Some(ServeInfo {
        addresses: vec!["100.64.0.7:7443".into()],
        advertise: Vec::new(),
        fingerprint: FP.into(),
    });
    state
}

fn opened_code(t: &Term) -> String {
    match t.tui.pairing().unwrap().stage() {
        Stage::Opened(opened) => opened.code.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_accounts_settings_pair_a_device_with_a_qr_code() {
    let mut t = Term::new(serving(), 100, 50);
    t.apply(Command::OpenSettings {
        section: Some(SettingsSection::Accounts),
    });
    let screen = t.screen();
    assert!(screen.contains("Pair a device…"), "{screen}");
    // The account, Add an account, then Devices (SRV-6): none paired yet, Pair a device….
    let screen = t.screen();
    assert!(screen.contains("No device is paired yet."), "{screen}");
    for _ in 0..4 {
        t.key(KeyCode::Down);
    }
    t.key(KeyCode::Enter);
    let screen = t.screen();
    assert!(screen.contains("Pair which kind of device?"), "{screen}");
    t.key(KeyCode::Tab); // an agent
    t.key(KeyCode::Enter); // no name yet: nothing
    assert!(t.take().is_empty());
    t.typed("Claude");
    t.key(KeyCode::Enter);
    t.pump();
    let code = opened_code(&t);
    let screen = t.screen();
    assert!(screen.contains(&code), "{screen}");
    assert!(screen.contains("Address: 100.64.0.7:7443"), "{screen}");
    assert!(screen.contains("5d41 402a"), "{screen}");
    assert!(screen.contains("Claude · An agent"), "{screen}");
    assert!(
        screen.contains('▀') && screen.contains('█'),
        "the QR code: {screen}"
    );
    // The code never reaches the state.
    let saved = serde_json::to_string(t.engine.state()).unwrap();
    assert!(!saved.contains(&code));
    t.key(KeyCode::Char('n'));
    assert_eq!(t.tui.pairing().unwrap().stage(), &Stage::Form);
    t.key(KeyCode::Esc);
    assert!(t.tui.pairing().is_none());
}

#[test]
fn the_palette_shows_its_code_in_the_dialog() {
    let mut t = Term::new(serving(), 100, 50);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    t.pump();
    t.typed("pair a device");
    t.key(KeyCode::Enter); // the command
    t.key(KeyCode::Enter); // a device of yours
    t.typed("Phone");
    t.key(KeyCode::Enter);
    let dialog = t.tui.pairing().unwrap();
    assert_eq!(dialog.role, DeviceRole::User);
    assert_eq!(dialog.stage(), &Stage::Waiting);
    t.pump();
    let code = opened_code(&t);
    assert!(t.screen().contains(&code));
}

#[test]
fn without_a_server_there_is_no_pair_entry() {
    let mut t = Term::new(synced_state(), 100, 40);
    t.apply(Command::OpenSettings {
        section: Some(SettingsSection::Accounts),
    });
    assert!(!t.screen().contains("Pair a device…"));
}

/// Opens a code on a `width`×`height` terminal through the palette; returns the screen and the
/// QR code's half-block rows.
fn opened_at(width: u16, height: u16) -> (String, Vec<String>) {
    let mut t = Term::new(serving(), width, height);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    t.pump();
    t.typed("pair a device");
    t.key(KeyCode::Enter);
    t.key(KeyCode::Enter);
    t.typed("Phone");
    t.key(KeyCode::Enter);
    t.pump();
    let Stage::Opened(opened) = t.tui.pairing().unwrap().stage() else {
        panic!("not opened");
    };
    let rows = app_front::qr::half_blocks(
        opened.qr.as_ref().unwrap(),
        app_front::qr::QUIET_ZONE,
        app_front::qr::Ink::Dark,
    );
    (t.screen(), rows)
}

#[test]
fn the_qr_code_is_drawn_whole_or_not_at_all() {
    // 80×24: no room. No partial code, a line asks for a larger terminal.
    let (screen, _) = opened_at(80, 24);
    assert!(screen.contains("Enlarge the terminal"), "{screen}");
    assert!(!screen.contains('▀') && !screen.contains('▄'), "{screen}");
    assert!(screen.contains("Pairing code"), "{screen}");
    // 100×50: every row of the code, in order.
    let (screen, rows) = opened_at(100, 50);
    assert!(!screen.contains("Enlarge the terminal"), "{screen}");
    let lines: Vec<&str> = screen.lines().collect();
    let inked: Vec<&String> = rows.iter().filter(|r| r.trim() != "").collect();
    let first = lines
        .iter()
        .position(|l| l.contains(inked[0].trim_end()))
        .unwrap_or_else(|| panic!("{screen}"));
    for (i, row) in inked.iter().enumerate() {
        assert!(
            lines[first + i].contains(row.trim_end()),
            "row {i}: {screen}"
        );
    }
}
