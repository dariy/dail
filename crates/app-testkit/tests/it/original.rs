//! "Open original" through a `Session` on the memory backend.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{BodyState, Command, Origin, ProviderPreset, Secret};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};

async fn run(session: &Session, cmd: Command) {
    session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn the_original_is_a_private_sanitized_page() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = Session::open(
        Arc::clone(&backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();
    run(
        &session,
        Command::AddAccount {
            preset: ProviderPreset::Hostinger,
            email: "alice@example.com".into(),
            display_name: None,
            password: Some(Secret::new("pw")),
            imap: None,
            smtp: None,
        },
    )
    .await;
    run(&session, Command::SyncNow { account: None }).await;
    let thread = *session.state().threads.keys().next().unwrap();
    run(&session, Command::OpenThread { id: thread }).await;
    let state = session.state();
    let blob = state
        .messages
        .values()
        .find_map(|m| match &m.body {
            BodyState::Fetched { blob } => Some(blob.clone()),
            _ => None,
        })
        .expect("a downloaded body");
    let path = session.open_original(&blob).await.unwrap().unwrap();
    let page = std::fs::read_to_string(&path).unwrap();
    assert!(page.contains("default-src 'none'"));
    assert!(page.contains("alice@example.com"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "only the user can read it");
    }
    std::fs::remove_file(&path).unwrap();
    let missing = "0".repeat(64);
    assert!(session.open_original(&missing).await.unwrap().is_none());
}
