//! `LiveBackend` with an in-test keyring and verifier, and `NetVerifier` failure paths against
//! loopback fake servers (no external network).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::support::{FakeFolder, FakeImap};
use app_core::{
    AccountId, AccountStatus, Command, Effect, Endpoint, Event, ImapLogin, MailboxId, MailboxRole,
    Origin, ProviderPreset, Secret, SecretRef, Security,
};
use app_io::{Config, ConfigStore, ProfileDirs, ProfileStore, Store};
use app_sync::{
    AccountVerifier, Backend, BoxFuture, CounterSeeds, LiveBackend, NetVerifier, SecretStore,
    Session, SmtpSender, SyncError, SystemClock,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[derive(Default)]
struct TestSecrets {
    entries: Mutex<BTreeMap<SecretRef, Secret>>,
    broken: bool,
}

impl SecretStore for TestSecrets {
    fn set(&self, secret: &SecretRef, value: &Secret) -> Result<(), SyncError> {
        if self.broken {
            return Err(SyncError::Keyring("no secret service".into()));
        }
        self.entries
            .lock()
            .unwrap()
            .insert(secret.clone(), value.clone());
        Ok(())
    }

    fn get(&self, secret: &SecretRef) -> Result<Option<Secret>, SyncError> {
        Ok(self.entries.lock().unwrap().get(secret).cloned())
    }

    fn delete(&self, secret: &SecretRef) -> Result<(), SyncError> {
        self.entries.lock().unwrap().remove(secret);
        Ok(())
    }
}

/// Accepts exactly one password and records the logins it was asked to check.
struct TestVerifier {
    password: &'static str,
    calls: Mutex<Vec<(String, String)>>,
}

impl AccountVerifier for TestVerifier {
    fn verify<'a>(
        &'a self,
        login: &'a str,
        password: &'a Secret,
        imap: &'a Endpoint,
        _smtp: &'a Endpoint,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap()
                .push((login.to_owned(), imap.host.clone()));
            if password.expose() == self.password {
                Ok(())
            } else {
                Err("IMAP LOGIN failed: invalid credentials".to_owned())
            }
        })
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-sync-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn verifier() -> Arc<TestVerifier> {
    Arc::new(TestVerifier {
        password: "app-password",
        calls: Mutex::new(Vec::new()),
    })
}

fn add(password: &str) -> Command {
    Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "me@gmail.com".into(),
        display_name: None,
        password: Some(Secret::new(password)),
        imap: None,
        smtp: None,
    }
}

#[tokio::test]
async fn session_on_live_backend_persists_config_and_verifies() {
    let dir = scratch("session");
    let config_path = dir.join("config.toml");
    let secrets = Arc::new(TestSecrets::default());
    let verifier = verifier();
    let backend = LiveBackend::new(
        Arc::new(ConfigStore::open(&config_path).unwrap()),
        Arc::clone(&secrets) as Arc<dyn SecretStore>,
        Arc::clone(&verifier) as Arc<dyn AccountVerifier>,
        Arc::new(FakeImap::default()),
    );
    let session = Session::open(
        Arc::new(backend),
        Arc::new(SystemClock),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();

    let receipt = session
        .submit(add("app-password"), Origin::Cli)
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
    assert_eq!(
        verifier.calls.lock().unwrap().as_slice(),
        [("me@gmail.com".to_owned(), "imap.gmail.com".to_owned())]
    );
    let text = std::fs::read_to_string(&config_path).unwrap();
    assert!(text.contains("me@gmail.com"));
    assert!(!text.contains("app-password"), "secret written to config");
    assert_eq!(
        Config::load(&config_path).unwrap().unwrap().accounts.len(),
        1
    );

    session
        .submit(Command::RemoveAccount { id, confirm: true }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.shutdown().await.unwrap();
    assert!(secrets.entries.lock().unwrap().is_empty());
    assert!(
        Config::load(&config_path)
            .unwrap()
            .unwrap()
            .accounts
            .is_empty()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn verify_without_password_or_with_wrong_one_fails_in_state() {
    let dir = scratch("verify");
    let secrets = Arc::new(TestSecrets::default());
    let backend = LiveBackend::new(
        Arc::new(ConfigStore::open(dir.join("config.toml")).unwrap()),
        Arc::clone(&secrets) as Arc<dyn SecretStore>,
        verifier(),
        Arc::new(FakeImap::default()),
    );
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(1));
    let secret = SecretRef::for_account(id);
    let endpoint = Endpoint {
        host: "imap.example.org".into(),
        port: 993,
        security: Security::Tls,
    };
    let verify = Effect::VerifyAccount {
        account: id,
        login: "me@example.org".into(),
        imap: endpoint.clone(),
        smtp: endpoint,
        secret: secret.clone(),
    };

    let events = backend.execute(verify.clone()).await.unwrap();
    assert!(matches!(
        &events[..],
        [Event::AccountVerifyFailed { error, .. }] if error.contains("no password stored")
    ));

    let events = backend
        .execute(Effect::StoreSecret {
            account: id,
            secret: secret.clone(),
            value: Secret::new("wrong"),
        })
        .await
        .unwrap();
    assert_eq!(events, vec![Event::SecretStored { account: id }]);
    let events = backend.execute(verify).await.unwrap();
    assert!(matches!(
        &events[..],
        [Event::AccountVerifyFailed { error, .. }] if error.contains("invalid credentials")
    ));

    let events = backend
        .execute(Effect::DeleteSecret {
            account: id,
            secret,
        })
        .await
        .unwrap();
    assert_eq!(events, vec![Event::SecretDeleted { account: id }]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn keyring_failure_is_an_effect_error() {
    let dir = scratch("keyring");
    let backend = LiveBackend::new(
        Arc::new(ConfigStore::open(dir.join("config.toml")).unwrap()),
        Arc::new(TestSecrets {
            broken: true,
            ..TestSecrets::default()
        }),
        verifier(),
        Arc::new(FakeImap::default()),
    );
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(1));
    let effect = Effect::StoreSecret {
        account: id,
        secret: SecretRef::for_account(id),
        value: Secret::new("x"),
    };
    let err = backend.execute(effect.clone()).await.unwrap_err();
    assert!(matches!(err, SyncError::Keyring(_)));
    // The executor turns it into an event that surfaces in state.
    assert!(matches!(
        effect.failure_event(err.to_string()),
        Event::SecretFailed { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

// --- NetVerifier against loopback fake servers -------------------------------------------------

fn local(port: u16, security: Security) -> Endpoint {
    Endpoint {
        host: "127.0.0.1".into(),
        port,
        security,
    }
}

fn net() -> NetVerifier {
    NetVerifier::new()
        .unwrap()
        .with_timeout(Duration::from_secs(5))
}

/// A port with nothing listening on it.
async fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

/// Serves one connection with `script`; returns everything the client sent.
async fn fake_server<F, Fut>(script: F) -> (u16, tokio::task::JoinHandle<String>)
where
    F: FnOnce(tokio::net::TcpStream) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = String> + Send,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        script(stream).await
    });
    (port, handle)
}

#[tokio::test]
async fn connection_refused_is_reported_per_protocol() {
    let password = Secret::new("pw");
    let port = closed_port().await;
    let err = net()
        .verify_imap(&local(port, Security::Tls), "me", &password)
        .await
        .unwrap_err();
    assert!(
        err.starts_with("IMAP 127.0.0.1:") && err.contains("connect failed"),
        "{err}"
    );
    let err = net()
        .verify_smtp(&local(port, Security::Tls), "me", &password)
        .await
        .unwrap_err();
    assert!(err.starts_with("SMTP 127.0.0.1:"), "{err}");

    let both = net()
        .verify(
            "me",
            &password,
            &local(port, Security::Tls),
            &local(port, Security::Tls),
        )
        .await
        .unwrap_err();
    assert!(both.contains("IMAP") && both.contains("SMTP"), "{both}");
    assert!(!both.contains("pw"));
}

#[tokio::test]
async fn plaintext_server_on_a_tls_port_fails_the_handshake() {
    let (port, server) = fake_server(|mut stream| async move {
        let _ = stream.write_all(b"* OK plaintext IMAP\r\n").await;
        let mut sent = Vec::new();
        let _ = tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut sent).await;
        String::from_utf8_lossy(&sent).into_owned()
    })
    .await;
    let err = net()
        .verify_imap(&local(port, Security::Tls), "me", &Secret::new("pw"))
        .await
        .unwrap_err();
    assert!(err.contains("TLS handshake failed"), "{err}");
    let sent = server.await.unwrap();
    // Only a TLS ClientHello went out. Its random bytes can spell `pw` (about 1 run in 400), so
    // look for the password quoted, as LOGIN sends it.
    assert!(sent.starts_with('\u{16}'), "a TLS handshake record");
    assert!(!sent.contains("LOGIN") && !sent.contains("\"pw\""));
}

#[tokio::test]
async fn imap_starttls_refusal_never_sends_credentials() {
    let (port, server) = fake_server(|stream| async move {
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(b"* OK fake IMAP ready\r\n").await.unwrap();
        let mut sent = String::new();
        if let Ok(Some(line)) = lines.next_line().await {
            let tag = line.split_whitespace().next().unwrap_or("*").to_owned();
            sent.push_str(&line);
            let reply = format!("{tag} NO STARTTLS disabled\r\n");
            write.write_all(reply.as_bytes()).await.unwrap();
        }
        while let Ok(Some(line)) = lines.next_line().await {
            sent.push_str(&line);
        }
        sent
    })
    .await;
    let err = net()
        .verify_imap(&local(port, Security::StartTls), "me", &Secret::new("pw"))
        .await
        .unwrap_err();
    assert!(err.contains("STARTTLS refused"), "{err}");
    let sent = server.await.unwrap();
    assert!(sent.contains("STARTTLS"));
    assert!(!sent.contains("LOGIN") && !sent.contains("pw"), "{sent}");
}

#[tokio::test]
async fn smtp_without_starttls_support_never_sends_credentials() {
    let (port, server) = fake_server(|stream| async move {
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(b"220 fake ESMTP\r\n").await.unwrap();
        let mut sent = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            sent.push_str(&line);
            sent.push('\n');
            let reply: &[u8] = if line.starts_with("EHLO") {
                b"250-fake\r\n250 AUTH PLAIN LOGIN\r\n"
            } else if line.starts_with("QUIT") {
                b"221 bye\r\n"
            } else {
                b"502 no\r\n"
            };
            if write.write_all(reply).await.is_err() {
                break;
            }
        }
        sent
    })
    .await;
    let err = net()
        .verify_smtp(&local(port, Security::StartTls), "me", &Secret::new("pw"))
        .await
        .unwrap_err();
    assert!(err.contains("STARTTLS failed"), "{err}");
    let sent = server.await.unwrap();
    assert!(!sent.contains("AUTH"), "{sent}");
}

#[tokio::test]
async fn silent_server_times_out() {
    let (port, _server) = fake_server(|stream| async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        drop(stream);
        String::new()
    })
    .await;
    let err = net()
        .with_timeout(Duration::from_millis(200))
        .verify_imap(&local(port, Security::StartTls), "me", &Secret::new("pw"))
        .await
        .unwrap_err();
    assert!(err.contains("timed out"), "{err}");
}

fn gmail_folders() -> Vec<FakeFolder> {
    vec![
        FakeFolder::new("INBOX", "\\HasNoChildren"),
        FakeFolder::new("[Gmail]", "\\HasChildren \\Noselect"),
        FakeFolder::new("[Gmail]/All Mail", "\\All \\HasNoChildren"),
        FakeFolder::new("[Gmail]/Sent Mail", "\\HasNoChildren \\Sent"),
        FakeFolder::new("[Gmail]/Spam", "\\HasNoChildren \\Junk"),
        FakeFolder::new("Receipts", "\\HasNoChildren"),
    ]
}

/// A backend whose keyring holds `password` for account 1, talking to `imap`.
fn mail_backend(dir: &std::path::Path, password: &str, imap: &FakeImap) -> (LiveBackend, Effect) {
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(1));
    let secret = SecretRef::for_account(id);
    let secrets = TestSecrets::default();
    secrets.set(&secret, &Secret::new(password)).unwrap();
    let backend = LiveBackend::new(
        Arc::new(ConfigStore::open(dir.join("config.toml")).unwrap()),
        Arc::new(secrets),
        verifier(),
        Arc::new(imap.clone()),
    );
    let effect = Effect::ListMailboxes {
        account: id,
        imap: ImapLogin {
            login: "me@gmail.com".into(),
            server: Endpoint {
                host: "imap.gmail.com".into(),
                port: 993,
                security: Security::Tls,
            },
            secret,
        },
    };
    (backend, effect)
}

#[tokio::test]
async fn list_mailboxes_reports_special_use_and_reuses_the_session() {
    let dir = scratch("list");
    let imap = FakeImap::new("app-password", gmail_folders());
    let (backend, list) = mail_backend(&dir, "app-password", &imap);

    let events = backend.execute(list.clone()).await.unwrap();
    let [Event::MailboxesListed { mailboxes, .. }] = &events[..] else {
        panic!("unexpected events {events:?}");
    };
    let summary: Vec<_> = mailboxes
        .iter()
        .map(|m| (m.path.as_str(), m.special_use, m.selectable))
        .collect();
    assert_eq!(
        summary,
        [
            ("INBOX", None, true),
            ("[Gmail]", None, false),
            ("[Gmail]/All Mail", Some(MailboxRole::All), true),
            ("[Gmail]/Sent Mail", Some(MailboxRole::Sent), true),
            ("[Gmail]/Spam", Some(MailboxRole::Junk), true),
            ("Receipts", None, true),
        ]
    );
    assert_eq!(mailboxes[0].delimiter.as_deref(), Some("/"));

    backend.execute(list.clone()).await.unwrap();
    assert_eq!(imap.connects(), 1, "the session is reused");

    imap.drop_connections();
    let events = backend.execute(list).await.unwrap();
    assert!(matches!(&events[..], [Event::MailboxesListed { .. }]));
    assert_eq!(imap.connects(), 2, "a dropped session is replaced");
    assert!(!imap.log().iter().any(|line| line.contains("app-password")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn list_mailboxes_with_a_wrong_password_is_a_sync_failure() {
    let dir = scratch("list-fail");
    let imap = FakeImap::new("app-password", gmail_folders());
    let (backend, list) = mail_backend(&dir, "wrong-password", &imap);
    let events = backend.execute(list).await.unwrap();
    let [Event::SyncFailed { error, mailbox, .. }] = &events[..] else {
        panic!("unexpected events {events:?}");
    };
    assert!(error.contains("IMAP LOGIN failed"), "{error}");
    assert!(!error.contains("wrong-password"));
    assert_eq!(*mailbox, None);
    std::fs::remove_dir_all(dir).unwrap();
}

fn fetch(path: &str, uid_validity: Option<u32>, since_uid: Option<u32>, limit: u32) -> Effect {
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(1));
    Effect::FetchEnvelopes {
        account: id,
        mailbox: MailboxId::from_uuid(uuid::Uuid::from_u128(2)),
        path: path.into(),
        imap: ImapLogin {
            login: "me@gmail.com".into(),
            server: Endpoint {
                host: "imap.gmail.com".into(),
                port: 993,
                security: Security::Tls,
            },
            secret: SecretRef::for_account(id),
        },
        uid_validity,
        since_uid,
        limit,
    }
}

#[tokio::test]
async fn fetch_envelopes_windows_increments_and_resets() {
    let dir = scratch("fetch");
    let mut inbox = FakeFolder::new("INBOX", "\\HasNoChildren")
        .with(3, "", "a@x", "Plan")
        .with(5, "\\Seen \\Flagged", "b@x", "Re: Plan")
        .with(9, "", "c@x", "Lunch?");
    inbox.uid_validity = 7;
    let imap = FakeImap::new("app-password", vec![inbox]);
    let (backend, _) = mail_backend(&dir, "app-password", &imap);

    // Initial window: the newest two of three.
    let events = backend
        .execute(fetch("INBOX", None, None, 2))
        .await
        .unwrap();
    let [
        Event::EnvelopesFetched {
            uid_validity,
            uid_next,
            total,
            unread,
            envelopes,
            mailbox,
            ..
        },
    ] = &events[..]
    else {
        panic!("unexpected events {events:?}");
    };
    assert_eq!(
        (*uid_validity, *uid_next, *total, *unread),
        (7, Some(10), 3, 2)
    );
    assert_eq!(mailbox.as_uuid().as_u128(), 2);
    let uids: Vec<u32> = envelopes.iter().map(|e| e.uid).collect();
    assert_eq!(uids, [5, 9]);
    assert_eq!(envelopes[0].subject, "Re: Plan");
    assert_eq!(envelopes[0].message_id_hdr.as_deref(), Some("b@x"));
    assert!(envelopes[0].flags.seen && envelopes[0].flags.flagged);
    assert_eq!(envelopes[0].from[0].email, "bob@example.org");
    assert_eq!(envelopes[0].date_ms, 1_699_954_860_000);
    assert!(envelopes[0].size > 0);

    // Incremental from UID 9.
    let events = backend
        .execute(fetch("INBOX", Some(7), Some(9), 2))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::EnvelopesFetched { envelopes, .. }] if envelopes.len() == 1 && envelopes[0].uid == 9)
    );

    // Nothing new: no FETCH round trip; `10:*` would return UID 9 again.
    let fetches_before = imap.log().iter().filter(|l| l.contains("FETCH")).count();
    let events = backend
        .execute(fetch("INBOX", Some(7), Some(10), 2))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::EnvelopesFetched { envelopes, .. }] if envelopes.is_empty())
    );
    let fetches_after = imap.log().iter().filter(|l| l.contains("FETCH")).count();
    assert_eq!(fetches_before, fetches_after);

    // UIDVALIDITY changed: the initial window again.
    let events = backend
        .execute(fetch("INBOX", Some(6), Some(9), 5))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::EnvelopesFetched { envelopes, uid_validity: 7, .. }] if envelopes.len() == 3)
    );

    // Unknown mailbox: a sync failure for that mailbox.
    let events = backend.execute(fetch("Nope", None, None, 5)).await.unwrap();
    assert!(
        matches!(&events[..], [Event::SyncFailed { mailbox: Some(_), error, .. }] if error.contains("EXAMINE Nope"))
    );
    assert_eq!(imap.connects(), 2, "a failed command discards the session");
    std::fs::remove_dir_all(dir).unwrap();
}

fn search(path: &str, query: &str, gmail_raw: bool, limit: u32) -> Effect {
    let Effect::FetchEnvelopes {
        account,
        mailbox,
        imap,
        ..
    } = fetch(path, None, None, 1)
    else {
        unreachable!()
    };
    let query = app_core::Query::parse(query).unwrap();
    Effect::SearchMailbox {
        account,
        mailbox,
        path: path.into(),
        imap,
        search: query.to_string(),
        started_ms: 5,
        mailboxes: 1,
        query,
        gmail_raw,
        limit,
    }
}

#[tokio::test]
async fn search_mailbox_translates_the_query_and_fetches_the_newest_hits() {
    let dir = scratch("search");
    let mut inbox = FakeFolder::new("INBOX", "\\HasNoChildren")
        .with(3, "", "a@x", "Plan")
        .with(5, "\\Seen", "b@x", "Re: Plan")
        .with(9, "", "c@x", "Lunch?");
    inbox.uid_validity = 7;
    let imap = FakeImap::new("app-password", vec![inbox]);
    let (backend, _) = mail_backend(&dir, "app-password", &imap);
    let hits = |events: &[Event]| -> Vec<u32> {
        let [
            Event::SearchResults {
                uid_validity: 7,
                search,
                started_ms: 5,
                mailboxes: 1,
                envelopes,
                ..
            },
        ] = events
        else {
            panic!("unexpected events {events:?}");
        };
        assert!(!search.is_empty());
        envelopes.iter().map(|e| e.uid).collect()
    };

    let events = backend
        .execute(search("INBOX", "plan -is:read", false, 10))
        .await
        .unwrap();
    assert_eq!(hits(&events), [3]);
    assert!(
        imap.log()
            .iter()
            .any(|l| l.contains(r#"UID SEARCH TEXT "plan" NOT SEEN"#)),
        "{:?}",
        imap.log()
    );

    // Bodies are searched; the envelope comes with it.
    let events = backend
        .execute(search("INBOX", r#""body of b@x""#, false, 10))
        .await
        .unwrap();
    assert_eq!(hits(&events), [5]);
    let Event::SearchResults { envelopes, .. } = &events[0] else {
        unreachable!()
    };
    assert_eq!(envelopes[0].subject, "Re: Plan");
    assert!(envelopes[0].flags.seen);

    // Only the newest `limit` hits are fetched, by UID set.
    let events = backend
        .execute(search("INBOX", "from:bob", false, 2))
        .await
        .unwrap();
    assert_eq!(hits(&events), [5, 9]);
    assert!(imap.log().iter().any(|l| l.contains("UID FETCH 5,9 ")));

    // Gmail: X-GM-RAW.
    let events = backend
        .execute(search("INBOX", "lunch", true, 10))
        .await
        .unwrap();
    assert_eq!(hits(&events), [9]);
    assert!(
        imap.log()
            .iter()
            .any(|l| l.contains(r#"UID SEARCH X-GM-RAW "lunch""#))
    );

    // No hits: no FETCH.
    let fetches = imap.log().iter().filter(|l| l.contains("FETCH")).count();
    let events = backend
        .execute(search("INBOX", "nothing-like-this", false, 10))
        .await
        .unwrap();
    assert_eq!(hits(&events), Vec::<u32>::new());
    assert_eq!(
        imap.log().iter().filter(|l| l.contains("FETCH")).count(),
        fetches
    );

    let events = backend
        .execute(search("Nope", "lunch", false, 10))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::SearchFailed { error, started_ms: 5, .. }] if error.contains("EXAMINE Nope")),
        "{events:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn session_syncs_a_listing_into_the_profile_store_and_reloads_it() {
    let dir = scratch("profile-sync");
    let dirs = ProfileDirs::at(&dir);
    let inbox = FakeFolder::new("INBOX", "\\HasNoChildren")
        .with(1, "\\Seen", "a@x", "Plan")
        .with(2, "", "b@x", "Re: Plan");
    let imap = FakeImap::new("app-password", vec![inbox]);
    let open = |imap: FakeImap| {
        let backend = LiveBackend::new(
            Arc::new(ProfileStore::open(&dirs).unwrap()),
            Arc::new(TestSecrets::default()),
            verifier(),
            Arc::new(imap),
        );
        Session::open(
            Arc::new(backend),
            Arc::new(SystemClock),
            Arc::new(CounterSeeds::starting_at(1)),
        )
    };
    let session = open(imap.clone()).await.unwrap();
    let receipt = session
        .submit(add("app-password"), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let account: AccountId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    session.flush().await.unwrap();
    session
        .ingest(Event::MailboxesListed {
            account,
            mailboxes: vec![app_core::RemoteMailbox {
                path: "INBOX".into(),
                delimiter: Some("/".into()),
                special_use: None,
                selectable: true,
            }],
        })
        .await
        .unwrap();
    session.flush().await.unwrap();
    let state = session.state();
    assert_eq!(state.messages.len(), 2, "{:?}", state.status);
    assert_eq!(state.threads.len(), 1);
    assert_eq!(state.status.persist_error, None);
    session.shutdown().await.unwrap();

    // A new session starts from the persisted cache.
    let session = open(imap).await.unwrap();
    let state = session.state();
    assert_eq!(state.accounts.len(), 1);
    assert_eq!(state.mailboxes.len(), 1);
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.threads.len(), 1);
    assert_eq!(state.mailboxes.values().next().unwrap().uid_next, Some(3));
    session.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn resync_flags_and_fetch_body_into_the_blob_store() {
    let dir = scratch("bodies");
    let dirs = ProfileDirs::at(&dir);
    let mut inbox = FakeFolder::new("INBOX", "\\HasNoChildren")
        .with(3, "\\Seen", "a@x", "Plan")
        .with(5, "\\Flagged", "b@x", "Re: Plan");
    inbox.uid_validity = 7;
    let imap = FakeImap::new("app-password", vec![inbox]);
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(1));
    let secrets = TestSecrets::default();
    secrets
        .set(&SecretRef::for_account(id), &Secret::new("app-password"))
        .unwrap();
    let store = Arc::new(ProfileStore::open(&dirs).unwrap());
    let backend = LiveBackend::new(
        Arc::clone(&store) as Arc<dyn Store>,
        Arc::new(secrets),
        verifier(),
        Arc::new(imap.clone()),
    );
    let Effect::FetchEnvelopes { imap: login, .. } = fetch("INBOX", None, None, 1) else {
        unreachable!()
    };
    let mailbox = MailboxId::from_uuid(uuid::Uuid::from_u128(2));

    // UIDs 1..=5: 3 and 5 exist (4 and 1 were expunged).
    let resync = Effect::ResyncFlags {
        account: id,
        mailbox,
        path: "INBOX".into(),
        imap: login.clone(),
        uid_validity: 7,
        from_uid: 1,
        to_uid: 5,
        changed_since: None,
    };
    let events = backend.execute(resync).await.unwrap();
    let [
        Event::FlagsChanged {
            uid_validity,
            flags,
            from_uid,
            to_uid,
            ..
        },
    ] = &events[..]
    else {
        panic!("unexpected events {events:?}");
    };
    assert_eq!((*uid_validity, *from_uid, *to_uid), (7, 1, 5));
    let summary: Vec<_> = flags
        .iter()
        .map(|f| (f.uid, f.flags.seen, f.flags.flagged))
        .collect();
    assert_eq!(summary, [(3, true, false), (5, false, true)]);

    let message = app_core::MessageId::from_uuid(uuid::Uuid::from_u128(9));
    let body = |uid, uid_validity| Effect::FetchBody {
        account: id,
        message,
        path: "INBOX".into(),
        imap: login.clone(),
        uid_validity,
        uid,
    };
    let events = backend.execute(body(5, 7)).await.unwrap();
    let [
        Event::BodyFetched {
            blob,
            has_attachments,
            snippet,
            ..
        },
    ] = &events[..]
    else {
        panic!("unexpected events {events:?}");
    };
    assert!(!has_attachments);
    assert!(
        snippet
            .as_deref()
            .is_some_and(|s| s.contains("Body of b@x")),
        "the snippet comes with the body: {snippet:?}"
    );
    let raw = store.get_blob(blob).unwrap().unwrap();
    assert!(String::from_utf8(raw).unwrap().contains("Body of b@x"));
    assert!(
        !imap
            .log()
            .iter()
            .any(|l| l.contains("BODY[]") && !l.contains("PEEK")),
        "bodies are fetched with PEEK so they stay unread"
    );

    for (uid, validity, needle) in [
        (4, 7, "no longer on the server"),
        (5, 6, "UIDVALIDITY changed"),
    ] {
        let events = backend.execute(body(uid, validity)).await.unwrap();
        assert!(
            matches!(&events[..], [Event::BodyFetchFailed { error, .. }] if error.contains(needle)),
            "{events:?}"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

fn run_op(steps: Vec<app_core::RemoteStep>) -> Effect {
    let Effect::FetchEnvelopes { imap, account, .. } = fetch("INBOX", None, None, 1) else {
        unreachable!()
    };
    Effect::RunRemoteOp {
        op: app_core::OpId::from_uuid(uuid::Uuid::from_u128(77)),
        account,
        imap,
        steps,
    }
}

fn flags_of(imap: &FakeImap, path: &str) -> Vec<(u32, String)> {
    let server = imap.server.lock().unwrap();
    server
        .folders
        .iter()
        .find(|f| f.path == path)
        .unwrap()
        .messages
        .iter()
        .map(|m| (m.uid, m.flags.clone()))
        .collect()
}

fn triage_folders() -> Vec<FakeFolder> {
    let mut inbox = FakeFolder::new("INBOX", "\\HasNoChildren")
        .with(1, "", "a@x", "One")
        .with(2, "\\Seen", "b@x", "Two")
        .with(3, "", "c@x", "Three");
    inbox.uid_validity = 7;
    vec![inbox, FakeFolder::new("Archive", "\\Archive")]
}

#[tokio::test]
async fn run_remote_op_stores_flags_and_moves_with_uid_move() {
    use app_core::{FlagKind, RemoteStep};
    let dir = scratch("op-move");
    let imap = FakeImap::new("app-password", triage_folders());
    let (backend, _) = mail_backend(&dir, "app-password", &imap);
    let events = backend
        .execute(run_op(vec![
            RemoteStep::Store {
                path: "INBOX".into(),
                uid_validity: 7,
                uids: vec![1, 3],
                flag: FlagKind::Seen,
                value: true,
            },
            RemoteStep::Store {
                path: "INBOX".into(),
                uid_validity: 7,
                uids: vec![2],
                flag: FlagKind::Seen,
                value: false,
            },
            RemoteStep::Move {
                path: "INBOX".into(),
                uid_validity: 7,
                uids: vec![2, 3],
                to: "Archive".into(),
                create: false,
            },
        ]))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::RemoteOpSucceeded { .. }]),
        "{events:?}"
    );
    assert_eq!(flags_of(&imap, "INBOX"), [(1, "\\Seen".to_owned())]);
    assert_eq!(
        flags_of(&imap, "Archive"),
        [(1, String::new()), (2, "\\Seen".to_owned())]
    );
    let log = imap.log();
    assert!(
        log.contains(&"UID STORE 1,3 +FLAGS.SILENT (\\Seen)".to_owned()),
        "{log:?}"
    );
    assert!(
        log.contains(&"UID MOVE 2:3 \"Archive\"".to_owned()),
        "{log:?}"
    );
    assert!(log.iter().any(|l| l.starts_with("SELECT")), "read-write");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn without_move_a_move_is_copy_delete_expunge_and_create_is_tolerant() {
    use app_core::RemoteStep;
    let dir = scratch("op-fallback");
    let imap = FakeImap::new("app-password", triage_folders());
    {
        let mut server = imap.server.lock().unwrap();
        // Neither MOVE nor UIDPLUS, and another client flagged UID 3 for its own expunge.
        server.no_move = true;
        server.folders[0].messages[2].flags = "\\Deleted".into();
    }
    let (backend, _) = mail_backend(&dir, "app-password", &imap);
    let events = backend
        .execute(run_op(vec![
            RemoteStep::Move {
                path: "INBOX".into(),
                uid_validity: 7,
                uids: vec![1],
                to: "Archive".into(),
                create: true,
            },
            RemoteStep::Copy {
                path: "INBOX".into(),
                uid_validity: 7,
                uids: vec![2],
                to: "Old".into(),
                create: true,
            },
        ]))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::RemoteOpSucceeded { .. }]),
        "{events:?}"
    );
    let inbox: Vec<u32> = flags_of(&imap, "INBOX")
        .iter()
        .map(|(uid, _)| *uid)
        .collect();
    assert_eq!(inbox, [2, 3], "only the moved message was expunged");
    assert_eq!(
        flags_of(&imap, "INBOX")[1],
        (3, "\\Deleted".to_owned()),
        "the other client's deletion is kept"
    );
    assert_eq!(flags_of(&imap, "Archive").len(), 1);
    assert_eq!(flags_of(&imap, "Old").len(), 1, "created, then copied into");
    let log = imap.log();
    let fallback: Vec<&String> = log
        .iter()
        .filter(|l| l.starts_with("UID") || l.starts_with("CREATE") || l.starts_with("EXPUNGE"))
        .collect();
    assert_eq!(
        fallback,
        [
            "CREATE \"Archive\"",
            "UID COPY 1 \"Archive\"",
            "UID STORE 1 +FLAGS.SILENT (\\Deleted)",
            "UID SEARCH DELETED",
            "UID STORE 3 -FLAGS.SILENT (\\Deleted)",
            "EXPUNGE",
            "UID STORE 3 +FLAGS.SILENT (\\Deleted)",
            "CREATE \"Old\"",
            "UID COPY 2 \"Old\"",
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn run_remote_op_failures_are_reported_as_events() {
    use app_core::{FlagKind, RemoteStep};
    let dir = scratch("op-fail");
    let imap = FakeImap::new("app-password", triage_folders());
    let (backend, _) = mail_backend(&dir, "app-password", &imap);
    let store = |path: &str, uid_validity| RemoteStep::Store {
        path: path.into(),
        uid_validity,
        uids: vec![1],
        flag: FlagKind::Flagged,
        value: true,
    };
    for (step, needle) in [
        (store("INBOX", 6), "UIDVALIDITY changed"),
        (store("Gone", 7), "IMAP SELECT Gone failed"),
        (
            RemoteStep::Move {
                path: "INBOX".into(),
                uid_validity: 7,
                uids: vec![1],
                to: "Nowhere".into(),
                create: false,
            },
            "IMAP UID MOVE INBOX -> Nowhere failed",
        ),
    ] {
        let events = backend.execute(run_op(vec![step])).await.unwrap();
        assert!(
            matches!(&events[..], [Event::RemoteOpFailed { error, .. }] if error.contains(needle)),
            "{events:?}"
        );
    }
    assert_eq!(flags_of(&imap, "INBOX")[0], (1, String::new()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn inspect_attachment_checks_local_files() {
    let dir = scratch("attachments");
    let file = dir.join("Q3 report.pdf");
    std::fs::write(&file, b"%PDF-1.4 numbers").unwrap();
    let (backend, _) = mail_backend(&dir, "pw", &FakeImap::default());
    let draft = app_core::DraftId::from_uuid(uuid::Uuid::from_u128(9));
    let inspect = |path: &std::path::Path| Effect::InspectAttachment {
        draft,
        path: path.display().to_string(),
    };
    let events = backend.execute(inspect(&file)).await.unwrap();
    assert_eq!(
        events,
        [Event::AttachmentInspected {
            draft,
            path: file.display().to_string(),
            size: 16,
            content_type: "application/pdf".into(),
        }]
    );
    for (path, expected) in [
        (dir.join("missing.txt"), "cannot read"),
        (dir.clone(), "is not a regular file"),
    ] {
        let events = backend.execute(inspect(&path)).await.unwrap();
        assert!(
            matches!(&events[..], [Event::AttachmentFailed { error, .. }] if error.contains(expected)),
            "{events:?}"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

fn outgoing(message_id: &str, subject: &str) -> app_core::OutgoingMessage {
    app_core::OutgoingMessage {
        from: app_core::Addr {
            name: Some("Me".into()),
            email: "me@gmail.com".into(),
        },
        to: vec![app_core::Addr {
            name: None,
            email: "bob@example.org".into(),
        }],
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: subject.into(),
        body_text: "Noon?".into(),
        message_id: message_id.into(),
        in_reply_to: None,
        references: Vec::new(),
        date_ms: 1_700_000_000_000,
        quote: None,
        attachments: Vec::new(),
    }
}

#[tokio::test]
async fn save_draft_appends_and_replaces_older_copies_then_delete_removes_them() {
    let dir = scratch("save-draft");
    let attached = dir.join("notes.txt");
    std::fs::write(&attached, b"see notes").unwrap();
    let imap = FakeImap::new(
        "pw",
        vec![
            FakeFolder::new("INBOX", "\\HasNoChildren"),
            FakeFolder::new("Drafts", "\\HasNoChildren \\Drafts")
                .with(1, "\\Seen \\Draft", "d1@gmail.com", "Old revision")
                .with(2, "\\Seen \\Draft", "other@gmail.com", "Another draft"),
        ],
    );
    let (backend, list) = mail_backend(&dir, "pw", &imap);
    let Effect::ListMailboxes {
        account,
        imap: login,
    } = list
    else {
        unreachable!()
    };
    let draft = app_core::DraftId::from_uuid(uuid::Uuid::from_u128(7));
    let mut message = outgoing("d1@gmail.com", "New revision");
    message.attachments.push(app_core::AttachmentRef {
        path: attached.display().to_string(),
        name: "notes.txt".into(),
        state: app_core::AttachmentState::Ready {
            size: 9,
            content_type: "text/plain".into(),
        },
    });
    let save = Effect::SaveDraft {
        draft,
        account,
        imap: login.clone(),
        path: "Drafts".into(),
        revision: 42,
        message: Box::new(message),
    };
    let events = backend.execute(save).await.unwrap();
    assert_eq!(
        events,
        [Event::DraftSaved {
            draft,
            revision: 42
        }]
    );
    let drafts = |imap: &FakeImap| -> Vec<(u32, String, String)> {
        let server = imap.server.lock().unwrap();
        let folder = server.folders.iter().find(|f| f.path == "Drafts").unwrap();
        folder
            .messages
            .iter()
            .map(|m| {
                let parsed = app_io::mime::parse_message(m.raw.as_bytes());
                (m.uid, m.flags.clone(), parsed.headers.subject)
            })
            .collect()
    };
    assert_eq!(
        drafts(&imap),
        [
            (2, "\\Seen \\Draft".to_owned(), "Another draft".to_owned()),
            (3, "\\Seen \\Draft".to_owned(), "New revision".to_owned()),
        ]
    );
    {
        let server = imap.server.lock().unwrap();
        let raw = &server.folders[1].messages[1].raw;
        assert_eq!(
            app_io::mime::parse_message(raw.as_bytes())
                .attachments
                .len(),
            1
        );
    }
    let log = imap.log();
    assert!(
        log.iter()
            .any(|l| l == "UID SEARCH HEADER Message-ID \"<d1@gmail.com>\""),
        "{log:?}"
    );
    assert!(
        log.iter().any(|l| l.starts_with("UID EXPUNGE 1")),
        "{log:?}"
    );

    let delete = Effect::DeleteDraft {
        draft,
        account,
        imap: login,
        path: "Drafts".into(),
        message_id: "d1@gmail.com".into(),
    };
    assert_eq!(
        backend.execute(delete).await.unwrap(),
        [Event::DraftDeleted { draft }]
    );
    assert_eq!(
        drafts(&imap),
        [(2, "\\Seen \\Draft".to_owned(), "Another draft".to_owned())]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn save_draft_failures_are_reported() {
    let dir = scratch("save-draft-fail");
    let imap = FakeImap::new("pw", vec![FakeFolder::new("INBOX", "")]);
    let (backend, list) = mail_backend(&dir, "pw", &imap);
    let Effect::ListMailboxes {
        account,
        imap: login,
    } = list
    else {
        unreachable!()
    };
    let draft = app_core::DraftId::from_uuid(uuid::Uuid::from_u128(7));
    // No Drafts folder on the server.
    let save = |message| Effect::SaveDraft {
        draft,
        account,
        imap: login.clone(),
        path: "Drafts".into(),
        revision: 1,
        message: Box::new(message),
    };
    let events = backend
        .execute(save(outgoing("d1@gmail.com", "x")))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::DraftSaveFailed { error, .. }] if error.contains("SELECT Drafts")),
        "{events:?}"
    );
    // An attachment that disappeared fails before anything is sent.
    let mut message = outgoing("d1@gmail.com", "x");
    message.attachments.push(app_core::AttachmentRef {
        path: dir.join("gone.pdf").display().to_string(),
        name: "gone.pdf".into(),
        state: app_core::AttachmentState::Ready {
            size: 1,
            content_type: "application/pdf".into(),
        },
    });
    let before = imap.log().len();
    let events = backend.execute(save(message)).await.unwrap();
    assert!(
        matches!(&events[..], [Event::DraftSaveFailed { error, .. }] if error.contains("cannot read")),
        "{events:?}"
    );
    assert_eq!(imap.log().len(), before, "no IMAP traffic");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_hostile_message_id_cannot_inject_imap_commands() {
    let dir = scratch("hostile-id");
    let imap = FakeImap::new(
        "pw",
        vec![
            FakeFolder::new("INBOX", "\\HasNoChildren"),
            FakeFolder::new("Drafts", "\\HasNoChildren \\Drafts"),
        ],
    );
    let (backend, list) = mail_backend(&dir, "pw", &imap);
    let Effect::ListMailboxes {
        account,
        imap: login,
    } = list
    else {
        unreachable!()
    };
    let draft = app_core::DraftId::from_uuid(uuid::Uuid::from_u128(7));
    let delete = Effect::DeleteDraft {
        draft,
        account,
        imap: login,
        path: "Drafts".into(),
        message_id: "a@b\r\nZ1 DELETE INBOX\r\nZ2 NOOP".into(),
    };
    assert_eq!(
        backend.execute(delete).await.unwrap(),
        [Event::DraftDeleted { draft }]
    );
    // One search line; the line breaks became spaces inside the quoted string.
    let log = imap.log();
    assert!(
        log.iter()
            .any(|l| l == "UID SEARCH HEADER Message-ID \"<a@b  Z1 DELETE INBOX  Z2 NOOP>\""),
        "{log:?}"
    );
    assert!(!log.iter().any(|l| l.starts_with("DELETE")), "{log:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// `login:password`, `MAIL FROM`, `RCPT TO` and the message of one submission.
type Submitted = (String, String, Vec<String>, String);

/// An `SmtpSender` that records submissions (or refuses them).
#[derive(Default)]
struct TestSmtp {
    sent: Mutex<Vec<Submitted>>,
    refuse: Option<String>,
}

impl app_sync::SmtpSender for TestSmtp {
    fn send<'a>(
        &'a self,
        login: &'a app_core::SmtpLogin,
        password: &'a Secret,
        message: app_sync::Submission<'a>,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let Some(reason) = &self.refuse {
                return Err(reason.clone());
            }
            self.sent.lock().unwrap().push((
                format!("{}:{}", login.login, password.expose()),
                message.from.to_owned(),
                message.recipients.to_vec(),
                String::from_utf8_lossy(message.raw).into_owned(),
            ));
            Ok(())
        })
    }
}

fn smtp_send(login: &ImapLogin, account: AccountId, sent_copy: Option<&str>) -> Effect {
    let mut message = outgoing("s1@gmail.com", "Q3");
    message.bcc.push(app_core::Addr {
        name: None,
        email: "boss@example.org".into(),
    });
    Effect::SmtpSend {
        draft: app_core::DraftId::from_uuid(uuid::Uuid::from_u128(7)),
        account,
        smtp: app_core::SmtpLogin {
            login: login.login.clone(),
            server: local(465, Security::Tls),
            secret: login.secret.clone(),
        },
        message: Box::new(message),
        sent_copy: sent_copy.map(|path| app_core::SentCopy {
            imap: login.clone(),
            path: path.into(),
        }),
    }
}

#[tokio::test]
async fn smtp_send_submits_without_bcc_and_appends_the_copy_to_sent() {
    let dir = scratch("smtp-send");
    let imap = FakeImap::new(
        "pw",
        vec![
            FakeFolder::new("INBOX", ""),
            FakeFolder::new("Sent", "\\Sent"),
        ],
    );
    let (backend, list) = mail_backend(&dir, "pw", &imap);
    let smtp = Arc::new(TestSmtp::default());
    let backend = backend.with_smtp(Arc::clone(&smtp) as Arc<dyn app_sync::SmtpSender>);
    let Effect::ListMailboxes {
        account,
        imap: login,
    } = list
    else {
        unreachable!()
    };
    let draft = app_core::DraftId::from_uuid(uuid::Uuid::from_u128(7));
    let events = backend
        .execute(smtp_send(&login, account, Some("Sent")))
        .await
        .unwrap();
    assert_eq!(
        events,
        [Event::MessageSent {
            draft,
            sent_copy_error: None
        }]
    );
    let sent = smtp.sent.lock().unwrap().clone();
    let [(auth, from, recipients, raw)] = &sent[..] else {
        panic!("{sent:?}")
    };
    assert_eq!(auth, "me@gmail.com:pw", "password from the keyring");
    assert_eq!(from, "me@gmail.com");
    assert_eq!(recipients, &["bob@example.org", "boss@example.org"]);
    assert!(!raw.contains("boss@"), "{raw}");
    {
        let server = imap.server.lock().unwrap();
        let copy = &server.folders[1].messages[0];
        assert_eq!(copy.flags, "\\Seen");
        assert!(copy.raw.contains("Bcc: <boss@example.org>"));
    }
    // No Sent folder on the server: sent anyway, the copy's failure is reported.
    let events = backend
        .execute(smtp_send(&login, account, Some("Gesendet")))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::MessageSent { sent_copy_error: Some(e), .. }] if e.contains("APPEND to Gesendet")),
        "{events:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_refused_submission_is_send_failed_and_nothing_is_appended() {
    let dir = scratch("smtp-refused");
    let imap = FakeImap::new(
        "pw",
        vec![
            FakeFolder::new("INBOX", ""),
            FakeFolder::new("Sent", "\\Sent"),
        ],
    );
    let (backend, list) = mail_backend(&dir, "pw", &imap);
    let backend = backend.with_smtp(Arc::new(TestSmtp {
        refuse: Some("SMTP 127.0.0.1:465: 550 relay denied".into()),
        ..TestSmtp::default()
    }));
    let Effect::ListMailboxes {
        account,
        imap: login,
    } = list
    else {
        unreachable!()
    };
    let events = backend
        .execute(smtp_send(&login, account, Some("Sent")))
        .await
        .unwrap();
    assert!(
        matches!(&events[..], [Event::SendFailed { error, .. }] if error.contains("relay denied")),
        "{events:?}"
    );
    assert!(imap.server.lock().unwrap().folders[1].messages.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn net_smtp_reports_an_unreachable_server_without_the_password() {
    let port = closed_port().await;
    let login = app_core::SmtpLogin {
        login: "me@example.com".into(),
        server: local(port, Security::Tls),
        secret: SecretRef::for_account(AccountId::from_uuid(uuid::Uuid::from_u128(1))),
    };
    let recipients = ["bob@example.org".to_owned()];
    let result = app_sync::NetSmtp::default()
        .with_timeout(Duration::from_secs(5))
        .send(
            &login,
            &Secret::new("hunter2"),
            app_sync::Submission {
                from: "me@example.com",
                recipients: &recipients,
                raw: b"Subject: x\r\n\r\nx\r\n",
            },
        )
        .await;
    let error = result.unwrap_err();
    assert!(
        error.starts_with(&format!("SMTP 127.0.0.1:{port}:")),
        "{error}"
    );
    assert!(!error.contains("hunter2"));
}

fn watch_of(list: &Effect) -> Effect {
    let Effect::ListMailboxes { account, imap } = list else {
        unreachable!()
    };
    Effect::WatchMailbox {
        account: *account,
        mailbox: MailboxId::from_uuid(uuid::Uuid::from_u128(2)),
        path: "INBOX".into(),
        imap: imap.clone(),
    }
}

#[tokio::test]
async fn watch_mailbox_idles_until_new_mail_and_reuses_its_connection() {
    let dir = scratch("idle");
    let imap = FakeImap::new("app-password", gmail_folders());
    let (backend, list) = mail_backend(&dir, "app-password", &imap);
    let backend = Arc::new(backend.with_idle_timeout(Duration::from_secs(5)));
    let watch = watch_of(&list);

    let waiting = tokio::spawn({
        let (backend, watch) = (Arc::clone(&backend), watch.clone());
        async move { backend.execute(watch).await.unwrap() }
    });
    while !imap.log().iter().any(|line| line == "IDLE") {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    imap.deliver("INBOX", "Subject: new\r\n\r\nHi\r\n");
    let events = tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&events[..], [Event::MailboxWatched { changed: true, .. }]),
        "{events:?}"
    );
    assert!(imap.log().iter().any(|line| line.starts_with("EXAMINE")));

    // A quiet IDLE ends at the timeout, on the same connection.
    let quiet = Arc::new(
        mail_backend(&dir, "app-password", &imap)
            .0
            .with_idle_timeout(Duration::from_millis(50)),
    );
    let events = quiet.execute(watch.clone()).await.unwrap();
    assert!(matches!(
        &events[..],
        [Event::MailboxWatched { changed: false, .. }]
    ));
    let connects = imap.connects();
    quiet.execute(watch).await.unwrap();
    assert_eq!(imap.connects(), connects, "the watch connection is reused");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn watch_mailbox_without_idle_or_login_fails() {
    let dir = scratch("idle-fail");
    let imap = FakeImap::new("app-password", gmail_folders());
    imap.server.lock().unwrap().no_idle = true;
    let (backend, list) = mail_backend(&dir, "app-password", &imap);
    let events = backend.execute(watch_of(&list)).await.unwrap();
    assert!(
        matches!(
            &events[..],
            [Event::WatchFailed {
                unsupported: true,
                ..
            }]
        ),
        "{events:?}"
    );
    let (wrong, list) = mail_backend(&dir, "wrong-password", &imap);
    let events = wrong.execute(watch_of(&list)).await.unwrap();
    let [
        Event::WatchFailed {
            unsupported: false,
            error,
            ..
        },
    ] = &events[..]
    else {
        panic!("{events:?}")
    };
    assert!(error.contains("LOGIN"), "{error}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn condstore_resyncs_only_changed_flags() {
    let dir = scratch("condstore");
    let mut inbox = FakeFolder::new("INBOX", "\\HasNoChildren")
        .with(3, "\\Seen", "a@x", "Plan")
        .with(5, "", "b@x", "Re: Plan")
        .with(6, "", "c@x", "Lunch");
    inbox.uid_validity = 7;
    let imap = FakeImap::new("app-password", vec![inbox]);
    imap.server.lock().unwrap().condstore = true;
    let (backend, list) = mail_backend(&dir, "app-password", &imap);
    let Effect::ListMailboxes {
        account,
        imap: login,
    } = list
    else {
        unreachable!()
    };
    let resync = |changed_since| Effect::ResyncFlags {
        account,
        mailbox: MailboxId::from_uuid(uuid::Uuid::from_u128(2)),
        path: "INBOX".into(),
        imap: login.clone(),
        uid_validity: 7,
        from_uid: 1,
        to_uid: 6,
        changed_since,
    };
    let flags_changed = |events: Vec<Event>| match events.as_slice() {
        [
            Event::FlagsChanged {
                flags,
                existing,
                highest_modseq,
                ..
            },
        ] => (
            flags.iter().map(|f| f.uid).collect::<Vec<_>>(),
            existing.clone(),
            *highest_modseq,
        ),
        other => panic!("{other:?}"),
    };

    // The first resync lists every flag and learns HIGHESTMODSEQ.
    let (uids, existing, modseq) = flags_changed(backend.execute(resync(None)).await.unwrap());
    assert_eq!((uids, existing, modseq), (vec![3, 5, 6], None, Some(1)));

    // Another client flags UID 5 and expunges UID 6.
    {
        let mut server = imap.server.lock().unwrap();
        let folder = &mut server.folders[0];
        folder.messages.retain(|m| m.uid != 6);
        let five = folder.messages.iter_mut().find(|m| m.uid == 5).unwrap();
        five.flags = "\\Flagged".into();
        five.modseq = 2;
    }
    let (uids, existing, modseq) = flags_changed(backend.execute(resync(Some(1))).await.unwrap());
    assert_eq!(uids, [5], "only the changed message");
    assert_eq!(existing, Some(vec![3, 5]), "UID 6 is gone");
    assert_eq!(modseq, Some(2));
    assert!(
        imap.log().iter().any(|l| l.contains("CHANGEDSINCE 1")),
        "{:?}",
        imap.log()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn fetch_older_walks_down_by_sequence_number() {
    let dir = scratch("older");
    let mut inbox = FakeFolder::new("INBOX", "\\HasNoChildren");
    for uid in [2, 4, 5, 7, 9, 10] {
        inbox = inbox.with(uid, "\\Seen", &format!("m{uid}@x"), "Old");
    }
    inbox.uid_validity = 7;
    let imap = FakeImap::new("app-password", vec![inbox]);
    let (backend, list) = mail_backend(&dir, "app-password", &imap);
    let Effect::ListMailboxes {
        account,
        imap: login,
    } = list
    else {
        unreachable!()
    };
    let older = |before_uid, limit, uid_validity| Effect::FetchOlder {
        account,
        mailbox: MailboxId::from_uuid(uuid::Uuid::from_u128(2)),
        path: "INBOX".into(),
        imap: login.clone(),
        uid_validity,
        before_uid,
        limit,
    };
    let fetched = |events: Vec<Event>| match events.as_slice() {
        [
            Event::OlderFetched {
                envelopes,
                complete,
                ..
            },
        ] => (
            envelopes.iter().map(|e| e.uid).collect::<Vec<_>>(),
            *complete,
        ),
        other => panic!("{other:?}"),
    };
    // UID 9 is the oldest cached: the two below it by sequence number.
    let got = fetched(backend.execute(older(9, 2, 7)).await.unwrap());
    assert_eq!(got, (vec![5, 7], false));
    let got = fetched(backend.execute(older(5, 10, 7)).await.unwrap());
    assert_eq!(got, (vec![2, 4], true), "down to the first message");
    // An expunged UID (8) falls back to a UID search.
    let got = fetched(backend.execute(older(8, 1, 7)).await.unwrap());
    assert_eq!(got, (vec![7], false));
    // Another UIDVALIDITY: nothing.
    let got = fetched(backend.execute(older(9, 2, 6)).await.unwrap());
    assert_eq!(got, (vec![], false));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn fetched_bodies_are_indexed_searched_and_dropped() {
    use app_io::fulltext::FullText;

    let dir = scratch("fulltext");
    let dirs = ProfileDirs::at(&dir);
    let mut inbox = FakeFolder::new("INBOX", "\\HasNoChildren").with(5, "", "b@x", "Re: Plan");
    inbox.uid_validity = 7;
    let imap = FakeImap::new("app-password", vec![inbox]);
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(1));
    let secrets = TestSecrets::default();
    secrets
        .set(&SecretRef::for_account(id), &Secret::new("app-password"))
        .unwrap();
    let store = Arc::new(ProfileStore::open(&dirs).unwrap());
    let index = Arc::new(FullText::in_memory().unwrap());
    let backend = LiveBackend::new(
        Arc::clone(&store) as Arc<dyn Store>,
        Arc::new(secrets),
        verifier(),
        Arc::new(imap.clone()),
    )
    .with_fulltext(Arc::clone(&index));
    let Effect::FetchEnvelopes { imap: login, .. } = fetch("INBOX", None, None, 1) else {
        unreachable!()
    };
    let message = app_core::MessageId::from_uuid(uuid::Uuid::from_u128(9));
    let events = backend
        .execute(Effect::FetchBody {
            account: id,
            message,
            path: "INBOX".into(),
            imap: login,
            uid_validity: 7,
            uid: 5,
        })
        .await
        .unwrap();
    let [Event::BodyFetched { blob, .. }] = &events[..] else {
        panic!("{events:?}")
    };
    let search = || Effect::SearchIndex {
        search: "body".into(),
        words: vec!["body".into()],
        limit: 10,
    };
    let events = backend.execute(search()).await.unwrap();
    assert_eq!(
        events,
        [Event::IndexSearched {
            search: "body".into(),
            hits: vec![message]
        }]
    );
    // Removed from the cache: removed from the index.
    let removal = app_core::MailDelta {
        removed_messages: vec![message],
        ..app_core::MailDelta::default()
    };
    backend
        .execute(Effect::PersistState {
            delta: app_core::PersistDelta::Mail(removal),
        })
        .await
        .unwrap();
    let events = backend.execute(search()).await.unwrap();
    assert!(matches!(&events[..], [Event::IndexSearched { hits, .. }] if hits.is_empty()));

    // A body stored before the index existed is indexed at the next start.
    let mut state = app_core::AppState::default();
    let mut envelope = app_core::RemoteEnvelope {
        uid: 5,
        message_id_hdr: None,
        in_reply_to: None,
        references: Vec::new(),
        from: Vec::new(),
        to: Vec::new(),
        cc: Vec::new(),
        subject: String::new(),
        date_ms: 0,
        flags: app_core::Flags::default(),
        size: 0,
        has_attachments: false,
    }
    .into_envelope(
        message,
        id,
        MailboxId::from_uuid(uuid::Uuid::from_u128(2)),
        app_core::ThreadId::from_uuid(uuid::Uuid::from_u128(3)),
    );
    envelope.body = app_core::BodyState::Fetched { blob: blob.clone() };
    state.messages.insert(message, envelope);
    let fresh = Arc::new(FullText::in_memory().unwrap());
    app_sync::fulltext::index_missing(Arc::clone(&fresh), store, &state);
    let mut hits = Vec::new();
    for _ in 0..200 {
        hits = fresh.search(&["body".into()], 10).unwrap();
        if !hits.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(hits, [message]);
    std::fs::remove_dir_all(dir).unwrap();
}
