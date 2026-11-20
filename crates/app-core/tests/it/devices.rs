//! The dail server's paired devices and a device's link to it (ADR 0017, SRV-2):
//! `list_devices`, `start_pairing`, `revoke_device`, `connect_server`, `trust_server`,
//! `disconnect_server`, `retry_offline`, `dismiss_offline_rejection` and their events.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::link::{is_pairing_code, pairing_code};
use app_core::{
    Command, CommandName, CoreError, Ctx, Device, DeviceId, DeviceRole, Effect, Engine, Event,
    LinkStatus, OfflineRejection, Origin, Outcome, PAIRING_TTL_MS, PersistDelta, Placement,
    ReplayInput, ReplayRecord, RequestId, Secret, SecretRef, ServerEndpoint, StateDiff,
};
use serde_json::json;

use crate::support::{Mail, T0, ctx, mail, names};

const FINGERPRINT: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

/// Dispatches `cmd` with seed `n << 64` from the CLI.
fn run(engine: &mut Engine, n: u128, cmd: Command) -> Result<Outcome, CoreError> {
    engine.dispatch(cmd, &ctx(n))
}

fn feed(engine: &mut Engine, n: u128, event: Event) -> Outcome {
    engine.ingest(event, &ctx(n)).unwrap()
}

fn device(n: u128, name: &str, role: DeviceRole) -> Device {
    Device {
        id: DeviceId::from_uuid(uuid::Uuid::from_u128(n)),
        name: name.to_owned(),
        role,
        paired_at_ms: T0,
        last_seen_ms: None,
    }
}

/// A server with two paired devices.
fn server() -> Engine {
    let mut engine = Engine::default();
    feed(
        &mut engine,
        1,
        Event::DevicePaired {
            device: device(1, "Phone", DeviceRole::User),
        },
    );
    feed(
        &mut engine,
        2,
        Event::DevicePaired {
            device: device(2, "Claude", DeviceRole::Agent),
        },
    );
    engine
}

/// A profile without accounts that connected to `home.example:7443`, whose certificate the
/// server presented.
fn offered() -> Engine {
    let mut engine = Engine::default();
    run(
        &mut engine,
        1,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443,
        },
    )
    .unwrap();
    feed(
        &mut engine,
        2,
        Event::ServerOffered {
            host: "home.example".into(),
            port: 7443,
            fingerprint: FINGERPRINT.into(),
        },
    );
    engine
}

fn trust(code: &str) -> Command {
    Command::TrustServer {
        fingerprint: FINGERPRINT.into(),
        code: Secret::new(code),
    }
}

/// A paired mirror whose server refused two queued commands.
fn with_rejections() -> Engine {
    let mut engine = offered();
    run(&mut engine, 3, trust("ABCD2345")).unwrap();
    feed(
        &mut engine,
        4,
        Event::ServerPaired {
            fingerprint: FINGERPRINT.into(),
        },
    );
    feed(
        &mut engine,
        4,
        Event::LinkChanged {
            status: LinkStatus::Offline,
        },
    );
    let mut state = engine.state().clone();
    let link = state.link.as_mut().unwrap();
    for (n, origin) in [(10_u128, Origin::Mcp), (11, Origin::Gui)] {
        link.rejected.push(OfflineRejection {
            request_id: RequestId::from_uuid(uuid::Uuid::from_u128(n << 64)),
            seed: n << 64,
            origin,
            command: Command::SyncNow { account: None },
            code: "ERR004".into(),
            error: "ERR004: account x not found".into(),
            at_ms: T0,
        });
    }
    Engine::new(state)
}

fn invalid(field: &str, reason: &str) -> CoreError {
    CoreError::InvalidArgument {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

#[test]
fn the_new_commands_are_classified() {
    use app_core::{AgentAccess, CommandKind};
    let table = [
        (
            CommandName::ListDevices,
            CommandKind::Query,
            AgentAccess::Allowed,
            Placement::Server,
        ),
        (
            CommandName::StartPairing,
            CommandKind::Mutation,
            AgentAccess::UserOnly,
            Placement::Server,
        ),
        (
            CommandName::RevokeDevice,
            CommandKind::Destructive,
            AgentAccess::UserOnly,
            Placement::Server,
        ),
        (
            CommandName::ConnectServer,
            CommandKind::Mutation,
            AgentAccess::UserOnly,
            Placement::Client,
        ),
        (
            CommandName::TrustServer,
            CommandKind::Mutation,
            AgentAccess::UserOnly,
            Placement::Client,
        ),
        (
            CommandName::DisconnectServer,
            CommandKind::Destructive,
            AgentAccess::UserOnly,
            Placement::Client,
        ),
        (
            CommandName::RetryOffline,
            CommandName::RetryPendingOps.kind(),
            CommandName::RetryPendingOps.agent_access(),
            Placement::Client,
        ),
        (
            CommandName::DismissOfflineRejection,
            CommandName::DiscardPendingOp.kind(),
            CommandName::DiscardPendingOp.agent_access(),
            Placement::Client,
        ),
    ];
    for (name, kind, agents, placement) in table {
        assert_eq!(name.kind(), kind, "{name:?}");
        assert_eq!(name.agent_access(), agents, "{name:?}");
        assert_eq!(name.placement(), placement, "{name:?}");
    }
    // `start_pairing` is classified like `sync_now`: it changes the world, not the state, and
    // has no undo.
    assert_eq!(
        CommandName::StartPairing.kind(),
        CommandName::SyncNow.kind()
    );
}

#[test]
fn the_new_effects_are_placed() {
    let secret = SecretRef::for_server(uuid::Uuid::from_u128(1));
    let device_local = [
        Effect::ProbeServer {
            host: "h".into(),
            port: 1,
        },
        Effect::PairWithServer {
            host: "h".into(),
            port: 1,
            fingerprint: FINGERPRINT.into(),
            code: Secret::new("ABCD2345"),
            secret: secret.clone(),
        },
        Effect::DisconnectServer {
            secret: Some(secret),
        },
    ];
    for effect in device_local {
        assert!(effect.is_device_local(), "{}", effect.name());
    }
    let server = [
        Effect::OpenPairing {
            code: Secret::new("ABCD2345"),
            role: DeviceRole::User,
            name: "Phone".into(),
            expires_at_ms: T0,
        },
        Effect::RevokeDevice {
            id: DeviceId::from_uuid(uuid::Uuid::from_u128(1)),
        },
    ];
    for effect in server {
        assert!(!effect.is_device_local(), "{}", effect.name());
        assert!(matches!(
            effect.failure_event("down".into()),
            Event::ServeFailed { .. }
        ));
    }
    assert_eq!(
        SecretRef::for_server(uuid::Uuid::from_u128(1)).user,
        "server/00000000-0000-0000-0000-000000000001"
    );
}

#[test]
fn list_devices_returns_them_in_pairing_order() {
    let mut engine = Engine::default();
    let empty = run(&mut engine, 3, Command::ListDevices {}).unwrap();
    assert_eq!(empty.receipt.data, json!({ "devices": [] }));
    let mut engine = server();
    let before = engine.state().clone();
    let listed = run(&mut engine, 3, Command::ListDevices {}).unwrap();
    assert_eq!(listed.receipt.data["devices"][0]["name"], "Phone");
    assert_eq!(listed.receipt.data["devices"][1]["role"], "agent");
    assert!(listed.effects.is_empty());
    assert_eq!(engine.state(), &before);
}

#[test]
fn start_pairing_puts_a_code_in_the_receipt_only() {
    let mut engine = server();
    let before = engine.state().clone();
    let outcome = run(
        &mut engine,
        5,
        Command::StartPairing {
            role: DeviceRole::Agent,
            name: "  Claude  ".into(),
        },
    )
    .unwrap();
    let code = outcome.receipt.data["code"].as_str().unwrap().to_owned();
    assert!(is_pairing_code(&code), "{code}");
    assert_eq!(code, pairing_code(5 << 64));
    let expires = T0 + 5000 + PAIRING_TTL_MS;
    assert_eq!(
        outcome.receipt.data,
        json!({
            "code": code,
            "role": "agent",
            "name": "Claude",
            "expires_at_ms": expires,
            "undoable": false,
        })
    );
    assert_eq!(names(&outcome.effects), ["open_pairing"]);
    let Effect::OpenPairing {
        code: opened,
        role,
        name,
        expires_at_ms,
    } = &outcome.effects[0]
    else {
        panic!("{:?}", outcome.effects);
    };
    assert_eq!(opened.expose(), code);
    assert_eq!(
        (*role, name.as_str(), *expires_at_ms),
        (DeviceRole::Agent, "Claude", expires)
    );
    // Never in the state, nor in the effect's JSON.
    assert_eq!(engine.state(), &before);
    assert!(
        !serde_json::to_string(engine.state())
            .unwrap()
            .contains(&code)
    );
    assert!(
        !serde_json::to_string(&outcome.effects)
            .unwrap()
            .contains(&code)
    );
    assert!(outcome.receipt.undo_label.is_none());
    assert_eq!(engine.history().undo_len(), 0);
}

#[test]
fn pairing_codes_use_every_part_of_the_seed() {
    // Runtime seeds are random UUIDs; test and replay seeds count in the high 64 bits.
    let codes = [
        pairing_code(0),
        pairing_code(1),
        pairing_code(1 << 64),
        pairing_code(2 << 64),
        pairing_code(1 << 100),
        pairing_code(u128::MAX),
    ];
    for code in &codes {
        assert!(is_pairing_code(code), "{code}");
    }
    let distinct: std::collections::BTreeSet<&String> = codes.iter().collect();
    assert_eq!(distinct.len(), codes.len(), "{codes:?}");
    assert_eq!(pairing_code(0), "AAAAAAAA");
    assert!(!is_pairing_code("abcd2345"));
    assert!(!is_pairing_code("ABCD234"));
    assert!(!is_pairing_code("ABCD2341"));
}

#[test]
fn start_pairing_refuses_bad_names_and_agents() {
    let reason = "must be 1 to 64 characters without control characters";
    for name in ["", "   ", "Ph\none", &"x".repeat(65)] {
        let mut engine = server();
        let err = run(
            &mut engine,
            5,
            Command::StartPairing {
                role: DeviceRole::User,
                name: name.into(),
            },
        )
        .unwrap_err();
        assert_eq!(err, invalid("name", reason), "{name:?}");
    }
    let mut engine = server();
    let ok = Command::StartPairing {
        role: DeviceRole::User,
        name: "é".repeat(64),
    };
    run(&mut engine, 5, ok.clone()).unwrap();
    let err = engine
        .dispatch(
            ok,
            &Ctx {
                origin: Origin::Mcp,
                ..ctx(6)
            },
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }), "{err:?}");
}

#[test]
fn revoke_device_removes_it_and_tells_the_server() {
    let mut engine = server();
    let id = DeviceId::from_uuid(uuid::Uuid::from_u128(1));
    let outcome = run(&mut engine, 5, Command::RevokeDevice { id, confirm: true }).unwrap();
    assert_eq!(
        outcome.receipt.data,
        json!({ "revoked": id, "name": "Phone", "undoable": false })
    );
    assert_eq!(outcome.effects, [Effect::RevokeDevice { id }]);
    assert_eq!(engine.state().devices.len(), 1);
    assert!(!engine.state().devices.contains_key(&id));
    assert_eq!(engine.history().undo_len(), 0);
}

#[test]
fn revoke_device_needs_confirm_and_a_known_device() {
    let mut engine = server();
    let id = DeviceId::from_uuid(uuid::Uuid::from_u128(1));
    let before = engine.state().clone();
    let err = run(&mut engine, 5, Command::RevokeDevice { id, confirm: false }).unwrap_err();
    assert_eq!(
        err,
        CoreError::ConfirmationRequired {
            command: "revoke_device".into()
        }
    );
    let unknown = DeviceId::from_uuid(uuid::Uuid::from_u128(9));
    let err = run(
        &mut engine,
        6,
        Command::RevokeDevice {
            id: unknown,
            confirm: true,
        },
    )
    .unwrap_err();
    assert_eq!(
        err,
        CoreError::NotFound {
            entity: "device".into(),
            id: unknown.to_string()
        }
    );
    assert_eq!(engine.state(), &before);
}

#[test]
fn device_events_update_the_list_and_the_serve_error() {
    let mut engine = server();
    let phone = DeviceId::from_uuid(uuid::Uuid::from_u128(1));
    feed(
        &mut engine,
        5,
        Event::DeviceSeen {
            id: phone,
            at_ms: T0 + 9,
        },
    );
    assert_eq!(engine.state().devices[&phone].last_seen_ms, Some(T0 + 9));
    let before = engine.state().clone();
    let ignored = feed(
        &mut engine,
        6,
        Event::DeviceSeen {
            id: DeviceId::from_uuid(uuid::Uuid::from_u128(9)),
            at_ms: T0,
        },
    );
    assert_eq!(ignored.receipt.data, json!({ "ignored": true }));
    assert_eq!(engine.state(), &before);
    feed(
        &mut engine,
        7,
        Event::ServeFailed {
            error: "not serving".into(),
        },
    );
    assert_eq!(
        engine.state().status.serve_error.as_deref(),
        Some("not serving")
    );
    // A device paired again replaces its entry, and pairing resolves the failure.
    let renamed = Device {
        name: "New phone".into(),
        ..device(1, "", DeviceRole::User)
    };
    feed(&mut engine, 8, Event::DevicePaired { device: renamed });
    assert_eq!(engine.state().devices.len(), 2);
    assert_eq!(engine.state().devices[&phone].name, "New phone");
    assert_eq!(engine.state().status.serve_error, None);
    assert_eq!(engine.history().undo_len(), 0);
}

#[test]
fn devices_are_shared_with_the_mirrors() {
    let old = Engine::default().state().clone();
    let new = server().state().clone();
    let diff = StateDiff::diff(&old, &new);
    assert_eq!(diff.devices.upsert.len(), 2);
    let mut mirror = old;
    diff.apply(&mut mirror);
    assert_eq!(mirror.devices, new.devices);
    // A paired desktop lists them from its mirror.
    let mut device = offered();
    feed(
        &mut device,
        3,
        Event::RemoteSynced {
            revision: 1,
            diff: Box::new(StateDiff::snapshot(&new)),
            snapshot: true,
        },
    );
    assert_eq!(device.state().devices, new.devices);
    assert!(device.state().link.as_ref().unwrap().server.is_some());
}

#[test]
fn on_a_mirror_pairing_commands_are_queued_for_the_server() {
    let mut engine = offered();
    feed(
        &mut engine,
        3,
        Event::LinkChanged {
            status: LinkStatus::Offline,
        },
    );
    let outcome = run(
        &mut engine,
        4,
        Command::StartPairing {
            role: DeviceRole::User,
            name: "Tablet".into(),
        },
    )
    .unwrap();
    // The server opens the code with the same seed, so it is the same code.
    assert_eq!(outcome.receipt.data["code"], pairing_code(4 << 64));
    assert!(outcome.effects.is_empty(), "{:?}", outcome.effects);
    let queued = &engine.state().link.as_ref().unwrap().queued;
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].seed, 4 << 64);
}

#[test]
fn connect_server_links_the_profile_and_probes_the_server() {
    let mut engine = Engine::default();
    let outcome = run(
        &mut engine,
        1,
        Command::ConnectServer {
            host: " home.example ".into(),
            port: 7443,
        },
    )
    .unwrap();
    assert_eq!(
        outcome.receipt.data,
        json!({ "host": "home.example", "port": 7443, "undoable": false })
    );
    // The probe is the device's own work: mirror mode lets it out.
    assert_eq!(
        outcome.effects,
        [Effect::ProbeServer {
            host: "home.example".into(),
            port: 7443
        }]
    );
    let link = engine.state().link.as_ref().unwrap();
    assert_eq!(link.status, LinkStatus::Connecting);
    let server = link.server.as_ref().unwrap();
    assert_eq!((server.host.as_str(), server.port), ("home.example", 7443));
    assert!(server.offered.is_none() && !server.is_pinned());
    assert_eq!(engine.history().undo_len(), 0);
    // Another server, before trusting this one, replaces it.
    run(
        &mut engine,
        2,
        Command::ConnectServer {
            host: "[fd7a::1]".into(),
            port: 7443,
        },
    )
    .unwrap();
    let server = engine
        .state()
        .link
        .as_ref()
        .unwrap()
        .server
        .as_ref()
        .unwrap();
    assert_eq!(server.host, "[fd7a::1]");
}

#[test]
fn connect_server_refuses_bad_input_accounts_and_a_second_server() {
    let cases = [
        (
            "",
            7443,
            invalid("host", "host must be a DNS name or IP address"),
        ),
        (
            "ho st",
            7443,
            invalid("host", "host must be a DNS name or IP address"),
        ),
        (
            "home.example",
            0,
            invalid("port", "port must be between 1 and 65535"),
        ),
    ];
    for (host, port, expected) in cases {
        let mut engine = Engine::default();
        let err = run(
            &mut engine,
            1,
            Command::ConnectServer {
                host: host.into(),
                port,
            },
        )
        .unwrap_err();
        assert_eq!(err, expected, "{host:?}:{port}");
        assert!(engine.state().link.is_none());
    }
    let mut m = Mail::hostinger();
    let err = m
        .run(Command::ConnectServer {
            host: "home.example".into(),
            port: 7443,
        })
        .unwrap_err();
    assert_eq!(err, CoreError::ProfileHasAccounts { accounts: 1 });
    assert_eq!(err.code(), "ERR013");
    assert!(err.to_string().starts_with("ERR013: "));
    assert!(m.engine.state().link.is_none());
    let mut engine = offered();
    run(&mut engine, 3, trust("ABCD2345")).unwrap();
    feed(
        &mut engine,
        4,
        Event::ServerPaired {
            fingerprint: FINGERPRINT.into(),
        },
    );
    let err = run(
        &mut engine,
        4,
        Command::ConnectServer {
            host: "other.example".into(),
            port: 7443,
        },
    )
    .unwrap_err();
    assert_eq!(
        err,
        CoreError::AlreadyExists {
            entity: "server link".into(),
            key: "home.example:7443".into()
        }
    );
}

#[test]
fn server_offered_records_the_fingerprint_of_this_server_only() {
    let mut engine = offered();
    let server = engine
        .state()
        .link
        .as_ref()
        .unwrap()
        .server
        .clone()
        .unwrap();
    assert_eq!(server.offered.as_deref(), Some(FINGERPRINT));
    let before = engine.state().clone();
    for (host, port, fingerprint) in [
        ("other.example", 7443, FINGERPRINT),
        ("home.example", 7444, FINGERPRINT),
        ("home.example", 7443, "not hex"),
    ] {
        let outcome = feed(
            &mut engine,
            3,
            Event::ServerOffered {
                host: host.into(),
                port,
                fingerprint: fingerprint.into(),
            },
        );
        assert_eq!(outcome.receipt.data, json!({ "ignored": true }));
        assert_eq!(engine.state(), &before);
    }
    // Without a link, nothing turns on.
    let mut plain = Engine::default();
    feed(
        &mut plain,
        1,
        Event::ServerOffered {
            host: "home.example".into(),
            port: 7443,
            fingerprint: FINGERPRINT.into(),
        },
    );
    assert!(plain.state().link.is_none());
}

#[test]
fn trust_server_pins_the_fingerprint_and_pairs() {
    let mut engine = offered();
    let shown = FINGERPRINT
        .to_uppercase()
        .as_bytes()
        .chunks(2)
        .map(|pair| std::str::from_utf8(pair).unwrap())
        .collect::<Vec<_>>()
        .join(":");
    let outcome = run(
        &mut engine,
        3,
        Command::TrustServer {
            fingerprint: shown,
            code: Secret::new(" abcd2345 "),
        },
    )
    .unwrap();
    let token = SecretRef::for_server(uuid::Uuid::from_u128(3 << 64));
    assert_eq!(
        outcome.receipt.data,
        json!({
            "host": "home.example",
            "port": 7443,
            "fingerprint": FINGERPRINT,
            "undoable": false,
        })
    );
    assert_eq!(
        outcome.effects,
        [Effect::PairWithServer {
            host: "home.example".into(),
            port: 7443,
            fingerprint: FINGERPRINT.into(),
            code: Secret::new("ABCD2345"),
            secret: token.clone(),
        }]
    );
    let server = engine
        .state()
        .link
        .as_ref()
        .unwrap()
        .server
        .clone()
        .unwrap();
    // Pinned only once the server accepts the code.
    assert_eq!(server.fingerprint, None);
    assert_eq!(server.pairing.as_deref(), Some(FINGERPRINT));
    assert_eq!(server.token, Some(token));
    let ignored = feed(
        &mut engine,
        4,
        Event::ServerPaired {
            fingerprint: "00".repeat(32),
        },
    );
    assert_eq!(ignored.receipt.data, json!({ "ignored": true }));
    assert!(ignored.effects.is_empty());
    let paired = feed(
        &mut engine,
        4,
        Event::ServerPaired {
            fingerprint: FINGERPRINT.into(),
        },
    );
    let server = engine
        .state()
        .link
        .as_ref()
        .unwrap()
        .server
        .clone()
        .unwrap();
    assert_eq!(server.fingerprint.as_deref(), Some(FINGERPRINT));
    assert_eq!(server.pairing, None);
    // The remote profile is written (`[remote]` in `config.toml`), without the offer.
    assert_eq!(
        paired.effects,
        [Effect::PersistState {
            delta: PersistDelta::Remote(Some(ServerEndpoint {
                offered: None,
                ..server.clone()
            }))
        }]
    );
    assert!(
        !serde_json::to_string(engine.state())
            .unwrap()
            .contains("ABCD2345")
    );
    assert!(
        !serde_json::to_string(&outcome.effects)
            .unwrap()
            .contains("ABCD2345")
    );
    assert_eq!(engine.history().undo_len(), 0);
    // Online clears an earlier error.
    feed(
        &mut engine,
        4,
        Event::LinkFailed {
            error: "refused".into(),
        },
    );
    let link = engine.state().link.as_ref().unwrap();
    assert_eq!(
        (link.status, link.error.as_deref()),
        (LinkStatus::Offline, Some("refused"))
    );
    feed(
        &mut engine,
        5,
        Event::LinkChanged {
            status: LinkStatus::Online,
        },
    );
    assert_eq!(engine.state().link.as_ref().unwrap().error, None);
}

#[test]
fn trust_server_refuses_without_a_matching_offer_or_code() {
    let mut plain = Engine::default();
    assert_eq!(
        run(&mut plain, 1, trust("ABCD2345")).unwrap_err(),
        CoreError::NotLinked
    );
    let mut unoffered = Engine::default();
    run(
        &mut unoffered,
        1,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443,
        },
    )
    .unwrap();
    assert_eq!(
        run(&mut unoffered, 2, trust("ABCD2345")).unwrap_err(),
        invalid(
            "fingerprint",
            "the server has not shown a certificate yet; wait for it after connect_server"
        )
    );
    let cases = [
        (
            "00".repeat(32),
            "ABCD2345",
            invalid(
                "fingerprint",
                "does not match the certificate the server presented; do not pair",
            ),
        ),
        (
            "zz".into(),
            "ABCD2345",
            invalid(
                "fingerprint",
                "expected a SHA-256 fingerprint: 64 hex digits",
            ),
        ),
        (
            FINGERPRINT.into(),
            "ABCD234",
            invalid(
                "code",
                "expected the 8-character pairing code from start_pairing (A-Z, 2-7)",
            ),
        ),
        (
            FINGERPRINT.into(),
            "ABCD2341",
            invalid(
                "code",
                "expected the 8-character pairing code from start_pairing (A-Z, 2-7)",
            ),
        ),
    ];
    for (fingerprint, code, expected) in cases {
        let mut engine = offered();
        let before = engine.state().clone();
        let err = run(
            &mut engine,
            3,
            Command::TrustServer {
                fingerprint: fingerprint.clone(),
                code: Secret::new(code),
            },
        )
        .unwrap_err();
        assert_eq!(err, expected, "{fingerprint} {code}");
        assert_eq!(engine.state(), &before);
    }
    let mut paired = offered();
    run(&mut paired, 3, trust("ABCD2345")).unwrap();
    feed(
        &mut paired,
        4,
        Event::ServerPaired {
            fingerprint: FINGERPRINT.into(),
        },
    );
    assert_eq!(
        run(&mut paired, 4, trust("ABCD2345")).unwrap_err(),
        CoreError::AlreadyExists {
            entity: "server link".into(),
            key: "home.example:7443".into()
        }
    );
    let err = paired
        .dispatch(
            trust("ABCD2345"),
            &Ctx {
                origin: Origin::McpAllowSend,
                ..ctx(5)
            },
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }), "{err:?}");
}

#[test]
fn disconnect_server_forgets_the_link_and_the_mirror() {
    let mut engine = with_rejections();
    // Something mirrored and something queued.
    let snapshot = StateDiff::snapshot(server().state());
    feed(
        &mut engine,
        5,
        Event::RemoteSynced {
            revision: 3,
            diff: Box::new(snapshot),
            snapshot: true,
        },
    );
    run(&mut engine, 6, Command::SyncNow { account: None }).unwrap();
    assert_eq!(engine.state().link.as_ref().unwrap().queued.len(), 1);
    assert_eq!(engine.state().devices.len(), 2);
    let token = engine
        .state()
        .link
        .as_ref()
        .unwrap()
        .server
        .as_ref()
        .unwrap()
        .token
        .clone();
    assert!(token.is_some());
    let outcome = run(&mut engine, 7, Command::DisconnectServer { confirm: true }).unwrap();
    assert_eq!(
        outcome.receipt.data,
        json!({ "disconnected": "home.example:7443", "lost": 1, "undoable": false })
    );
    // `[remote]` leaves `config.toml`: the profile is a plain one again.
    assert_eq!(
        outcome.effects,
        [
            Effect::PersistState {
                delta: PersistDelta::Remote(None)
            },
            Effect::DisconnectServer { secret: token }
        ]
    );
    assert!(engine.state().link.is_none());
    assert!(engine.state().devices.is_empty());
    assert_eq!(engine.history().undo_len(), 0);
    // The token's deletion failing shows as a keyring error.
    feed(
        &mut engine,
        8,
        Event::LinkFailed {
            error: "locked".into(),
        },
    );
    assert_eq!(
        engine.state().status.keyring_error.as_deref(),
        Some("locked")
    );
    assert!(engine.state().link.is_none());
}

#[test]
fn disconnect_server_needs_confirm_and_a_link() {
    let mut engine = offered();
    let before = engine.state().clone();
    assert_eq!(
        run(&mut engine, 3, Command::DisconnectServer { confirm: false }).unwrap_err(),
        CoreError::ConfirmationRequired {
            command: "disconnect_server".into()
        }
    );
    assert_eq!(engine.state(), &before);
    let mut plain = Engine::default();
    let err = run(&mut plain, 1, Command::DisconnectServer { confirm: true }).unwrap_err();
    assert_eq!(err, CoreError::NotLinked);
    assert_eq!(err.code(), "ERR012");
    assert!(err.to_string().starts_with("ERR012: "));
    // Before pairing there is no token to delete.
    let outcome = run(&mut engine, 3, Command::DisconnectServer { confirm: true }).unwrap();
    assert_eq!(outcome.effects, [Effect::DisconnectServer { secret: None }]);
    assert_eq!(outcome.receipt.data["lost"], 0);
}

#[test]
fn retry_offline_queues_the_refused_commands_under_new_request_ids() {
    let mut engine = with_rejections();
    let rejected = engine.state().link.as_ref().unwrap().rejected.clone();
    let outcome = run(&mut engine, 20, Command::RetryOffline {}).unwrap();
    let link = engine.state().link.as_ref().unwrap();
    assert!(link.rejected.is_empty());
    assert_eq!(link.queued.len(), 2);
    for (queued, rejection) in link.queued.iter().zip(&rejected) {
        assert_ne!(queued.request_id, rejection.request_id);
        assert_eq!(queued.seed, rejection.seed);
        assert_eq!(queued.origin, rejection.origin);
        assert_eq!(queued.command, rejection.command);
        assert_eq!(queued.queued_at_ms, T0 + 20_000);
    }
    let first = RequestId::from_uuid(uuid::Uuid::from_u128(20 << 64));
    let second = RequestId::from_uuid(uuid::Uuid::from_u128((20 << 64) + 1));
    assert_eq!(
        outcome.receipt.data,
        json!({ "retried": [first, second], "undoable": false })
    );
    assert_eq!(link.queued[0].request_id, first);
    // The agent's command stays the agent's.
    assert_eq!(link.queued[0].origin, Origin::Mcp);
    assert!(outcome.effects.is_empty());
    // Nothing left: still fine.
    let again = run(&mut engine, 21, Command::RetryOffline {}).unwrap();
    assert_eq!(again.receipt.data["retried"], json!([]));
    assert_eq!(
        run(&mut Engine::default(), 1, Command::RetryOffline {}).unwrap_err(),
        CoreError::NotLinked
    );
    assert_eq!(engine.history().undo_len(), 0);
}

#[test]
fn dismiss_offline_rejection_drops_one() {
    let mut engine = with_rejections();
    let id = RequestId::from_uuid(uuid::Uuid::from_u128(10 << 64));
    let outcome = run(&mut engine, 20, Command::DismissOfflineRejection { id }).unwrap();
    assert_eq!(
        outcome.receipt.data,
        json!({ "dismissed": id, "command": "sync_now", "undoable": false })
    );
    let rejected = &engine.state().link.as_ref().unwrap().rejected;
    assert_eq!(rejected.len(), 1);
    assert_ne!(rejected[0].request_id, id);
    let before = engine.state().clone();
    assert_eq!(
        run(&mut engine, 21, Command::DismissOfflineRejection { id }).unwrap_err(),
        CoreError::NotFound {
            entity: "offline rejection".into(),
            id: id.to_string()
        }
    );
    assert_eq!(engine.state(), &before);
    assert_eq!(
        run(
            &mut Engine::default(),
            1,
            Command::DismissOfflineRejection { id }
        )
        .unwrap_err(),
        CoreError::NotLinked
    );
}

#[test]
fn none_of_them_is_undoable() {
    let mut engine = with_rejections();
    let id = RequestId::from_uuid(uuid::Uuid::from_u128(10 << 64));
    run(&mut engine, 20, Command::DismissOfflineRejection { id }).unwrap();
    run(&mut engine, 21, Command::RetryOffline {}).unwrap();
    run(&mut engine, 22, Command::ListDevices {}).unwrap();
    run(
        &mut engine,
        23,
        Command::StartPairing {
            role: DeviceRole::User,
            name: "Tablet".into(),
        },
    )
    .unwrap();
    assert_eq!(
        run(&mut engine, 24, Command::Undo {}).unwrap_err(),
        CoreError::NothingToUndo
    );
    assert_eq!(
        run(&mut engine, 25, Command::Redo {}).unwrap_err(),
        CoreError::NothingToRedo
    );
}

#[test]
fn the_audit_log_hides_the_pairing_code() {
    let at = ctx(5);
    let record = ReplayRecord::new(
        &at,
        ReplayInput::Command(Command::StartPairing {
            role: DeviceRole::User,
            name: "Phone".into(),
        }),
    );
    let line = record.redacted_line();
    // The code derives from the seed: the line has none.
    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(value.get("seed").is_none(), "{line}");
    assert!(!line.contains(&pairing_code(at.id_seed)));
    // Its replay leaves the same state (the code is never in it).
    let mut original = Engine::default();
    let ReplayInput::Command(command) = record.input.clone() else {
        panic!("{record:?}");
    };
    original.dispatch(command, &at).unwrap();
    let parsed: ReplayRecord = serde_json::from_str(&line).unwrap();
    let mut replayed = Engine::default();
    app_core::replay::run(&mut replayed, vec![(1, parsed)]);
    assert_eq!(replayed.state(), original.state());
    let trusted = ReplayRecord::new(&at, ReplayInput::Command(trust("ABCD2345"))).redacted_line();
    assert!(!trusted.contains("ABCD2345"), "{trusted}");
    // Other lines keep their seed.
    assert!(trusted.contains("\"seed\""), "{trusted}");
}

#[test]
fn a_failed_pairing_can_be_retried_with_a_new_code() {
    let mut engine = offered();
    let first = run(&mut engine, 3, trust("ABCD2345")).unwrap();
    let Effect::PairWithServer { secret: token, .. } = first.effects[0].clone() else {
        panic!("{:?}", first.effects);
    };
    // The code was mistyped or expired: the server refused it.
    feed(
        &mut engine,
        4,
        Event::LinkFailed {
            error: "the server refused the pairing code".into(),
        },
    );
    let state = engine.state().clone();
    let server = state.link.as_ref().unwrap().server.as_ref().unwrap();
    assert!(!server.is_pinned());
    assert_eq!(server.pairing, None);
    assert_eq!(server.offered.as_deref(), Some(FINGERPRINT));
    // The palette offers it again, and a new code pairs.
    assert!((CommandName::TrustServer.spec().available)(&state));
    let again = run(&mut engine, 5, trust("QRST2345")).unwrap();
    let Effect::PairWithServer { secret, code, .. } = &again.effects[0] else {
        panic!("{:?}", again.effects);
    };
    assert_eq!(code.expose(), "QRST2345");
    assert_eq!(*secret, token, "a retry keeps the keyring entry");
    feed(
        &mut engine,
        6,
        Event::ServerPaired {
            fingerprint: FINGERPRINT.into(),
        },
    );
    feed(
        &mut engine,
        7,
        Event::LinkChanged {
            status: LinkStatus::Online,
        },
    );
    let link = engine.state().link.clone().unwrap();
    assert_eq!(link.status, LinkStatus::Online);
    assert_eq!(
        link.server.unwrap().fingerprint.as_deref(),
        Some(FINGERPRINT)
    );
    assert!(!(CommandName::TrustServer.spec().available)(engine.state()));
}

#[test]
fn disconnect_server_removes_the_mirror_from_the_store() {
    let mut m = Mail::hostinger();
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Hi", false)]);
    let mirrored = m.engine.state().clone();
    let mut engine = offered();
    feed(
        &mut engine,
        3,
        Event::RemoteSynced {
            revision: 1,
            diff: Box::new(StateDiff::snapshot(&mirrored)),
            snapshot: true,
        },
    );
    let outcome = run(&mut engine, 4, Command::DisconnectServer { confirm: true }).unwrap();
    let removed_account = outcome.effects.iter().any(|e| {
        matches!(e, Effect::PersistState { delta: PersistDelta::AccountRemoved(id) } if *id == m.account)
    });
    assert!(removed_account, "{:?}", names(&outcome.effects));
    let mail = outcome
        .effects
        .iter()
        .find_map(|e| match e {
            Effect::PersistState {
                delta: PersistDelta::Mail(mail),
            } => Some(mail),
            _ => None,
        })
        .unwrap();
    assert_eq!(mail.removed_mailboxes.len(), mirrored.mailboxes.len());
    assert_eq!(mail.removed_messages.len(), mirrored.messages.len());
    assert_eq!(mail.removed_threads.len(), mirrored.threads.len());
    assert!(!mirrored.messages.is_empty());
    assert_eq!(names(&outcome.effects).last(), Some(&"disconnect_server"));
    assert!(engine.state().accounts.is_empty() && engine.state().messages.is_empty());
}
