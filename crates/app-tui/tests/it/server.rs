//! The dail server in the TUI (SRV-6): the link in the status line, the server's refusals as
//! problems with their actions and the devices in the Accounts settings, and *Connect to a dail
//! server instead* in the account setup.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AppState, Command, Device, DeviceRole, Event, IdGen, LinkState, LinkStatus, OfflineRejection,
    Origin, QueuedCommand, RequestId, ServerEndpoint, SettingsSection,
};
use app_tui::{Input, Purpose};
use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::support::{Term, fixed_now, inbox_of, synced_state};

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

fn last_line(screen: &str) -> &str {
    screen.lines().last().unwrap_or("")
}

/// alice's synced state mirrored from a pinned server, offline with three changes queued, one
/// refused archive and a paired phone.
fn mirror() -> (AppState, RequestId, app_core::DeviceId) {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    let mut ids = IdGen::new(7);
    let queued = (0..3)
        .map(|n| QueuedCommand {
            request_id: ids.next_id(),
            seed: n,
            origin: Origin::Tui,
            command: Command::MarkRead {
                ids: vec![threads[0]],
                messages: Vec::new(),
            },
            queued_at_ms: fixed_now(),
        })
        .collect();
    let refused: RequestId = ids.next_id();
    state.link = Some(LinkState {
        status: LinkStatus::Offline,
        revision: 42,
        queued,
        rejected: vec![OfflineRejection {
            request_id: refused,
            seed: 9,
            origin: Origin::Tui,
            command: Command::ArchiveThreads {
                ids: vec![threads[0]],
                messages: Vec::new(),
            },
            code: "ERR004".into(),
            error: "ERR004: thread 1 not found".into(),
            at_ms: fixed_now() - 3_600_000,
        }],
        server: Some(ServerEndpoint {
            host: "home.example".into(),
            port: 7443,
            fingerprint: Some(FP.into()),
            ..ServerEndpoint::default()
        }),
        error: Some("connect failed: connection refused".into()),
    });
    let phone: app_core::DeviceId = ids.next_id();
    state.devices.insert(
        phone,
        Device {
            id: phone,
            name: "Phone".into(),
            role: DeviceRole::User,
            paired_at_ms: fixed_now() - 86_400_000,
            last_seen_ms: Some(fixed_now() - 300_000),
        },
    );
    (state, refused, phone)
}

#[test]
fn the_status_line_shows_the_link_and_the_refusal_and_a_tap_opens_the_details() {
    let (state, _, _) = mirror();
    let mut t = Term::new(state, 120, 24);
    let screen = t.screen();
    let line = last_line(&screen).to_owned();
    assert!(
        line.starts_with("Refused by the server: Archive (settings › Accounts)"),
        "{line}"
    );
    assert!(line.contains("Offline · 3 queued"), "{line}");
    insta::assert_snapshot!(line);
    t.input(Input::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10,
        row: 23,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        t.pump(),
        [Command::OpenSettings {
            section: Some(SettingsSection::Accounts)
        }]
    );
    // `?` has the full text.
    let (state, _, _) = mirror();
    let mut t = Term::new(state, 120, 40);
    t.key(KeyCode::Char('?'));
    let help = t.screen();
    assert!(
        help.contains("Refused by the server: Archive: ERR004: thread 1 not found"),
        "{help}"
    );
}

#[test]
fn the_accounts_settings_show_the_server_its_refusals_and_the_devices() {
    let (state, refused, phone) = mirror();
    let mut t = Term::new(state, 100, 30);
    t.apply(Command::OpenSettings {
        section: Some(SettingsSection::Accounts),
    });
    let screen = t.screen();
    insta::assert_snapshot!(screen);
    for text in [
        "dail server",
        "home.example:7443",
        "Offline · 3 queued",
        "5d41 402a bc4b 2a76 b971 9d91 1017 c592",
        "Refused by the server: Archive",
        "ERR004: thread 1 not found · refused 07:00",
        "Retry all",
        "Dismiss",
        "Retry refused changes",
        "Disconnect…",
        "Devices",
        "A device of yours · paired Nov 14 · last seen 5m ago",
        "Revoke…",
        "Pair a device…",
    ] {
        assert!(screen.contains(text), "{text}:\n{screen}");
    }
    // The account, Add an account, dail server, Server, Connection, the certificate (2 lines),
    // Revision, Waiting to be sent, Last error, the refusal, Retry all, Dismiss.
    for _ in 0..12 {
        t.key(KeyCode::Down);
    }
    t.key(KeyCode::Enter);
    assert_eq!(t.take(), [Command::DismissOfflineRejection { id: refused }]);
    t.key(KeyCode::Up);
    t.key(KeyCode::Enter);
    assert_eq!(t.take(), [Command::RetryOffline {}]);
    // Retry refused changes, Disconnect…, Devices, Phone, Revoke… asks first.
    for _ in 0..6 {
        t.key(KeyCode::Down);
    }
    t.key(KeyCode::Enter);
    assert_eq!(t.pump(), [Command::OpenPalette {}]);
    let step = t.tui.palette_step().unwrap();
    assert!(step.prompt.starts_with("Revoke this device?"), "{step:?}");
    assert!(t.screen().contains("Revoke Phone"));
    t.key(KeyCode::Enter);
    assert_eq!(
        t.take().first(),
        Some(&Command::RevokeDevice {
            id: phone,
            confirm: true
        })
    );
}

#[test]
fn the_setup_connects_to_a_dail_server_instead() {
    let mut t = Term::new(AppState::default(), 100, 30);
    t.key(KeyCode::Enter);
    assert!(t.tui.setup_open());
    let screen = t.screen();
    assert!(
        screen.contains("Connect to a dail server instead"),
        "{screen}"
    );
    for _ in 0..3 {
        t.key(KeyCode::Down);
    }
    t.key(KeyCode::Enter);
    t.typed("home.example");
    assert!(t.screen().contains("Server: home.example▏"));
    t.key(KeyCode::Enter);
    let [request] = &t.out[..] else {
        panic!("{:?}", t.out)
    };
    assert_eq!(request.purpose, Purpose::Setup);
    assert_eq!(
        request.cmd,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443
        }
    );
    assert!(t.pump().is_empty(), "the setup's own request");
    assert!(t.screen().contains("Contacting home.example:7443…"));
    assert!(t.tui.wants_tick(t.engine.state()), "it ticks while waiting");
    t.ingest(Event::ServerOffered {
        host: "home.example".into(),
        port: 7443,
        fingerprint: FP.into(),
    });
    t.typed("abcd2345");
    let screen = t.screen();
    insta::assert_snapshot!(screen);
    assert!(screen.contains("5d41 402a bc4b 2a76"), "{screen}");
    assert!(screen.contains("Pairing code: ABCD2345▏"), "{screen}");
    t.key(KeyCode::Enter);
    let Some(Command::TrustServer { fingerprint, code }) = t.out.first().map(|r| r.cmd.clone())
    else {
        panic!("{:?}", t.out)
    };
    assert_eq!((fingerprint.as_str(), code.expose()), (FP, "ABCD2345"));
    t.pump();
    assert!(t.screen().contains("Pairing with home.example:7443…"));
    t.ingest(Event::ServerPaired {
        fingerprint: FP.into(),
    });
    let screen = t.screen();
    assert!(screen.contains("Paired with home.example:7443"), "{screen}");
    let saved = serde_json::to_string(t.engine.state()).unwrap();
    assert!(
        !saved.contains("ABCD2345"),
        "the code never reaches the state"
    );
    t.key(KeyCode::Enter);
    assert!(!t.tui.setup_open());
    // Linked, with no mail yet: the first-run screen waits for the server's.
    let screen = t.screen();
    assert!(screen.contains("Linked to home.example:7443"), "{screen}");
    assert!(last_line(&screen).contains("Connecting…"), "{screen}");
}

#[test]
fn a_pasted_link_with_another_fingerprint_is_refused() {
    let mut t = Term::new(AppState::default(), 100, 30);
    t.key(KeyCode::Enter);
    for _ in 0..3 {
        t.key(KeyCode::Down);
    }
    t.key(KeyCode::Enter);
    let other = "00".repeat(32);
    t.input(Input::Paste(format!(
        "dail://pair?v=1&h=home.example&p=7443&c=ABCD2345&fp={other}&n=Laptop"
    )));
    assert!(
        t.screen()
            .contains("From the pairing link: home.example:7443")
    );
    t.key(KeyCode::Enter);
    t.pump();
    t.ingest(Event::ServerOffered {
        host: "home.example".into(),
        port: 7443,
        fingerprint: FP.into(),
    });
    let screen = t.screen();
    assert!(
        screen.contains("home.example:7443 shows another fingerprint than the pairing link"),
        "{screen}"
    );
    t.key(KeyCode::Char('x'));
    t.key(KeyCode::Enter);
    assert!(t.take().is_empty(), "nothing is trusted");
    let screen = t.screen();
    assert!(
        screen.contains("Server: home.example:7443▏"),
        "back to the address:\n{screen}"
    );
    assert!(!screen.contains("ABCD2345"), "the link's code is dropped");
}

#[test]
fn a_profile_with_accounts_is_not_offered_the_server() {
    let mut t = Term::new(synced_state(), 100, 30);
    t.apply(Command::OpenSettings {
        section: Some(SettingsSection::Accounts),
    });
    let screen = t.screen();
    assert!(!screen.contains("Connect to a dail server"), "{screen}");
    assert!(!screen.contains("Devices"), "{screen}");
}
