//! Updates end to end against the fake update server (Wave 7 S5): check, download and install
//! through a session; a tampered manifest, signature or artifact ends in `update_failed`; agents
//! cannot install.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{Command, CoreError, Event, Origin, UpdateStatus};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::MemoryBackend;
use app_testkit::updates::{VERSION, host};

async fn open(backend: &Arc<MemoryBackend>) -> Session {
    let session = Session::open(
        Arc::clone(backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();
    session
        .ingest(Event::UpdateHostDetected { host: host() })
        .await
        .unwrap();
    session
}

async fn run(session: &Session, cmd: Command, origin: Origin) -> Result<UpdateStatus, CoreError> {
    let result = session.submit(cmd, origin).await.unwrap();
    session.flush().await.unwrap();
    result.map(|_| session.state().update.status.clone())
}

#[tokio::test(start_paused = true)]
async fn a_build_updates_itself_end_to_end() {
    let backend = Arc::new(MemoryBackend::new());
    let session = open(&backend).await;
    let status = run(&session, Command::CheckForUpdates {}, Origin::Cli)
        .await
        .unwrap();
    assert!(
        matches!(status, UpdateStatus::Available { .. }),
        "{status:?}"
    );
    let status = run(&session, Command::DownloadUpdate {}, Origin::Mcp)
        .await
        .unwrap();
    assert!(matches!(status, UpdateStatus::Ready { .. }), "{status:?}");
    let refused = run(
        &session,
        Command::InstallUpdate { confirm: true },
        Origin::Mcp,
    )
    .await;
    assert!(
        matches!(refused, Err(CoreError::OriginNotAllowed { .. })),
        "{refused:?}"
    );
    let status = run(
        &session,
        Command::InstallUpdate { confirm: true },
        Origin::Cli,
    )
    .await
    .unwrap();
    assert_eq!(
        status,
        UpdateStatus::Installed {
            version: VERSION.to_owned()
        }
    );
    assert_eq!(backend.updates().installed, [VERSION]);
}

#[tokio::test(start_paused = true)]
async fn tampering_fails_with_update_failed() {
    for tamper in ["manifest", "signature", "artifact"] {
        let backend = Arc::new(MemoryBackend::new());
        match tamper {
            "manifest" => backend.updates().tamper_manifest(),
            "signature" => backend.updates().tamper_signature(),
            _ => backend.updates().tamper_artifact(),
        }
        let session = open(&backend).await;
        let mut status = run(&session, Command::CheckForUpdates {}, Origin::Cli)
            .await
            .unwrap();
        if tamper == "artifact" {
            assert!(
                matches!(status, UpdateStatus::Available { .. }),
                "{status:?}"
            );
            status = run(&session, Command::DownloadUpdate {}, Origin::Cli)
                .await
                .unwrap();
        }
        assert!(
            matches!(status, UpdateStatus::Failed { .. }),
            "{tamper}: {status:?}"
        );
        assert!(backend.updates().installed.is_empty());
    }
}
