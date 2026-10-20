//! Test-kit self tests: store, backend, fixtures, and a Session running on the memory backend.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;
use std::sync::Arc;

use app_core::{
    AccountId, AccountStatus, AppState, BodyState, Command, Effect, Endpoint, Event, ImapLogin,
    MailboxId, MailboxRole, Origin, PersistDelta, ProviderPreset, Secret, SecretRef, Security,
    SettingKey, Settings,
};
use app_io::Store;
use app_sync::{Backend, CounterSeeds, Session, SyncError, VirtualClock};
use app_testkit::{FakeMessage, Fixture, MemoryBackend, MemoryStore, TestkitError};
use serde_json::json;

fn persist(poll_secs: u64) -> Effect {
    Effect::PersistState {
        delta: PersistDelta::Settings(Settings {
            poll_secs,
            ..Settings::default()
        }),
    }
}

#[test]
fn memory_store_applies_deltas() {
    let store = MemoryStore::default();
    assert_eq!(store.load_snapshot().unwrap(), None);
    store
        .apply(&PersistDelta::Settings(Settings {
            poll_secs: 42,
            ..Settings::default()
        }))
        .unwrap();
    assert_eq!(store.snapshot().unwrap().settings.poll_secs, 42);
}

#[tokio::test]
async fn backend_persists_and_logs() {
    let backend = MemoryBackend::new();
    let events = backend.execute(persist(77)).await.unwrap();
    assert_eq!(events, vec![Event::Persisted {}]);
    assert_eq!(backend.executed(), vec![persist(77)]);
    let loaded = backend.load_state().await.unwrap().unwrap();
    assert_eq!(loaded.settings.poll_secs, 77);
}

#[tokio::test]
async fn backend_fault_injection() {
    let backend = MemoryBackend::new();
    backend.fail_next("server on fire");
    let err = backend.execute(persist(1)).await.unwrap_err();
    assert!(matches!(err, SyncError::Effect { ref reason, .. } if reason == "server on fire"));
    assert_eq!(
        backend.store().snapshot(),
        None,
        "failed effect has no effect"
    );
    assert!(
        backend.execute(persist(2)).await.is_ok(),
        "faults are one-shot"
    );
}

#[tokio::test]
async fn backend_rejects_timer_effects() {
    let backend = MemoryBackend::new();
    let err = backend
        .execute(Effect::ScheduleTick { at_ms: 1 })
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::Unsupported { .. }));
}

#[tokio::test]
async fn backend_starts_from_store_state() {
    let mut state = AppState::default();
    state.settings.history_cap = 9;
    let backend = MemoryBackend::new().with_store(MemoryStore::with_state(state.clone()));
    assert_eq!(backend.load_state().await.unwrap(), Some(state));
}

#[test]
fn builtin_basic_fixture_loads() {
    assert!(Fixture::builtin_names().contains(&"basic"));
    let fixture = Fixture::builtin("basic").unwrap();
    let inbox = fixture.mailbox("alice@example.com", "INBOX").unwrap();
    assert_eq!(inbox.special_use.as_deref(), Some("inbox"));
    assert_eq!(inbox.messages.len(), 2);
    let backend = MemoryBackend::from_fixture(fixture.clone());
    assert_eq!(backend.server(), fixture);
}

#[test]
fn unknown_fixture_is_an_error() {
    let err = Fixture::builtin("nope").unwrap_err();
    assert!(matches!(err, TestkitError::UnknownFixture { .. }));
    assert!(err.to_string().contains("basic"));
}

#[test]
fn fixture_read_and_parse_errors() {
    let err = Fixture::from_path(Path::new("/definitely/missing.json")).unwrap_err();
    assert!(matches!(err, TestkitError::Read { .. }));
    let err = Fixture::from_json("{ not json").unwrap_err();
    assert!(matches!(err, TestkitError::Parse(_)));
}

#[test]
fn fixture_validation_rejects_duplicate_uids() {
    let json = json!({"accounts": [{"email": "a@x", "mailboxes": [{
        "path": "INBOX", "uid_validity": 1,
        "messages": [{"uid": 2, "raw": ""}, {"uid": 2, "raw": ""}]
    }]}]});
    let err = Fixture::from_json(&json.to_string()).unwrap_err();
    assert!(matches!(err, TestkitError::Invalid(_)));
    let json = json!({"accounts": [{"email": "a@x"}, {"email": "a@x"}]});
    assert!(Fixture::from_json(&json.to_string()).is_err());
    let json = json!({"accounts": [{"email": "a@x", "mailboxes": [
        {"path": "INBOX", "uid_validity": 1}, {"path": "INBOX", "uid_validity": 1}
    ]}]});
    assert!(Fixture::from_json(&json.to_string()).is_err());
}

#[tokio::test(start_paused = true)]
async fn session_on_memory_backend_persists_and_surfaces_failures() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = Session::open(
        Arc::clone(&backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(0)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();
    let set = Command::SetSetting {
        key: SettingKey::PollSecs,
        value: json!(600),
    };
    session.submit(set, Origin::Batch).await.unwrap().unwrap();
    session.flush().await.unwrap();
    assert_eq!(backend.store().snapshot().unwrap().settings.poll_secs, 600);

    backend.fail_next("disk full");
    session
        .submit(Command::Undo {}, Origin::Batch)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    let error = session.state().status.persist_error.clone().unwrap();
    assert!(error.contains("disk full"));
    assert_eq!(backend.store().snapshot().unwrap().settings.poll_secs, 600);
    session.shutdown().await.unwrap();
}

async fn open(backend: &Arc<MemoryBackend>) -> Session {
    Session::open(
        Arc::clone(backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(0)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

fn add_alice(password: Option<&str>) -> Command {
    Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email: "alice@example.com".into(),
        display_name: None,
        password: password.map(Secret::new),
        imap: None,
        smtp: None,
    }
}

#[tokio::test(start_paused = true)]
async fn account_lifecycle_on_memory_backend() {
    let fixture = json!({"accounts": [{"email": "alice@example.com", "password": "s3cret"}]});
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::from_json(&fixture.to_string()).unwrap(),
    ));
    let session = open(&backend).await;

    // Wrong password: stored in the keyring, but the fake server rejects it.
    let receipt = session
        .submit(add_alice(Some("wrong")), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let id: AccountId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    let secret = SecretRef::for_account(id);
    session.flush().await.unwrap();
    assert!(backend.has_secret(&secret));
    assert!(
        !format!("{backend:?}").contains("wrong"),
        "Debug leaks secret"
    );
    assert_eq!(
        backend.store().snapshot().unwrap().accounts[&id].email,
        "alice@example.com"
    );
    session
        .submit(Command::TestAccount { id }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    assert!(matches!(
        &session.state().accounts[&id].status,
        AccountStatus::Failed { error, .. } if error.contains("invalid credentials")
    ));

    // Undo removes the account and its secret; removal is persisted.
    session
        .submit(Command::Undo {}, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    assert!(!backend.has_secret(&secret));
    assert!(backend.store().snapshot().unwrap().accounts.is_empty());

    // Right password verifies.
    let receipt = session
        .submit(add_alice(Some("s3cret")), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let id: AccountId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    session
        .submit(Command::TestAccount { id }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    assert!(matches!(
        session.state().accounts[&id].status,
        AccountStatus::Verified { .. }
    ));
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn verify_fails_without_password_or_unknown_user() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = open(&backend).await;
    let receipt = session
        .submit(add_alice(None), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let id: AccountId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    session
        .submit(Command::TestAccount { id }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    assert!(matches!(
        &session.state().accounts[&id].status,
        AccountStatus::Failed { error, .. } if error.contains("no password")
    ));

    let unknown = Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "nobody@gmail.com".into(),
        display_name: None,
        password: Some(Secret::new("x")),
        imap: None,
        smtp: None,
    };
    let receipt = session.submit(unknown, Origin::Cli).await.unwrap().unwrap();
    let id: AccountId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    session
        .submit(Command::TestAccount { id }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    assert!(matches!(
        &session.state().accounts[&id].status,
        AccountStatus::Failed { error, .. } if error.contains("unknown user")
    ));
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn backend_lists_fixture_mailboxes_after_checking_the_login() {
    let backend = MemoryBackend::from_fixture(Fixture::builtin("basic").unwrap());
    let account =
        serde_json::from_value::<AccountId>(json!("00000000-0000-0000-0000-000000000001")).unwrap();
    let secret = SecretRef::for_account(account);
    let list = Effect::ListMailboxes {
        account,
        imap: ImapLogin {
            login: "alice@example.com".into(),
            server: Endpoint {
                host: "imap.example.com".into(),
                port: 993,
                security: Security::Tls,
            },
            secret: secret.clone(),
        },
    };
    let events = backend.execute(list.clone()).await.unwrap();
    assert!(
        matches!(&events[..], [Event::SyncFailed { error, .. }] if error.contains("no password")),
        "{events:?}"
    );

    backend
        .execute(Effect::StoreSecret {
            account,
            secret,
            value: Secret::new("anything"),
        })
        .await
        .unwrap();
    let events = backend.execute(list).await.unwrap();
    let [Event::MailboxesListed { mailboxes, .. }] = &events[..] else {
        panic!("unexpected events {events:?}");
    };
    let listed: Vec<_> = mailboxes
        .iter()
        .map(|m| (m.path.as_str(), m.special_use))
        .collect();
    assert_eq!(
        listed,
        [
            ("INBOX", Some(MailboxRole::Inbox)),
            ("Sent", Some(MailboxRole::Sent)),
            ("Drafts", Some(MailboxRole::Drafts)),
            ("Archive", Some(MailboxRole::Archive)),
        ]
    );
}

#[tokio::test]
async fn backend_fetches_fixture_envelopes_with_window_rules() {
    let backend = MemoryBackend::from_fixture(Fixture::builtin("basic").unwrap());
    let account =
        serde_json::from_value::<AccountId>(json!("00000000-0000-0000-0000-000000000001")).unwrap();
    let mailbox =
        serde_json::from_value::<MailboxId>(json!("00000000-0000-0000-0000-000000000002")).unwrap();
    let secret = SecretRef::for_account(account);
    backend
        .execute(Effect::StoreSecret {
            account,
            secret: secret.clone(),
            value: Secret::new("anything"),
        })
        .await
        .unwrap();
    let fetch = |uid_validity, since_uid, limit| Effect::FetchEnvelopes {
        account,
        mailbox,
        path: "INBOX".into(),
        imap: ImapLogin {
            login: "alice@example.com".into(),
            server: Endpoint {
                host: "imap.example.com".into(),
                port: 993,
                security: Security::Tls,
            },
            secret: secret.clone(),
        },
        uid_validity,
        since_uid,
        limit,
    };
    let events = backend.execute(fetch(None, None, 1)).await.unwrap();
    let [
        Event::EnvelopesFetched {
            envelopes,
            total,
            unread,
            uid_next,
            ..
        },
    ] = &events[..]
    else {
        panic!("unexpected events {events:?}");
    };
    assert_eq!((*total, *unread, *uid_next), (2, 1, Some(3)));
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0].subject, "Lunch?");
    assert_eq!(envelopes[0].from[0].email, "carol@example.org");

    let events = backend.execute(fetch(Some(1), Some(3), 10)).await.unwrap();
    assert!(
        matches!(&events[..], [Event::EnvelopesFetched { envelopes, .. }] if envelopes.is_empty())
    );
    let events = backend.execute(fetch(Some(9), Some(3), 10)).await.unwrap();
    assert!(
        matches!(&events[..], [Event::EnvelopesFetched { envelopes, .. }] if envelopes.len() == 2 && envelopes[0].flags.seen)
    );
}

#[tokio::test(start_paused = true)]
async fn sync_now_fills_the_cache_and_polls_inbox() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = open(&backend).await;
    let added = session
        .submit(add_alice(Some("pw")), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let account: AccountId = serde_json::from_value(added.data["id"].clone()).unwrap();
    session
        .submit(Command::SyncNow { account: None }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();

    let state = session.state();
    let roles: Vec<_> = state.mailboxes.values().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [
            Some(MailboxRole::Inbox),
            Some(MailboxRole::Sent),
            Some(MailboxRole::Drafts),
            Some(MailboxRole::Archive)
        ]
    );
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.threads.len(), 2);
    let status = &state.status.sync[&account];
    assert_eq!((status.pending, status.last_error.clone()), (0, None));
    assert!(status.last_sync_ms.is_some());
    let persisted = backend.store().snapshot().unwrap();
    assert_eq!(persisted.messages.len(), 2, "the cache is persisted");

    let inbox = *state.mailboxes.keys().next().unwrap();
    let listed = session
        .submit(
            Command::ListThreads {
                mailbox: inbox,
                offset: 0,
                limit: 10,
            },
            Origin::Mcp,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        listed.data["threads"][0]["untrusted_content"]["subject"],
        "Lunch?"
    );

    // The poll timer fires after poll_secs and fetches INBOX incrementally.
    let fetches = |backend: &MemoryBackend| {
        backend
            .executed()
            .iter()
            .filter(|e| e.name() == "fetch_envelopes")
            .count()
    };
    let before = fetches(&backend);
    tokio::time::sleep(std::time::Duration::from_secs(121)).await;
    session.flush().await.unwrap();
    assert_eq!(fetches(&backend), before + 1);
    let last = backend
        .executed()
        .into_iter()
        .rev()
        .find(|e| e.name() == "fetch_envelopes");
    assert!(matches!(
        last,
        Some(Effect::FetchEnvelopes { since_uid: Some(3), ref path, .. }) if path == "INBOX"
    ));
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn sync_prefetches_unread_bodies_and_resyncs_flags() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = open(&backend).await;
    session
        .submit(add_alice(Some("pw")), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let sync = || Command::SyncNow { account: None };
    session.submit(sync(), Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();

    let state = session.state();
    let unread = state
        .messages
        .values()
        .find(|m| m.subject == "Lunch?")
        .unwrap();
    let BodyState::Fetched { blob } = &unread.body else {
        panic!("unread INBOX body not prefetched: {:?}", unread.body);
    };
    let raw = backend.store().get_blob(blob).unwrap().unwrap();
    assert!(String::from_utf8(raw).unwrap().contains("Lunch at noon?"));
    let read = state
        .messages
        .values()
        .find(|m| m.subject == "Q3 numbers")
        .unwrap();
    assert_eq!(read.body, BodyState::NotFetched, "read mail stays lazy");
    assert_eq!(backend.store().blob_count(), 1);

    // Elsewhere: Lunch? gets read and Q3 numbers deleted; new mail arrives.
    backend.edit_server(|server| {
        let inbox = &mut server.accounts[0].mailboxes[0];
        inbox.messages.retain(|m| m.uid != 1);
        inbox.messages[0].flags = vec!["\\Seen".into()];
        inbox.messages.push(FakeMessage {
            uid: 3,
            flags: Vec::new(),
            raw: "Message-ID: <new@x>\r\nSubject: New\r\n\r\nHi\r\n".into(),
        });
    });
    session.submit(sync(), Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
    let state = session.state();
    let subjects: Vec<_> = state
        .messages
        .values()
        .map(|m| m.subject.as_str())
        .collect();
    assert_eq!(subjects, ["Lunch?", "New"]);
    assert!(
        state
            .messages
            .values()
            .find(|m| m.subject == "Lunch?")
            .unwrap()
            .flags
            .seen
    );
    assert_eq!(
        backend.store().blob_count(),
        2,
        "the new unread mail was prefetched"
    );
    let pending = state.status.sync.values().next().unwrap().pending;
    assert_eq!(pending, 0);
    session.shutdown().await.unwrap();
}
