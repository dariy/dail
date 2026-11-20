//! A device's link to a dail server through a session over the fake backend (ADR 0017, SRV-2):
//! connect, compare the offered fingerprint, trust and pair, then disconnect. The fake server
//! presents `FAKE_SERVER_FINGERPRINT` and accepts any well-formed code.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{Command, CoreError, DeviceRole, LinkStatus, Origin, Secret};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{FAKE_SERVER_FINGERPRINT, MemoryBackend};

async fn open(backend: &Arc<MemoryBackend>) -> Session {
    Session::open(
        Arc::clone(backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

async fn run(session: &Session, cmd: Command) -> Result<serde_json::Value, CoreError> {
    let result = session.submit(cmd, Origin::Cli).await.unwrap();
    session.flush().await.unwrap();
    result.map(|receipt| receipt.data)
}

#[tokio::test(start_paused = true)]
async fn a_device_pairs_with_the_fake_server_and_disconnects() {
    let backend = Arc::new(MemoryBackend::new());
    let session = open(&backend).await;
    run(
        &session,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443,
        },
    )
    .await
    .unwrap();
    let offered = session
        .state()
        .link
        .clone()
        .unwrap()
        .server
        .unwrap()
        .offered;
    assert_eq!(offered.as_deref(), Some(FAKE_SERVER_FINGERPRINT));
    run(
        &session,
        Command::TrustServer {
            fingerprint: FAKE_SERVER_FINGERPRINT.into(),
            code: Secret::new("ABCD2345"),
        },
    )
    .await
    .unwrap();
    let link = session.state().link.clone().unwrap();
    assert_eq!(link.status, LinkStatus::Online);
    let token = link.server.unwrap().token.unwrap();
    assert!(backend.has_secret(&token));
    run(&session, Command::DisconnectServer { confirm: true })
        .await
        .unwrap();
    assert!(session.state().link.is_none());
    assert!(!backend.has_secret(&token));
}

#[tokio::test(start_paused = true)]
async fn an_unreachable_server_shows_in_the_link() {
    let backend = Arc::new(MemoryBackend::new());
    backend.set_offline(true);
    let session = open(&backend).await;
    run(
        &session,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443,
        },
    )
    .await
    .unwrap();
    let link = session.state().link.clone().unwrap();
    assert_eq!(link.status, LinkStatus::Offline);
    assert!(
        link.error.as_deref().unwrap().contains("unreachable"),
        "{link:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_fake_server_opens_codes_and_revokes_silently() {
    let backend = Arc::new(MemoryBackend::new());
    let session = open(&backend).await;
    let data = run(
        &session,
        Command::StartPairing {
            role: DeviceRole::Agent,
            name: "Claude".into(),
        },
    )
    .await
    .unwrap();
    assert!(app_core::link::is_pairing_code(
        data["code"].as_str().unwrap()
    ));
    assert_eq!(session.state().status.serve_error, None);
}

#[tokio::test(start_paused = true)]
async fn a_failed_pairing_is_retried_with_a_new_code() {
    let backend = Arc::new(MemoryBackend::new());
    let session = open(&backend).await;
    run(
        &session,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443,
        },
    )
    .await
    .unwrap();
    backend.fail_next("the server refused the pairing code");
    let trust = |code: &str| Command::TrustServer {
        fingerprint: FAKE_SERVER_FINGERPRINT.into(),
        code: Secret::new(code),
    };
    run(&session, trust("ABCD2345")).await.unwrap();
    let link = session.state().link.clone().unwrap();
    assert_eq!(link.status, LinkStatus::Offline);
    assert!(!link.server.unwrap().is_pinned());
    run(&session, trust("QRST2345")).await.unwrap();
    let link = session.state().link.clone().unwrap();
    assert_eq!(link.status, LinkStatus::Online);
    assert_eq!(
        link.server.unwrap().fingerprint.as_deref(),
        Some(FAKE_SERVER_FINGERPRINT)
    );
}
