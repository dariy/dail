//! Settings on the server (ADR 0015, SET-2) through a `Session` on the memory backend: the fake
//! server's `dail/Settings` folder, push, pull on the next listing, and the hidden folder.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{Command, Origin, ProviderPreset, Secret, Stamped, SyncedSettings};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};

async fn run(session: &Session, cmd: Command, origin: Origin) -> Result<(), app_core::CoreError> {
    let result = session.submit(cmd, origin).await.unwrap().map(|_| ());
    session.flush().await.unwrap();
    result
}

async fn open(backend: &Arc<MemoryBackend>) -> Session {
    let session = Session::open(
        Arc::clone(backend) as Arc<dyn Backend>,
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
        Origin::Cli,
    )
    .await
    .unwrap();
    run(&session, Command::SyncNow { account: None }, Origin::Cli)
        .await
        .unwrap();
    session
}

fn settings_messages(backend: &MemoryBackend) -> Option<usize> {
    backend
        .server()
        .mailbox("alice@example.com", "dail/Settings")
        .map(|m| m.messages.len())
}

#[tokio::test(start_paused = true)]
async fn push_creates_the_folder_and_keeps_one_copy() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = open(&backend).await;
    // No folder yet: the listing counted as a pull.
    assert!(session.state().synced.pulled_ms.is_some());
    assert_eq!(settings_messages(&backend), None);
    assert!(matches!(
        run(&session, Command::PushSettings {}, Origin::Mcp).await,
        Err(app_core::CoreError::OriginNotAllowed { .. })
    ));
    run(&session, Command::PushSettings {}, Origin::Cli)
        .await
        .unwrap();
    assert_eq!(settings_messages(&backend), Some(1));
    run(&session, Command::PushSettings {}, Origin::Cli)
        .await
        .unwrap();
    assert_eq!(
        settings_messages(&backend),
        Some(1),
        "older copies are expunged"
    );
    let synced = session.state().synced.clone();
    assert!(synced.pushed_ms.is_some());
    assert!(synced.copy.is_some(), "the write is read back");
    assert_eq!(synced.error, None);

    // The folder never becomes a mailbox.
    run(&session, Command::SyncNow { account: None }, Origin::Cli)
        .await
        .unwrap();
    assert!(
        session
            .state()
            .mailboxes
            .values()
            .all(|m| !m.path.starts_with("dail"))
    );
}

#[tokio::test(start_paused = true)]
async fn another_devices_copy_is_pulled_on_listing_and_a_bad_one_is_ignored() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let mut doc = SyncedSettings::new();
    doc.prefs.insert(
        "list_density".into(),
        Stamped {
            updated_ms: 5,
            value: 1.into(),
        },
    );
    let json = String::from_utf8(doc.to_json().unwrap()).unwrap();
    let raw = app_io::settings_blob::render(&json, 0).unwrap();
    backend.edit_server(|server| {
        let account = server
            .accounts
            .iter_mut()
            .find(|a| a.email == "alice@example.com")
            .unwrap();
        let mut folder = app_testkit::fixture::FakeMailbox::new("dail/Settings");
        folder.append(vec![], String::from_utf8(raw).unwrap());
        account.mailboxes.push(folder);
    });
    let session = open(&backend).await;
    let synced = session.state().synced.clone();
    assert_eq!(synced.settings.prefs["list_density"].value, 1);
    assert_eq!(synced.error, None);

    backend.edit_server(|server| {
        let account = server
            .accounts
            .iter_mut()
            .find(|a| a.email == "alice@example.com")
            .unwrap();
        let folder = account
            .mailboxes
            .iter_mut()
            .find(|m| m.path == "dail/Settings")
            .unwrap();
        folder.append(vec![], "Subject: x\r\n\r\nnot settings\r\n".into());
    });
    run(
        &session,
        Command::PullSettings { account: None },
        Origin::Mcp,
    )
    .await
    .unwrap();
    let synced = session.state().synced.clone();
    assert_eq!(synced.settings.prefs["list_density"].value, 1);
    assert!(synced.error.unwrap().error.starts_with("ERR021: "));
}
