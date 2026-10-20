//! Search through a `Session` on the memory backend: local results first, then the server's
//! hits (outside the synced window, or matching only in the body) merged into the cache.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{Command, CommandReceipt, Origin, ProviderPreset, Secret, SettingKey};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};
use serde_json::Value;

async fn run(session: &Session, cmd: Command) -> CommandReceipt {
    let receipt = session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
    receipt
}

/// alice@example.com on the `basic` fixture, synced with a window of one message: only the
/// newest INBOX message ("Lunch?") is cached, not "Q3 numbers".
async fn synced() -> (Arc<MemoryBackend>, Session) {
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
        Command::SetSetting {
            key: SettingKey::InitialWindow,
            value: Value::from(1_u64),
        },
    )
    .await;
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
    assert_eq!(session.state().messages.len(), 1);
    (backend, session)
}

fn search(query: &str) -> Command {
    Command::Search {
        query: query.into(),
        account: None,
        offset: 0,
        limit: 50,
        server: true,
        order: app_core::SearchOrder::default(),
    }
}

fn subjects(receipt: &CommandReceipt) -> Vec<String> {
    receipt.data["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            t["untrusted_content"]["subject"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn server_hits_outside_the_window_and_in_bodies_are_merged() {
    let (backend, session) = synced().await;
    // "attached" is only in the body of "Q3 numbers", which is outside the window.
    let first = run(&session, search("attached")).await;
    assert!(subjects(&first).is_empty(), "the cache knows nothing yet");
    assert_eq!(first.data["server"][0]["state"], "started");
    // After the server answered (`run` waits for the effects), the hit is cached and found.
    let again = run(&session, search("attached")).await;
    assert_eq!(subjects(&again), ["Q3 numbers"]);
    assert_eq!(again.data["server"][0]["state"], "done");
    assert_eq!(again.data["server"][0]["hits"], 1);
    let state = session.state();
    assert_eq!(state.messages.len(), 2);
    assert!(
        state
            .messages
            .values()
            .any(|m| m.subject == "Q3 numbers" && m.uid == 1)
    );
    // INBOX, Sent, Drafts and Archive were searched (nothing has the `all` role).
    let searched: Vec<String> = backend
        .executed()
        .iter()
        .filter_map(|e| match e {
            app_core::Effect::SearchMailbox {
                path, gmail_raw, ..
            } => {
                assert!(!gmail_raw);
                Some(path.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(searched, ["INBOX", "Sent", "Drafts", "Archive"]);
    // The merged message was persisted like a fetched one.
    let reopened = backend.store().snapshot().unwrap();
    assert_eq!(reopened.messages.len(), 2);
}

#[tokio::test(start_paused = true)]
async fn set_search_tracks_the_server_round_and_failures() {
    let (backend, session) = synced().await;
    let shown = run(
        &session,
        Command::SetSearch {
            query: "from:bob".into(),
        },
    )
    .await;
    assert!(subjects(&shown).is_empty());
    assert_eq!(shown.data["server"][0]["state"], "searching");
    let state = session.state();
    assert_eq!(state.view.search.as_deref(), Some("from:bob"));
    let entry = &state.status.searches[0];
    assert_eq!((entry.pending, entry.hits.len()), (0, 1));
    let found = run(&session, search("from:bob")).await;
    assert_eq!(subjects(&found), ["Q3 numbers"]);

    // Offline: every mailbox search fails, and the failure is in state.
    backend.set_offline(true);
    run(
        &session,
        Command::SetSearch {
            query: "lunch".into(),
        },
    )
    .await;
    let state = session.state();
    let entry = state.status.searches.last().unwrap();
    assert_eq!(entry.query, "lunch");
    assert_eq!(entry.pending, 0);
    assert!(
        entry
            .last_error
            .as_deref()
            .unwrap()
            .contains("network is unreachable")
    );
    // The cached message is still found.
    let found = run(&session, search("lunch")).await;
    assert_eq!(subjects(&found), ["Lunch?"]);
}

#[tokio::test(start_paused = true)]
async fn words_of_downloaded_bodies_are_found_offline() {
    let (backend, session) = synced().await;
    let q3 = session
        .state()
        .threads
        .values()
        .find(|t| t.subject == "Lunch?")
        .unwrap()
        .id;
    // Download the body ("Lunch at noon?"), then go offline.
    session
        .submit(Command::OpenThread { id: q3 }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    backend.set_offline(true);
    let search = |session: &Session| {
        let session = session.client();
        async move {
            session
                .submit(
                    Command::Search {
                        query: "noon".into(),
                        account: None,
                        offset: 0,
                        limit: 50,
                        server: false,
                        order: app_core::SearchOrder::default(),
                    },
                    Origin::Cli,
                )
                .await
                .unwrap()
                .unwrap()
        }
    };
    // The first search asks the index; its hits are kept for the query.
    let first = search(&session).await;
    assert_eq!(first.data["total"], 0, "no header says it");
    session.flush().await.unwrap();
    let second = search(&session).await;
    assert_eq!(second.data["total"], 1);
    assert_eq!(second.data["threads"][0]["id"], serde_json::json!(q3));
    session.shutdown().await.unwrap();
}
