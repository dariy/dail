//! The dail server in the front ends (SRV-6): the link's status text and details, the commands
//! the server refused offline as problems with their actions, and the paired devices.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AppState, Command, Ctx, Device, DeviceId, DeviceRole, Engine, IdGen, LinkState, LinkStatus,
    OfflineRejection, Origin, QueuedCommand, RequestId, ServerEndpoint,
};
use app_front::actions::{Run, device_actions, link_actions, rejection_actions};
use app_front::format::utc;
use app_front::i18n::{Key, Lang};
use app_front::palette::{Choice, Flow};
use app_front::server::{Tone, details, devices, rejections, shows_devices, summary};
use app_front::status::{Fix, problems};

use crate::support::{NOW, inbox_of, synced_state};

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

fn pinned() -> ServerEndpoint {
    ServerEndpoint {
        host: "home.example".into(),
        port: 7443,
        fingerprint: Some(FP.into()),
        ..ServerEndpoint::default()
    }
}

/// A mirror of alice's synced state, `status`, with `queued` commands waiting.
fn mirror(status: LinkStatus, queued: usize) -> AppState {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    let mut ids = IdGen::new(7);
    let queued = (0..queued)
        .map(|n| QueuedCommand {
            request_id: ids.next_id(),
            seed: n as u128,
            origin: Origin::Gui,
            command: Command::MarkRead {
                ids: vec![threads[0]],
                messages: Vec::new(),
            },
            queued_at_ms: NOW,
        })
        .collect();
    state.link = Some(LinkState {
        status,
        revision: 42,
        queued,
        server: Some(pinned()),
        ..LinkState::default()
    });
    state
}

fn refused(state: &mut AppState, seed: u128) -> RequestId {
    let (_, threads) = inbox_of(state);
    let id: RequestId = IdGen::new(seed).next_id();
    state
        .link
        .as_mut()
        .unwrap()
        .rejected
        .push(OfflineRejection {
            request_id: id,
            seed,
            origin: Origin::Gui,
            command: Command::ArchiveThreads {
                ids: vec![threads[0]],
                messages: Vec::new(),
            },
            code: "ERR004".into(),
            error: "ERR004: thread 1 not found".into(),
            at_ms: NOW - 3_600_000,
        });
    id
}

#[test]
fn the_link_reads_online_connecting_or_offline_with_its_queue() {
    assert_eq!(summary(&synced_state(), Lang::English), None);
    let text = |status, queued, lang| summary(&mirror(status, queued), lang).unwrap();
    assert_eq!(
        text(LinkStatus::Online, 0, Lang::English),
        app_front::server::Summary {
            text: "Online".into(),
            tone: Tone::Online
        }
    );
    assert_eq!(
        text(LinkStatus::Connecting, 0, Lang::English).text,
        "Connecting…"
    );
    assert_eq!(
        text(LinkStatus::Connecting, 0, Lang::English).tone,
        Tone::Busy
    );
    let offline = text(LinkStatus::Offline, 3, Lang::English);
    assert_eq!(
        (offline.text.as_str(), offline.tone),
        ("Offline · 3 queued", Tone::Offline)
    );
    assert_eq!(
        text(LinkStatus::Offline, 3, Lang::French).text,
        "Hors ligne · 3 en file"
    );
    assert_eq!(
        text(LinkStatus::Connecting, 1, Lang::PortuguesePortugal).text,
        "A ligar… · 1 em fila"
    );
}

#[test]
fn the_details_show_the_server_certificate_revision_and_error() {
    let mut state = mirror(LinkStatus::Offline, 2);
    state.link.as_mut().unwrap().error = Some("connect failed: connection refused".into());
    let fields: Vec<(String, Vec<String>)> = details(&state, Lang::English)
        .into_iter()
        .map(|f| (f.label, f.lines))
        .collect();
    assert_eq!(
        fields,
        [
            ("Server".to_owned(), vec!["home.example:7443".to_owned()]),
            (
                "Connection".to_owned(),
                vec!["Offline · 2 queued".to_owned()]
            ),
            (
                "Server certificate (SHA-256)".to_owned(),
                vec![
                    "5d41 402a bc4b 2a76 b971 9d91 1017 c592".to_owned(),
                    "ae2f 5e3c 8a1b 7d0f 4e6c 2a9b 8d7f 1e03".to_owned()
                ]
            ),
            ("Revision".to_owned(), vec!["42".to_owned()]),
            ("Waiting to be sent".to_owned(), vec!["2".to_owned()]),
            (
                "Last error".to_owned(),
                vec!["connect failed: connection refused".to_owned()]
            ),
        ]
    );
    // While pairing: the offered certificate, and the link says it is not paired yet.
    let state = AppState {
        link: Some(LinkState {
            server: Some(ServerEndpoint {
                host: "home.example".into(),
                port: 7443,
                offered: Some(FP.into()),
                ..ServerEndpoint::default()
            }),
            ..LinkState::default()
        }),
        ..AppState::default()
    };
    let fields = details(&state, Lang::English);
    assert_eq!(fields[1].lines, ["Connecting… · Not paired yet"]);
    assert_eq!(fields[2].label, "Server certificate (SHA-256)");
    assert!(details(&synced_state(), Lang::English).is_empty());
}

#[test]
fn the_link_actions_retry_refusals_and_disconnect_after_a_confirmation() {
    assert!(link_actions(&synced_state()).is_empty());
    let mut state = mirror(LinkStatus::Online, 0);
    let labels = |state: &AppState| {
        link_actions(state)
            .into_iter()
            .map(|e| e.label)
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(&state), [Key::DisconnectDots]);
    refused(&mut state, 9);
    assert_eq!(labels(&state), [Key::RetryRefused, Key::DisconnectDots]);
    let entries = link_actions(&state);
    assert_eq!(entries[0].run, Run::Command(Command::RetryOffline {}));
    let Flow::Ask(step) = entries[1].run.flow(&state) else {
        panic!("Disconnect… confirms")
    };
    assert!(step.prompt.starts_with("Disconnect from the dail server?"));
    assert!(matches!(
        step.answer(&state, Choice::Text("no".into())),
        Flow::Hidden
    ));
    let Flow::Run(cmd) = step.answer(&state, Choice::Yes) else {
        panic!()
    };
    assert_eq!(cmd, Command::DisconnectServer { confirm: true });
}

#[test]
fn a_refused_command_is_a_problem_that_can_be_retried_or_dismissed() {
    let mut state = mirror(LinkStatus::Online, 0);
    let first = refused(&mut state, 9);
    let second = refused(&mut state, 11);
    let listed = problems(&state, Lang::English);
    let refusals: Vec<_> = listed
        .iter()
        .filter(|p| matches!(p.fix, Some(Fix::Refused(_))))
        .collect();
    assert_eq!(refusals.len(), 2);
    assert_eq!(refusals[0].text, "Refused by the server: Archive");
    assert_eq!(refusals[0].detail, "ERR004: thread 1 not found");
    assert_eq!(refusals[0].fix, Some(Fix::Refused(first)));
    assert!(refusals[0].hint.as_ref().unwrap().contains("Retry all"));
    // The registry title and the engine error in the user's language.
    let french = &problems(&state, Lang::French)[0];
    assert_eq!(french.text, "Refusé par le serveur : Archiver");
    assert!(french.detail.starts_with("ERR004 : "), "{}", french.detail);

    let rows = rejections(&state, Lang::English, NOW, utc);
    assert_eq!(
        (rows[0].id, rows[0].text.as_str(), rows[0].when.as_str()),
        (first, "Refused by the server: Archive", "refused 07:00")
    );
    let entries = rejection_actions(&state, second);
    assert_eq!(
        entries
            .iter()
            .map(|e| (e.label, e.run.clone()))
            .collect::<Vec<_>>(),
        [
            (Key::RetryAll, Run::Command(Command::RetryOffline {})),
            (
                Key::Dismiss,
                Run::Command(Command::DismissOfflineRejection { id: second })
            ),
        ]
    );
    assert!(rejection_actions(&state, IdGen::new(99).next_id()).is_empty());

    // Dismissing one leaves the other; retrying queues it again.
    let mut engine = Engine::new(state);
    let ctx = Ctx {
        now_ms: NOW,
        id_seed: 5 << 64,
        origin: Origin::Gui,
    };
    engine
        .dispatch(Command::DismissOfflineRejection { id: second }, &ctx)
        .unwrap();
    assert_eq!(rejections(engine.state(), Lang::English, NOW, utc).len(), 1);
    engine.dispatch(Command::RetryOffline {}, &ctx).unwrap();
    let link = engine.state().link.as_ref().unwrap();
    assert!(link.rejected.is_empty());
    assert_eq!(link.queued.len(), 1);
    assert!(problems(engine.state(), Lang::English).is_empty());
}

#[test]
fn devices_list_their_role_pairing_and_last_connection_and_revoke_after_a_confirmation() {
    let mut state = synced_state();
    assert!(!shows_devices(&state));
    let mut ids = IdGen::new(3);
    let phone: DeviceId = ids.next_id();
    let agent: DeviceId = ids.next_id();
    for device in [
        Device {
            id: phone,
            name: "Phone\u{202e}".into(),
            role: DeviceRole::User,
            paired_at_ms: NOW - 86_400_000,
            last_seen_ms: Some(NOW - 300_000),
        },
        Device {
            id: agent,
            name: "Claude".into(),
            role: DeviceRole::Agent,
            paired_at_ms: NOW - 1_800_000,
            last_seen_ms: None,
        },
    ] {
        state.devices.insert(device.id, device);
    }
    assert!(shows_devices(&state));
    let rows = devices(&state, Lang::English, NOW, utc);
    let shown: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.name.as_str(),
                r.role.as_str(),
                r.paired.as_str(),
                r.seen.as_str(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            (
                "Phone\u{fffd}",
                "A device of yours",
                "paired Nov 14",
                "last seen 5m ago"
            ),
            ("Claude", "An agent", "paired 07:30", "never connected"),
        ]
    );
    let [entry] = &device_actions(&state, agent)[..] else {
        panic!()
    };
    assert_eq!(
        (entry.label, &entry.run),
        (Key::RevokeDots, &Run::Revoke(agent))
    );
    let Flow::Ask(step) = entry.run.flow(&state) else {
        panic!("Revoke… confirms")
    };
    assert!(matches!(
        &step.kind,
        app_front::palette::AskKind::Confirm(text) if text == "Revoke Claude"
    ));
    let Flow::Run(cmd) = step.answer(&state, Choice::Yes) else {
        panic!()
    };
    assert_eq!(
        cmd,
        Command::RevokeDevice {
            id: agent,
            confirm: true
        }
    );
    assert!(device_actions(&state, IdGen::new(99).next_id()).is_empty());
    // A device of a server shows the server's devices (they come with its diffs).
    let device = AppState {
        link: Some(LinkState {
            server: Some(pinned()),
            ..LinkState::default()
        }),
        ..AppState::default()
    };
    assert!(shows_devices(&device), "it can pair another device");
}
