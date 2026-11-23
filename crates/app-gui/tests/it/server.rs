//! The dail server in the GUI (SRV-6): the link chip in the top bar and its details, the
//! server's refusals in the problems popover, the server and *Devices* in the Accounts settings,
//! and *Connect to a dail server instead* in the account wizard.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Gui, apply, fixed_now, gui, inbox_of, ingest, synced_state};
use app_core::{
    AppState, Command, Device, DeviceId, DeviceRole, Event, IdGen, LinkState, LinkStatus,
    OfflineRejection, Origin, QueuedCommand, RequestId, ServerEndpoint, SettingsSection,
};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

fn button(h: &Harness<'static, Gui>, label: &str) {
    h.get(By::new().role(Role::Button).label(label)).click();
}

/// alice's synced state mirrored from a pinned server, offline with three changes queued, one
/// refused archive and a paired phone.
fn mirror() -> (AppState, RequestId, DeviceId) {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    let mut ids = IdGen::new(7);
    let queued = (0..3)
        .map(|n| QueuedCommand {
            request_id: ids.next_id(),
            seed: n,
            origin: Origin::Gui,
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
            origin: Origin::Gui,
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
        error: None,
    });
    let phone: DeviceId = ids.next_id();
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
fn the_link_chip_shows_the_state_and_its_details_disconnect_after_a_confirmation() {
    let (state, _, _) = mirror();
    let mut h = gui(state);
    h.run();
    button(&h, "Offline · 3 queued");
    h.run();
    h.get_by_label("DAIL SERVER");
    h.get_by_label("home.example:7443");
    h.get_by_label("5d41 402a bc4b 2a76 b971 9d91 1017 c592");
    h.get_by_label("42");
    button(&h, "Disconnect…");
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::OpenPalette {}]);
    h.run();
    // The palette's confirmation (its prompt is a caption, in capitals).
    h.get_by_label_contains("DISCONNECT FROM THE DAIL SERVER?");
    h.get_by_label_contains("Disconnect from home.example:7443")
        .click();
    h.run();
    assert_eq!(
        h.state_mut().take()[0],
        Command::DisconnectServer { confirm: true }
    );
}

#[test]
fn a_refusal_is_listed_with_retry_all_and_dismiss() {
    let (state, refused, _) = mirror();
    let mut h = gui(state);
    h.run();
    h.get(
        By::new()
            .role(Role::Button)
            .label_contains("Refused by the server: Archive"),
    )
    .click();
    h.run();
    h.get_by_label("ERR004: thread 1 not found · refused 07:00");
    button(&h, "Dismiss");
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::DismissOfflineRejection { id: refused }]
    );
    h.run();
    assert!(
        h.query_by_label_contains("Refused by the server").is_none(),
        "dismissed"
    );
}

#[test]
fn the_accounts_settings_show_the_server_and_its_devices() {
    let (state, _, phone) = mirror();
    let mut h = gui(state);
    h.run();
    apply(
        &mut h,
        Command::OpenSettings {
            section: Some(SettingsSection::Accounts),
        },
    );
    h.get_by_label("DAIL SERVER");
    h.get_by_label("DEVICES");
    h.get_by_label("Phone");
    h.get_by_label("A device of yours · paired Nov 14 · last seen 5m ago");
    // The link's actions, and *Pair a device…* moved into Devices.
    button(&h, "Retry refused changes");
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::RetryOffline {}]);
    h.run();
    h.get(By::new().role(Role::Button).label("Pair a device…"));
    button(&h, "Revoke…");
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::OpenPalette {}]);
    h.run();
    h.get_by_label_contains("REVOKE THIS DEVICE?");
    h.get_by_label_contains("Revoke Phone").click();
    h.run();
    assert_eq!(
        h.state_mut().take()[0],
        Command::RevokeDevice {
            id: phone,
            confirm: true
        }
    );
}

#[test]
fn the_wizard_connects_to_a_dail_server_instead() {
    let mut h = gui(AppState::default());
    h.run();
    button(&h, "Connect to a dail server instead");
    h.run();
    h.get_by_label("Server").click();
    h.run();
    h.event(egui::Event::Text("home.example".to_owned()));
    h.run();
    button(&h, "Connect");
    // The waiting spinner keeps repainting: step instead of run until the reply is in.
    h.step();
    h.step();
    h.get_by_label("Contacting home.example:7443…");
    assert!(h.state_mut().pump().is_empty(), "the wizard's own request");
    h.step();
    ingest(
        &mut h,
        Event::ServerOffered {
            host: "home.example".into(),
            port: 7443,
            fingerprint: FP.into(),
        },
    );
    h.get_by_label("5d41 402a bc4b 2a76 b971 9d91 1017 c592");
    h.get_by_label("Pairing code").click();
    h.run();
    h.event(egui::Event::Text("abcd2345".to_owned()));
    h.run();
    button(&h, "Pair");
    h.step();
    h.step();
    h.get_by_label("Pairing with home.example:7443…");
    assert!(h.state_mut().pump().is_empty());
    h.step();
    ingest(
        &mut h,
        Event::ServerPaired {
            fingerprint: FP.into(),
        },
    );
    h.get_by_label_contains("Paired with home.example:7443");
    let saved = serde_json::to_string(h.state().engine.state()).unwrap();
    assert!(
        !saved.contains("ABCD2345"),
        "the code never reaches the state"
    );
    let server = h
        .state()
        .engine
        .state()
        .link
        .clone()
        .unwrap()
        .server
        .unwrap();
    assert_eq!(server.fingerprint.as_deref(), Some(FP));
    button(&h, "Close");
    h.run();
    assert!(h.query_by_label("Connect to a dail server").is_none());
    // Linked: the wizard does not open again for the empty mirror.
    h.run();
    assert!(h.query_by_label("Add an account").is_none());
    // The link chip.
    h.get(By::new().role(Role::Button).label("Connecting…"));
}

#[test]
fn a_profile_with_accounts_is_not_offered_the_server() {
    let mut h = gui(synced_state());
    h.run();
    apply(
        &mut h,
        Command::OpenSettings {
            section: Some(SettingsSection::Accounts),
        },
    );
    assert!(h.query_by_label("Connect to a dail server").is_none());
    assert!(h.query_by_label("DEVICES").is_none());
    button(&h, "Add account");
    h.run();
    assert!(
        h.query_by_label("Connect to a dail server instead")
            .is_none()
    );
}
