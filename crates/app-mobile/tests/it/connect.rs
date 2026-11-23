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
use app_mobile::event::ScanFailure;
use app_mobile::frame::{InputKind, SetupSheet};
use app_mobile::{External, Purpose, UiEvent};

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
    let labels: Vec<&str> = address.actions.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Scan QR code"], "nothing to connect to yet");
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
    assert!(address.actions.iter().all(|b| b.label != "Connect"));
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

/// A link from outside the app, then the user's *Connect* on the server stage it stops at.
fn link_and_connect(phone: &mut Phone, fp: &str) -> Vec<Command> {
    phone.event(UiEvent::PairLink { uri: link(fp) });
    let address = sheet(phone);
    phone.tap(button(&address, "Connect"))
}

#[test]
fn a_link_from_outside_waits_for_the_user_before_contacting_its_server() {
    let mut phone = Phone::new(AppState::default(), 360, 800);
    // The deep link or the scanner (SRV-QR2, SRV-QR3) send this event: any page or app may.
    phone.event(UiEvent::PairLink { uri: link(FP) });
    assert!(
        phone.take().is_empty(),
        "nothing is sent before the user acts"
    );
    for _ in 0..3 {
        phone.event(UiEvent::Tick);
        phone.frame();
    }
    assert!(phone.take().is_empty(), "nor later");
    let address = sheet(&mut phone);
    assert_eq!(address.title, "Connect to a dail server");
    assert_eq!(address.fields[0].field, Field::Server);
    assert_eq!(address.fields[0].value.text, "home.example:7443");
    let warning = address.warning.as_deref().expect("a link's flow warns");
    assert!(warning.contains("home.example:7443"), "{warning}");
    assert!(warning.contains("your own dail server"), "{warning}");

    // The user's *Connect*: only now connect_server, then Compare as before.
    phone.take();
    phone.event(UiEvent::Tap {
        action: button(&address, "Connect"),
    });
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

#[test]
fn a_link_from_outside_warns_on_compare_and_never_pairs_by_itself() {
    let mut phone = Phone::new(AppState::default(), 360, 800);
    link_and_connect(&mut phone, FP);
    phone.ingest(offered(FP));
    let compare = sheet(&mut phone);
    let warning = compare.warning.as_deref().expect("a link's flow warns");
    assert!(warning.contains("home.example:7443"), "{warning}");
    assert!(warning.contains("your own dail server"), "{warning}");
    assert!(
        compare.actions.iter().any(|b| b.label == "Pair"),
        "the link's code is filled in"
    );
    // Nothing goes to the server before the user's *Pair*: not trust_server, from any frame.
    phone.take();
    for _ in 0..3 {
        phone.event(UiEvent::Tick);
        phone.frame();
    }
    assert!(
        phone
            .take()
            .iter()
            .all(|r| !matches!(r.cmd, Command::TrustServer { .. }))
    );
    let compare = sheet(&mut phone);
    let cmds = phone.tap(button(&compare, "Pair"));
    assert!(matches!(cmds.as_slice(), [Command::TrustServer { .. }]));

    // Typed by hand, the flow does not warn.
    let mut phone = at_connect();
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "home.example".to_owned(),
    });
    let address = sheet(&mut phone);
    assert_eq!(address.warning, None);
    phone.tap(button(&address, "Connect"));
    phone.ingest(offered(FP));
    assert_eq!(sheet(&mut phone).warning, None);

    // A link's server edited by hand is no longer the link's: no warning.
    let mut phone = Phone::new(AppState::default(), 360, 800);
    phone.event(UiEvent::PairLink { uri: link(FP) });
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "home.example:7444".to_owned(),
    });
    assert_eq!(sheet(&mut phone).warning, None);
}

#[test]
fn a_link_does_not_replace_a_pairing_under_way() {
    // At the Compare stage of a flow typed by hand: the link is dropped with a snackbar.
    let mut phone = at_connect();
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "home.example".to_owned(),
    });
    let address = sheet(&mut phone);
    phone.tap(button(&address, "Connect"));
    phone.ingest(offered(FP));
    phone.take();
    phone.event(UiEvent::PairLink {
        uri: "dail://pair?v=1&h=evil.example&p=7443&c=abcd2345&fp=00&n=X".to_owned(),
    });
    assert!(phone.take().is_empty(), "no command for the dropped link");
    let frame = phone.frame();
    assert!(
        frame
            .snackbar
            .unwrap()
            .text
            .contains("A pairing is already under way")
    );
    let compare = frame.setup.unwrap();
    assert_eq!(compare.fields[0].field, Field::Code, "still at Compare");
    assert_eq!(compare.warning, None);
    assert!(!serde_json::to_string(&compare).unwrap().contains("evil"));

    // A second link while the first one waits for the user's *Connect*: dropped too.
    let mut phone = Phone::new(AppState::default(), 360, 800);
    phone.event(UiEvent::PairLink { uri: link(FP) });
    phone.event(UiEvent::PairLink { uri: link(OTHER) });
    assert!(phone.take().is_empty());
    assert!(
        phone
            .frame()
            .snackbar
            .unwrap()
            .text
            .contains("A pairing is already under way")
    );
    let address = sheet(&mut phone);
    phone.tap(button(&address, "Connect"));
    phone.ingest(offered(FP));
    let compare = sheet(&mut phone);
    assert!(compare.warning.is_some());
    assert!(
        compare.error.is_none(),
        "the first link's fingerprint still applies"
    );

    // An empty server field gives way to the link, which waits there too.
    let mut phone = at_connect();
    phone.take();
    phone.event(UiEvent::PairLink { uri: link(FP) });
    assert!(phone.take().is_empty());
    let address = sheet(&mut phone);
    assert_eq!(address.fields[0].value.text, "home.example:7443");
    assert!(address.warning.is_some());
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

/// Taps *Scan QR code* on the server stage: the host is asked for its scanner, and nothing else.
fn scan(phone: &mut Phone) {
    let address = sheet(phone);
    phone.externals();
    let cmds = phone.tap(button(&address, "Scan QR code"));
    assert!(cmds.is_empty(), "a scan sends nothing");
    assert_eq!(phone.externals(), [External::ScanQr]);
}

fn snackbar(phone: &mut Phone) -> Option<String> {
    phone.frame().snackbar.map(|s| s.text)
}

#[test]
fn the_scanner_is_offered_only_where_a_phone_can_pair() {
    let mut phone = at_connect();
    // Before the main button, which stays last.
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "home.example".to_owned(),
    });
    let labels: Vec<String> = sheet(&mut phone)
        .actions
        .into_iter()
        .map(|b| b.label)
        .collect();
    assert_eq!(labels, ["Scan QR code", "Connect"]);
    scan(&mut phone);
    // Only the server stage has it.
    phone.event(UiEvent::Scanned { text: link(FP) });
    let address = sheet(&mut phone);
    phone.tap(button(&address, "Connect"));
    phone.ingest(offered(FP));
    let compare = sheet(&mut phone);
    assert!(compare.actions.iter().all(|b| b.label != "Scan QR code"));

    // A phone with accounts or a server never shows it (nor the flow).
    let mut phone = Phone::inbox(360, 800);
    let frame = serde_json::to_string(&phone.frame()).unwrap();
    assert!(!frame.contains("Scan QR code"));
    // A flow whose phone got an account meanwhile: no scanner, and a stale tap does nothing.
    let mut phone = at_connect();
    let address = sheet(&mut phone);
    phone.dispatch(Command::AddAccount {
        preset: app_core::ProviderPreset::Gmail,
        email: "bob@gmail.com".to_owned(),
        display_name: None,
        password: None,
        imap: None,
        smtp: None,
    });
    phone.externals();
    phone.tap(button(&address, "Scan QR code"));
    assert!(phone.externals().is_empty());
    if let Some(sheet) = phone.frame().setup {
        assert!(sheet.actions.iter().all(|b| b.label != "Scan QR code"));
    }
}

#[test]
fn a_scanned_pairing_code_waits_for_the_user_as_a_link_does() {
    let mut phone = at_connect();
    // Typed text at the server stage gives way to the user's own scan.
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "other.example".to_owned(),
    });
    scan(&mut phone);
    phone.take();
    phone.event(UiEvent::Scanned { text: link(FP) });
    assert!(phone.take().is_empty(), "nothing is sent before Connect");
    for _ in 0..3 {
        phone.event(UiEvent::Tick);
        phone.frame();
    }
    assert!(phone.take().is_empty(), "nor later");
    let address = sheet(&mut phone);
    assert_eq!(address.fields[0].value.text, "home.example:7443");
    let warning = address.warning.as_deref().expect("a scanned code warns");
    assert!(warning.contains("home.example:7443"), "{warning}");
    assert!(warning.contains("your own dail server"), "{warning}");
    assert_eq!(snackbar(&mut phone), None);
    let cmds = phone.tap(button(&address, "Connect"));
    assert_eq!(
        cmds,
        [Command::ConnectServer {
            host: "home.example".into(),
            port: 7443
        }]
    );
    phone.ingest(offered(FP));
    let compare = sheet(&mut phone);
    assert!(compare.warning.is_some());
    assert!(compare.note.unwrap().contains("It matches the fingerprint"));
    assert_eq!(compare.fields[0].value.text, "ABCD2345");
    assert!(
        phone
            .take()
            .iter()
            .all(|r| !matches!(r.cmd, Command::TrustServer { .. })),
        "only the user's Pair trusts it"
    );
}

#[test]
fn scanned_text_that_is_no_pairing_code_is_never_followed() {
    for text in [
        "https://evil.example/pair",
        "evil.example:7443",
        "dail://pair?v=1&h=evil.example&p=7443&c=EVIL&fp=00&n=x",
        "",
    ] {
        let mut phone = at_connect();
        scan(&mut phone);
        phone.take();
        phone.event(UiEvent::Scanned {
            text: text.to_owned(),
        });
        assert!(phone.take().is_empty(), "{text}");
        assert!(phone.externals().is_empty(), "{text}");
        assert_eq!(
            snackbar(&mut phone).as_deref(),
            Some("This QR code is not a dail pairing code."),
            "{text}"
        );
        let address = sheet(&mut phone);
        assert!(address.fields[0].value.text.is_empty(), "{text}");
        assert_eq!(address.warning, None, "{text}");
        assert!(!serde_json::to_string(&address).unwrap().contains("evil"));
    }
}

#[test]
fn a_scan_that_was_not_asked_for_or_did_not_happen_changes_nothing() {
    // No scan asked for: a `scanned` event (from anywhere) is ignored.
    let mut phone = at_connect();
    phone.event(UiEvent::Scanned { text: link(FP) });
    assert!(sheet(&mut phone).fields[0].value.text.is_empty());
    assert_eq!(snackbar(&mut phone), None);
    // Cancelled: the flow stays as it was, quietly, and a late result is ignored.
    phone.event(UiEvent::Text {
        field: Field::Server,
        text: "home.example".to_owned(),
    });
    scan(&mut phone);
    phone.event(UiEvent::ScanCancelled);
    phone.event(UiEvent::Scanned { text: link(OTHER) });
    let address = sheet(&mut phone);
    assert_eq!(address.fields[0].value.text, "home.example");
    assert_eq!(address.warning, None);
    assert_eq!(snackbar(&mut phone), None);
    assert!(phone.take().iter().all(|r| r.purpose != Purpose::Connect));
}

#[test]
fn a_scanner_that_cannot_run_says_why() {
    for (reason, says) in [
        (ScanFailure::Denied, "system settings"),
        (ScanFailure::NoCamera, "no camera"),
        (ScanFailure::Failed, "did not start"),
    ] {
        let mut phone = at_connect();
        scan(&mut phone);
        phone.event(UiEvent::ScanFailed { reason });
        let text = snackbar(&mut phone).expect("a snackbar");
        assert!(text.contains(says), "{reason:?}: {text}");
        assert!(phone.take().iter().all(|r| r.purpose != Purpose::Connect));
        // The server stage is still there, and the scan can be asked for again.
        scan(&mut phone);
    }
    // Unasked, a failure shows nothing.
    let mut phone = at_connect();
    phone.event(UiEvent::ScanFailed {
        reason: ScanFailure::Denied,
    });
    assert_eq!(snackbar(&mut phone), None);
}
