//! Pinned threads (MISC-4) through a `Session` on the memory backend: the `$Pinned` keyword on
//! the fake server, and the device-only pin where it keeps no keywords.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{Command, CommandReceipt, Origin, ProviderPreset, Secret, ThreadId, listing};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};

async fn run(session: &Session, cmd: Command) -> CommandReceipt {
    let receipt = session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
    receipt
}

async fn synced(keywords: bool) -> (Arc<MemoryBackend>, Session) {
    let mut fixture = Fixture::builtin("basic").unwrap();
    for mailbox in &mut fixture.accounts[0].mailboxes {
        mailbox.keywords = keywords;
    }
    let backend = Arc::new(MemoryBackend::from_fixture(fixture));
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
    (backend, session)
}

/// The inbox's oldest thread.
fn oldest(session: &Session) -> ThreadId {
    let state = session.state();
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.path == "INBOX")
        .unwrap()
        .id;
    listing::threads_in(&state, inbox).last().unwrap().id
}

fn server_pins(backend: &MemoryBackend) -> usize {
    backend.server().accounts[0]
        .mailboxes
        .iter()
        .flat_map(|m| &m.messages)
        .filter(|m| m.flags.iter().any(|f| f == "$Pinned"))
        .count()
}

#[tokio::test(start_paused = true)]
async fn the_pin_is_a_keyword_on_the_server() {
    let (backend, session) = synced(true).await;
    let thread = oldest(&session);
    run(
        &session,
        Command::PinThreads {
            ids: vec![thread],
            messages: Vec::new(),
        },
    )
    .await;
    assert_eq!(server_pins(&backend), 1);
    // Another sync reads it back: still first.
    run(&session, Command::SyncNow { account: None }).await;
    let state = session.state();
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.path == "INBOX")
        .unwrap()
        .id;
    assert_eq!(listing::threads_in(&state, inbox)[0].id, thread);
}

#[tokio::test(start_paused = true)]
async fn without_keywords_the_pin_stays_on_the_device() {
    let (backend, session) = synced(false).await;
    let thread = oldest(&session);
    run(
        &session,
        Command::PinThreads {
            ids: vec![thread],
            messages: Vec::new(),
        },
    )
    .await;
    assert_eq!(server_pins(&backend), 0);
    let state = session.state();
    assert!(
        state.outbox.is_empty(),
        "nothing waits: the store was skipped"
    );
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.path == "INBOX")
        .unwrap();
    assert_eq!(inbox.custom_keywords, Some(false));
    run(&session, Command::SyncNow { account: None }).await;
    let state = session.state();
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.path == "INBOX")
        .unwrap()
        .id;
    assert_eq!(
        listing::threads_in(&state, inbox)[0].id,
        thread,
        "a sync keeps the device's pin"
    );
}
