//! *Connect to a dail server* on the phone (SRV-7): the setup's other first choice, each stage of
//! `app_front::connect::ConnectFlow` as the setup sheet draws it (snapshots), a pasted or an
//! outside `dail://pair` link, and a linked phone's link status, refusals and devices.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use app_core::{
    AppState, Command, Device, DeviceRole, Event, IdGen, LinkState, LinkStatus, OfflineRejection,
    Origin, ServerEndpoint,
};
use app_mobile::event::Field;
use app_mobile::frame::{InputKind, SetupSheet};
use app_mobile::{Purpose, UiEvent};

use crate::support::{Phone, advance, requests_of, synced_state};

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";
const OTHER: &str = "0000402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

fn link(fp: &str) -> String {
    format!("dail://pair?v=1&h=home.example&p=7443&c=abcd2345&fp={fp}&n=Laptop")
}

fn sheet(phone: &mut Phone) -> SetupSheet {
    phone.frame().setup.expect("a sheet")
}

fn button(sheet: &SetupSheet, label: &str) -> u32 {
    sheet
        .actions
        .iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no {label} in {:?}", sheet.actions))
        .action
}

fn offered(fingerprint: &str) -> Event {
    Event::ServerOffered {
        host: "home.example".into(),
        port: 7443,
        fingerprint: fingerprint.into(),
    }
}

/// A phone without accounts, at the setup's *Connect to a dail server*.
fn at_connect() -> Phone {
    let mut phone = Phone::new(AppState::default(), 360, 800);
    let first = sheet(&mut phone);
    let server = first.presets.last().unwrap();
    assert_eq!(server.label, "Connect to a dail server");
    assert!(!server.selected);
    phone.tap(server.action);
    let first = sheet(&mut phone);
    assert!(first.presets.last().unwrap().selected);
    assert!(
        first.presets.iter().filter(|p| p.selected).count() == 1,
        "one choice"
    );
    phone.tap(button(&first, "Next"));
    phone
}

#[test]
fn the_first_run_offers_a_dail_server_and_pairs_with_it() {
    let mut phone = at_connect();
    let address = sheet(&mut phone);
    insta::assert_json_snapshot!("connect_address", &address);
    assert_eq!(address.fields[0].field, Field::Server);
    assert_eq!(address.fields[0].input, InputKind::Server);
    assert!(address.actions.is_empty(), "nothing to connect to yet");
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "home.example".to_owned(),
    });
    let address = sheet(&mut phone);
    phone.take();
    let cmds = phone.tap(button(&address, "Connect"));
    assert_eq!(
        cmds,
        [Command::ConnectServer {
            host: "home.example".into(),
            port: 7443
        }]
    );
    let probing = phone.frame();
    assert!(probing.wants_tick, "a failure counts after a while");
    let probing = probing.setup.unwrap();
    assert!(probing.busy);
    insta::assert_json_snapshot!("connect_probing", &probing);

    phone.ingest(offered(FP));
    let compare = sheet(&mut phone);
    insta::assert_json_snapshot!("connect_compare", &compare);
    assert_eq!(compare.fingerprints.len(), 1);
    assert_eq!(
        compare.fingerprints[0].lines,
        [
            "5d41 402a bc4b 2a76 b971 9d91 1017 c592",
            "ae2f 5e3c 8a1b 7d0f 4e6c 2a9b 8d7f 1e03"
        ]
    );
    assert_eq!(compare.fields[0].field, Field::Code);
    assert_eq!(compare.fields[0].input, InputKind::Code);
    assert!(compare.actions.is_empty(), "no code yet");
    phone.event(UiEvent::Text {
        field: Field::Code,
        text: "abcd2345".to_owned(),
    });
    let compare = sheet(&mut phone);
    let cmds = phone.tap(button(&compare, "Pair"));
    let [Command::TrustServer { fingerprint, code }] = cmds.as_slice() else {
        panic!("{cmds:?}")
    };
    assert_eq!((fingerprint.as_str(), code.expose()), (FP, "ABCD2345"));
    let pairing = sheet(&mut phone);
    assert!(pairing.busy);
    assert!(
        pairing.fields.is_empty() && !serde_json::to_string(&pairing).unwrap().contains("ABCD"),
        "the code left the sheet"
    );

    phone.ingest(Event::ServerPaired {
        fingerprint: FP.into(),
    });
    let paired = sheet(&mut phone);
    insta::assert_json_snapshot!("connect_paired", &paired);
    phone.tap(button(&paired, "Close"));
    let frame = phone.frame();
    assert!(frame.setup.is_none(), "the setup closes with the flow");
    // The link shows in the top bar from now on.
    assert!(
        frame
            .top_bar
            .subtitle
            .as_deref()
            .is_some_and(|s| s.starts_with("Connecting")),
        "{:?}",
        frame.top_bar.subtitle
    );
    // A linked phone is not offered the setup again.
    let mut again = Phone::new(phone.state().clone(), 360, 800);
    assert!(again.frame().setup.is_none());
}

#[test]
fn a_pasted_link_is_read_at_once_and_its_fingerprint_must_match() {
    let mut phone = at_connect();
    let before = sheet(&mut phone).fields[0].value.version;
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: link(FP),
    });
    let address = sheet(&mut phone);
    // The field is replaced: the host reloads it.
    assert_eq!(address.fields[0].value.text, "home.example:7443");
    assert!(address.fields[0].value.version > before);
    assert!(
        address
            .note
            .as_deref()
            .unwrap()
            .contains("From the pairing link")
    );
    phone.tap(button(&address, "Connect"));
    phone.ingest(offered(FP));
    let compare = sheet(&mut phone);
    assert!(compare.note.unwrap().contains("It matches the fingerprint"));
    assert_eq!(compare.fields[0].value.text, "ABCD2345", "the link's code");
    assert!(compare.actions.iter().any(|b| b.label == "Pair"));

    // Another fingerprint than the link's: never trusted.
    let mut phone = at_connect();
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: link(OTHER),
    });
    let address = sheet(&mut phone);
    phone.tap(button(&address, "Connect"));
    phone.ingest(offered(FP));
    let mismatch = sheet(&mut phone);
    insta::assert_json_snapshot!("connect_mismatch", &mismatch);
    assert_eq!(mismatch.fingerprints.len(), 2);
    assert!(mismatch.actions.iter().all(|b| b.label != "Pair"));
    // Back to the server; the link's code is gone.
    phone.tap(button(&mismatch, "Back"));
    let address = sheet(&mut phone);
    assert_eq!(address.fields[0].field, Field::Server);
    // Cancel removes the unpinned link: the profile is a plain one again.
    phone.event(UiEvent::Back);
    let cmds = requests_of(&phone.take());
    assert!(cmds.contains(&Command::DisconnectServer { confirm: true }));
    assert!(phone.state().link.is_none());
    // Back at the setup's first step.
    assert_eq!(sheet(&mut phone).title, "Add an account");
}

#[test]
fn a_bad_link_is_refused_without_repeating_it() {
    let mut phone = at_connect();
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "dail://pair?v=1&h=home.example&p=7443&c=EVIL&fp=00&n=x".to_owned(),
    });
    let address = sheet(&mut phone);
    assert!(address.fields[0].value.text.is_empty());
    let note = address.note.unwrap();
    assert!(note.contains("Not a usable pairing link"), "{note}");
    assert!(!note.contains("EVIL"));
    assert!(address.actions.is_empty());
}

#[test]
fn an_unreachable_server_fails_and_can_be_tried_again() {
    let mut phone = at_connect();
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "home.example:7443".to_owned(),
    });
    let address = sheet(&mut phone);
    phone.tap(button(&address, "Connect"));
    phone.ingest(Event::LinkFailed {
        error: "connection refused".into(),
    });
    advance(app_front::connect::GRACE_MS);
    phone.event(UiEvent::Tick);
    let failed = sheet(&mut phone);
    insta::assert_json_snapshot!("connect_failed", &failed);
    assert!(
        failed
            .error
            .as_deref()
            .unwrap()
            .contains("connection refused")
    );
    // A button of an older stage does nothing now.
    phone.take();
    assert!(phone.tap(address.back.action).is_empty());
    assert!(phone.frame().setup.is_some());
    phone.tap(button(&failed, "Try again"));
    let address = sheet(&mut phone);
    assert_eq!(address.fields[0].value.text, "home.example:7443", "kept");
    assert!(address.actions.iter().any(|b| b.label == "Connect"));
}

#[test]
fn a_link_from_outside_contacts_its_server_at_once() {
    let mut phone = Phone::new(AppState::default(), 360, 800);
    // The deep link or the scanner (SRV-QR2, SRV-QR3) send this event.
    phone.event(UiEvent::PairLink { uri: link(FP) });
    let requests = phone.take();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].purpose, Purpose::Connect);
    assert_eq!(
        requests[0].cmd,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443
        }
    );
    phone.ingest(offered(FP));
    let compare = sheet(&mut phone);
    assert_eq!(compare.title, "Connect to a dail server");
    assert_eq!(compare.fields[0].value.text, "ABCD2345");
    // Back from the code to the server, then out: nothing of the setup underneath.
    phone.event(UiEvent::Back);
    assert_eq!(sheet(&mut phone).fields[0].field, Field::Server);
    phone.event(UiEvent::Back);
    let frame = phone.frame();
    assert!(frame.setup.is_none() || frame.setup.unwrap().title == "Add an account");

    // A phone with accounts keeps them: the link is refused with a note.
    let mut phone = Phone::inbox(360, 800);
    phone.take();
    phone.event(UiEvent::PairLink { uri: link(FP) });
    assert!(phone.take().is_empty());
    let frame = phone.frame();
    assert!(frame.setup.is_none());
    assert!(
        frame
            .snackbar
            .unwrap()
            .text
            .contains("the pairing link was not used")
    );
}

/// The synced fixture as a mirror of a server, with one refused change and one paired device.
fn linked() -> (Phone, app_core::RequestId) {
    let mut state = synced_state();
    let mut ids = IdGen::new(5);
    let device: app_core::DeviceId = ids.next_id();
    state.devices.insert(
        device,
        Device {
            id: device,
            name: "Laptop".into(),
            role: DeviceRole::User,
            paired_at_ms: crate::support::NOW,
            last_seen_ms: None,
        },
    );
    let refused: app_core::RequestId = ids.next_id();
    state.link = Some(LinkState {
        status: LinkStatus::Online,
        server: Some(ServerEndpoint {
            host: "home.example".into(),
            port: 7443,
            fingerprint: Some(FP.into()),
            ..ServerEndpoint::default()
        }),
        rejected: vec![OfflineRejection {
            request_id: refused,
            seed: 1,
            origin: Origin::Gui,
            command: Command::RetryOffline {},
            code: "ERR004".into(),
            error: "ERR004: gone".into(),
            at_ms: crate::support::NOW,
        }],
        ..LinkState::default()
    });
    state.settings.stale_secs = 86_400;
    (Phone::new(state, 360, 800), refused)
}

#[test]
fn a_linked_phone_shows_its_link_refusals_and_devices() {
    let (mut phone, refused) = linked();
    let frame = phone.frame();
    assert!(frame.setup.is_none());
    assert!(
        frame
            .top_bar
            .subtitle
            .as_deref()
            .is_some_and(|s| s.starts_with("Online · ")),
        "{:?}",
        frame.top_bar.subtitle
    );
    // The refusal's banner: *Retry all* and *Dismiss*.
    let banner = frame.banner.unwrap();
    assert!(
        banner.text.contains("Refused by the server"),
        "{}",
        banner.text
    );
    assert_eq!(banner.action.as_ref().unwrap().label, "Retry all");
    assert_eq!(
        phone.tap(banner.action.unwrap().action),
        [Command::RetryOffline {}]
    );
    let (mut phone, _) = linked();
    let banner = phone.frame().banner.unwrap();
    assert_eq!(
        phone.tap(banner.dismiss.unwrap().action),
        [Command::DismissOfflineRejection { id: refused }]
    );

    // The Accounts settings: the server's details and actions, and the devices.
    let (mut phone, _) = linked();
    phone.dispatch(Command::OpenSettings {
        section: Some(app_core::SettingsSection::Accounts),
    });
    let page = phone.frame().settings.unwrap();
    let server = page
        .groups
        .iter()
        .find(|g| g.title == "dail server")
        .unwrap();
    assert!(server.rows.iter().any(|r| r.value == "home.example:7443"));
    assert!(
        server
            .rows
            .iter()
            .any(|r| r.value.contains("5d41 402a") && r.value.contains('\n'))
    );
    let disconnect = server
        .rows
        .iter()
        .flat_map(|r| &r.actions)
        .find(|b| b.label == "Disconnect…")
        .unwrap()
        .action;
    let devices = page.groups.iter().find(|g| g.title == "Devices").unwrap();
    assert_eq!(devices.rows[0].label, "Laptop");
    let revoke = devices.rows[0].actions[0].clone();
    assert_eq!(revoke.label, "Revoke…");
    // Both ask first, in the palette.
    let cmds = phone.tap(disconnect);
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, Command::DisconnectServer { .. })),
        "{cmds:?}"
    );
    assert!(phone.frame().palette.is_some());
    phone.event(UiEvent::Back);
    let page = phone.frame().settings.unwrap();
    let revoke = page
        .groups
        .iter()
        .find(|g| g.title == "Devices")
        .unwrap()
        .rows[0]
        .actions[0]
        .action;
    let cmds = phone.tap(revoke);
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, Command::RevokeDevice { .. })),
        "{cmds:?}"
    );
    assert!(phone.frame().palette.is_some());
}
