//! Remote profiles (ADR 0017, SRV-5a): a device session (`Session::open_remote`) linked over
//! loopback TLS to an in-process dail server over the memory backend (`TestServer`): the
//! snapshot and the diffs, forwarded commands with the device's seed, refusals, placement,
//! uploads, blobs, a server restart and the online-only refusal.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use app_core::{
    AppState, Command, DeviceRole, DraftId, LinkStatus, Origin, ProviderPreset, Secret, SettingKey,
    ThreadId,
};
use app_sync::{Connection, RemoteOptions, Session, SyncError};
use app_testkit::{Fixture, MemoryBackend, TestDevice, TestServer};
use serde_json::json;

fn temp_dir_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("dail-remote-{}-{name}", std::process::id()))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = temp_dir_path(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Short pauses, so a lost server is dialed again at once.
fn options() -> RemoteOptions {
    RemoteOptions {
        backoff_min: Duration::from_millis(50),
        backoff_max: Duration::from_millis(200),
        settle_timeout: Duration::from_secs(10),
    }
}

/// A server on `fixture` with alice's account synced.
async fn server(name: &str, fixture: Fixture) -> TestServer {
    let server = TestServer::start(&temp_dir(name), MemoryBackend::from_fixture(fixture))
        .await
        .unwrap();
    let session = server.session().unwrap();
    for cmd in [
        Command::AddAccount {
            preset: ProviderPreset::Hostinger,
            email: "alice@example.com".into(),
            display_name: None,
            password: Some(Secret::new("pw")),
            imap: None,
            smtp: None,
        },
        Command::SyncNow { account: None },
    ] {
        session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
        session.flush().await.unwrap();
    }
    server
}

/// A user device of `server`, online.
async fn device(server: &TestServer) -> (Session, Connection) {
    let session = server
        .device(DeviceRole::User, 1, options())
        .await
        .unwrap()
        .session;
    let conn = session.connection();
    assert!(matches!(conn, Connection::Remote(_)));
    let remote = session.remote().unwrap();
    assert_eq!(remote.wait_settled().await, Some(LinkStatus::Online));
    (session, conn)
}

/// Waits until `check` holds for the device's state.
async fn until(session: &Session, what: &str, check: impl Fn(&AppState) -> bool) {
    let mut states = session.subscribe();
    let wait = async {
        loop {
            if check(&states.borrow_and_update()) {
                return;
            }
            states.changed().await.unwrap();
        }
    };
    if tokio::time::timeout(Duration::from_secs(10), wait)
        .await
        .is_err()
    {
        panic!("timed out waiting until {what}");
    }
}

fn thread(state: &AppState, subject: &str) -> ThreadId {
    state
        .threads
        .values()
        .find(|t| t.subject == subject)
        .unwrap_or_else(|| panic!("no thread {subject}"))
        .id
}

/// Whether any message of thread `id` is starred.
fn starred(state: &AppState, id: ThreadId) -> bool {
    state.threads[&id]
        .messages
        .iter()
        .any(|m| state.messages[m].flags.flagged)
}

fn status(session: &Session) -> Option<LinkStatus> {
    session.state().link.as_ref().map(|link| link.status)
}

#[tokio::test]
async fn a_device_mirrors_the_snapshot_and_the_servers_changes() {
    let server = server("snapshot", Fixture::builtin("basic").unwrap()).await;
    let (device, _conn) = device(&server).await;
    let mirrored = device.state();
    let served = server.session().unwrap().state();
    assert_eq!(mirrored.accounts.len(), 1);
    assert_eq!(mirrored.threads.len(), served.threads.len());
    assert_eq!(mirrored.messages.len(), served.messages.len());
    assert!(mirrored.link.as_ref().unwrap().revision > 0);
    assert!(mirrored.link.as_ref().unwrap().error.is_none());

    // A change made on the server arrives as a diff.
    let q3 = thread(&served, "Q3 numbers");
    server
        .session()
        .unwrap()
        .submit(
            Command::Star {
                ids: vec![q3],
                messages: Vec::new(),
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    until(&device, "the star arrives", |state| starred(state, q3)).await;
    // The device ran no mail effect of its own: nothing reached its fake servers.
    device.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_forwarded_command_creates_the_same_ids_and_the_server_confirms_it() {
    let server = server("seeded", Fixture::builtin("basic").unwrap()).await;
    let (device, conn) = device(&server).await;
    let account = *device.state().accounts.keys().next().unwrap();
    let receipt = conn
        .submit(
            Command::CreateDraft {
                account,
                to: Vec::new(),
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "From the device".into(),
                body: "Hello".into(),
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    // Applied on the mirror at once (optimistic), with the ID the server made from the same
    // seed.
    assert!(device.state().drafts.contains_key(&draft));
    assert!(
        server
            .session()
            .unwrap()
            .state()
            .drafts
            .contains_key(&draft)
    );
    conn.flush().await.unwrap();
    assert_eq!(
        device.state().drafts[&draft].subject,
        server.session().unwrap().state().drafts[&draft].subject
    );

    // Triage: applied here, confirmed by the server's diff.
    let q3 = thread(&device.state(), "Q3 numbers");
    conn.submit(
        Command::Star {
            ids: vec![q3],
            messages: Vec::new(),
        },
        Origin::Tui,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(starred(&device.state(), q3));
    assert!(starred(&server.session().unwrap().state(), q3));
    conn.flush().await.unwrap();
    assert!(starred(&device.state(), q3));
    // Online: nothing is queued.
    assert!(device.state().link.as_ref().unwrap().queued.is_empty());
}

#[tokio::test]
async fn a_server_refusal_reaches_the_caller_and_the_mirror_is_reset() {
    let server = server("refused", Fixture::builtin("basic").unwrap()).await;
    let (device, conn) = device(&server).await;
    let account = *device.state().accounts.keys().next().unwrap();
    // A draft only the mirror has (submitted to the local session, not through the link).
    let local = device
        .submit(
            Command::CreateDraft {
                account,
                to: Vec::new(),
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Mirror only".into(),
                body: String::new(),
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(local.data["id"].clone()).unwrap();
    assert!(device.state().drafts.contains_key(&draft));
    // The mirror accepts the edit; the server does not know the draft.
    let refused = conn
        .submit(
            Command::UpdateDraft {
                id: draft,
                to: None,
                cc: None,
                bcc: None,
                subject: Some("Edited".into()),
                body: None,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap_err();
    assert!(refused.to_string().starts_with("ERR"), "{refused}");
    assert!(
        refused.to_string().contains("not found") || refused.to_string().contains("draft"),
        "{refused}"
    );
    // A fresh snapshot replaced the mirror: the server's state, without the local draft.
    assert!(!device.state().drafts.contains_key(&draft));
    assert_eq!(
        device.state().threads.len(),
        server.session().unwrap().state().threads.len()
    );
}

#[tokio::test]
async fn client_commands_never_reach_the_server() {
    let server = server("client", Fixture::builtin("basic").unwrap()).await;
    let (device, conn) = device(&server).await;
    let before = server.session().unwrap().state();
    let q3 = thread(&device.state(), "Q3 numbers");
    for cmd in [
        Command::OpenThread { id: q3 },
        Command::SetSetting {
            key: SettingKey::TextScale,
            value: json!(150),
        },
    ] {
        assert!(cmd.placement() == app_core::Placement::Client);
        conn.submit(cmd, Origin::Gui).await.unwrap().unwrap();
    }
    conn.flush().await.unwrap();
    assert_eq!(device.state().settings.text_scale, 150);
    assert_eq!(device.state().view.thread, Some(q3));
    let after = server.session().unwrap().state();
    assert_eq!(after.settings.text_scale, before.settings.text_scale);
    assert_eq!(after.revision, before.revision);
}

#[tokio::test]
async fn attach_file_uploads_the_file_and_the_server_attaches_its_copy() {
    let server = server("upload", Fixture::builtin("basic").unwrap()).await;
    let (device, conn) = device(&server).await;
    let account = *device.state().accounts.keys().next().unwrap();
    let receipt = conn
        .submit(
            Command::CreateDraft {
                account,
                to: Vec::new(),
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "With a file".into(),
                body: String::new(),
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    let file = temp_dir("upload-file").join("notes.txt");
    std::fs::write(&file, b"local notes").unwrap();
    let local_path = file.display().to_string();
    conn.submit(
        Command::AttachFile {
            draft,
            path: local_path.clone(),
            index: None,
        },
        Origin::Cli,
    )
    .await
    .unwrap()
    .unwrap();
    server.session().unwrap().flush().await.unwrap();
    let served = server.session().unwrap().state();
    let attached = &served.drafts[&draft].attachments;
    assert_eq!(attached.len(), 1);
    let path = &attached[0].path;
    assert_ne!(path, &local_path);
    assert!(path.ends_with("notes.txt"), "{path}");
    assert!(path.contains("uploads"), "{path}");
    assert_eq!(std::fs::read(path).unwrap(), b"local notes");
    // The mirror ends with the server's copy.
    conn.flush().await.unwrap();
    assert_eq!(device.state().drafts[&draft].attachments[0].path, *path);

    // A missing local file is refused before anything changes.
    let missing = conn
        .submit(
            Command::AttachFile {
                draft,
                path: "/nonexistent/dail/file.txt".into(),
                index: None,
            },
            Origin::Cli,
        )
        .await
        .unwrap_err();
    assert!(matches!(missing, SyncError::Remote(_)), "{missing}");
}

#[tokio::test]
async fn save_attachments_fetches_the_body_from_the_server() {
    let server = server("blob", Fixture::media().unwrap()).await;
    let session = server.session().unwrap();
    let state = session.state();
    let message = state
        .messages
        .values()
        .find(|m| m.subject == "Photos and the report")
        .map(|m| m.id)
        .unwrap();
    let thread_id = state.messages[&message].thread;
    // The server downloads the body (a query starts it).
    session
        .submit(
            Command::GetThread {
                id: thread_id,
                include_body: true,
                rich_body: false,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();

    let (device, conn) = device(&server).await;
    conn.flush().await.unwrap();
    // A query for the server is answered by the server, with the body.
    let answered = conn
        .submit(
            Command::GetThread {
                id: thread_id,
                include_body: true,
                rich_body: false,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        answered.data["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["untrusted_content"]["body"]["text"].is_string()),
        "{}",
        answered.data
    );
    let dir = temp_dir("blob-saved");
    conn.submit(
        Command::SaveAttachments {
            message,
            parts: Vec::new(),
            dir: dir.display().to_string(),
        },
        Origin::Cli,
    )
    .await
    .unwrap()
    .unwrap();
    conn.flush().await.unwrap();
    let saved: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        saved.iter().any(|name| name == "photo.jpg"),
        "{saved:?} {:?} {:?}",
        device.state().status.saves,
        device.state().messages[&message].body
    );
    assert!(device.state().messages.contains_key(&message));
}

#[tokio::test]
async fn the_device_resyncs_after_the_server_restarts() {
    let mut server = server("restart", Fixture::builtin("basic").unwrap()).await;
    let (device, conn) = device(&server).await;
    let threads = device.state().threads.len();
    server.restart().await.unwrap();
    // The link goes down and comes back with a fresh snapshot from the new instance.
    until(&device, "the link is lost", |state| {
        state.link.as_ref().unwrap().status != LinkStatus::Online
    })
    .await;
    until(&device, "the link is back", |state| {
        state.link.as_ref().unwrap().status == LinkStatus::Online
    })
    .await;
    assert_eq!(
        device.remote().unwrap().wait_settled().await,
        Some(LinkStatus::Online)
    );
    assert_eq!(device.state().threads.len(), threads);
    // Commands reach the new instance.
    let q3 = thread(&device.state(), "Q3 numbers");
    conn.submit(
        Command::Star {
            ids: vec![q3],
            messages: Vec::new(),
        },
        Origin::Cli,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(starred(&server.session().unwrap().state(), q3));
}

#[tokio::test]
async fn without_the_server_commands_for_it_are_refused_and_queries_use_the_mirror() {
    let server = server("offline", Fixture::builtin("basic").unwrap()).await;
    let (device, conn) = device(&server).await;
    let q3 = thread(&device.state(), "Q3 numbers");
    let threads = device.state().threads.len();
    drop(server);
    until(&device, "the link is offline", |state| {
        state.link.as_ref().unwrap().status == LinkStatus::Offline
    })
    .await;
    let link = device.state().link.clone().unwrap();
    assert!(link.error.is_some());
    let refused = conn
        .submit(
            Command::Star {
                ids: vec![q3],
                messages: Vec::new(),
            },
            Origin::Cli,
        )
        .await
        .unwrap_err();
    assert!(matches!(refused, SyncError::NotOnline { .. }), "{refused}");
    assert!(
        refused.to_string().starts_with("ERR212: `star`"),
        "{refused}"
    );
    // Nothing was applied or queued.
    assert!(!starred(&device.state(), q3));
    assert!(device.state().link.as_ref().unwrap().queued.is_empty());
    // Queries are answered from the mirror; the device's own commands still run.
    let listed = conn
        .submit(
            Command::ListThreads {
                mailbox: *device.state().mailboxes.keys().next().unwrap(),
                offset: 0,
                limit: 10,
            },
            Origin::Cli,
        )
        .await;
    assert!(matches!(listed, Ok(Ok(_))), "{listed:?}");
    conn.submit(Command::OpenThread { id: q3 }, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(device.state().threads.len(), threads);
    assert!(matches!(
        status(&device),
        Some(LinkStatus::Offline | LinkStatus::Connecting)
    ));
}

#[tokio::test]
async fn the_device_token_is_never_in_state_errors_or_debug_output() {
    let mut server = server("token", Fixture::builtin("basic").unwrap()).await;
    let TestDevice {
        session: device,
        secrets,
        ..
    } = server.device(DeviceRole::User, 1, options()).await.unwrap();
    let remote = device.remote().unwrap();
    assert_eq!(remote.wait_settled().await, Some(LinkStatus::Online));
    let entry = device
        .state()
        .link
        .as_ref()
        .and_then(|link| link.server.as_ref())
        .and_then(|server| server.token.clone())
        .unwrap();
    let token = app_sync::SecretStore::get(secrets.as_ref(), &entry)
        .unwrap()
        .unwrap();
    let token = token.expose().to_owned();
    assert_eq!(token.len(), 43);
    let state = serde_json::to_string(device.state().as_ref()).unwrap();
    assert!(!state.contains(&token));
    assert!(!format!("{remote:?} {secrets:?}").contains(&token));
    let config = std::fs::read_to_string(temp_dir_path("token").join("config.toml")).unwrap();
    assert!(!config.contains(&token));
    // A token the server does not know: `hello` is refused, without echoing it.
    app_sync::SecretStore::set(secrets.as_ref(), &entry, &Secret::new("wrong-token-value"))
        .unwrap();
    server.restart().await.unwrap();
    until(&device, "hello is refused", |state| {
        state
            .link
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .is_some_and(|error| error.contains("hello"))
    })
    .await;
    let error = device.state().link.as_ref().unwrap().error.clone().unwrap();
    assert!(
        !error.contains(&token) && !error.contains("wrong-token-value"),
        "{error}"
    );
    assert_ne!(status(&device), Some(LinkStatus::Online));
}

/// Whether `backend` ever ran an SMTP submission.
fn sent_mail(backend: &MemoryBackend) -> bool {
    backend
        .executed()
        .iter()
        .any(|effect| matches!(effect, app_core::Effect::SmtpSend { .. }))
}

fn new_draft(account: app_core::AccountId) -> Command {
    Command::CreateDraft {
        account,
        to: vec![app_core::Addr {
            name: None,
            email: "bob@example.org".into(),
        }],
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "From an agent".into(),
        body: "Hello".into(),
    }
}

#[tokio::test]
async fn a_refused_origin_is_a_clean_refusal_and_the_link_stays_up() {
    let server = server("origin", Fixture::builtin("basic").unwrap()).await;
    let (device, conn) = device(&server).await;
    let q3 = thread(&device.state(), "Q3 numbers");
    let star = Command::Star {
        ids: vec![q3],
        messages: Vec::new(),
    };
    // A user device may not act as MCP: refused before anything is applied.
    let refused = conn.submit(star.clone(), Origin::Mcp).await.unwrap_err();
    assert!(
        matches!(
            refused,
            SyncError::Refused {
                code: app_sync::serve::ORIGIN_REFUSED,
                ..
            }
        ),
        "{refused}"
    );
    assert!(refused.to_string().starts_with("ERR213: "), "{refused}");
    assert!(refused.to_string().contains("`user`"), "{refused}");
    assert!(!starred(&device.state(), q3));
    assert!(device.remote().unwrap().link().is_online());
    assert_eq!(status(&device), Some(LinkStatus::Online));
    // A preview as MCP is refused the same way.
    assert!(matches!(
        conn.preview(star.clone(), Origin::Mcp).await,
        Err(SyncError::Refused { .. })
    ));

    // An agent device as `mcp_allow_send` on a server without `--allow-send`: the server
    // refuses the request; the mirror's optimistic change is taken back, the link stays up.
    let agent = server
        .device(DeviceRole::Agent, 1 << 20, options())
        .await
        .unwrap()
        .session;
    let agent_conn = agent.connection();
    assert_eq!(
        agent.remote().unwrap().wait_settled().await,
        Some(LinkStatus::Online)
    );
    let refused = agent_conn
        .submit(star.clone(), Origin::McpAllowSend)
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            SyncError::Refused {
                code: app_sync::serve::ORIGIN_REFUSED,
                ..
            }
        ),
        "{refused}"
    );
    assert!(!starred(&agent.state(), q3));
    assert!(agent.remote().unwrap().link().is_online());
    assert_eq!(status(&agent), Some(LinkStatus::Online));
    assert!(!starred(&server.session().unwrap().state(), q3));
    // The links still work.
    conn.submit(star, Origin::Cli).await.unwrap().unwrap();
    assert!(starred(&server.session().unwrap().state(), q3));
}

#[tokio::test]
async fn an_agents_send_over_the_link_waits_for_approval_on_the_server() {
    let server = server("outbound", Fixture::builtin("basic").unwrap()).await;
    let account = *server
        .session()
        .unwrap()
        .state()
        .accounts
        .keys()
        .next()
        .unwrap();
    let agent = server
        .device(DeviceRole::Agent, 1, options())
        .await
        .unwrap();
    let conn = agent.session.connection();
    assert_eq!(
        agent.session.remote().unwrap().wait_settled().await,
        Some(LinkStatus::Online)
    );
    let draft = conn
        .submit(new_draft(account), Origin::Mcp)
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(draft.data["id"].clone()).unwrap();
    conn.submit(Command::SendDraft { id: draft }, Origin::Mcp)
        .await
        .unwrap()
        .unwrap();
    conn.flush().await.unwrap();
    let served = server.session().unwrap().state();
    let approvals: Vec<_> = served.approvals.values().collect();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].draft, draft);
    assert_eq!(approvals[0].origin, Origin::Mcp);
    assert!(server.backend().sent().is_empty());
    assert!(!sent_mail(server.backend()));
    // The mirror shows the same approval; the device sent nothing itself.
    assert_eq!(agent.session.state().approvals.len(), 1);
    assert!(!sent_mail(&agent.backend));
}

#[tokio::test]
async fn before_the_first_snapshot_an_agents_send_creates_an_approval_and_sends_nothing() {
    let server = server("outbound-early", Fixture::builtin("basic").unwrap()).await;
    let served = server.session().unwrap().state();
    let token = server.pair(DeviceRole::Agent).await.unwrap();
    let entry = app_core::SecretRef::for_server(uuid::Uuid::from_u128(7));
    let secrets = std::sync::Arc::new(app_testkit::MemorySecrets::new());
    app_sync::SecretStore::set(secrets.as_ref(), &entry, &token).unwrap();
    // A mirror that has the account from an earlier connection, but no server to reach yet.
    let mut state = server.device_state(entry);
    if let Some(server_link) = state.link.as_mut().and_then(|l| l.server.as_mut()) {
        server_link.port = 1;
    }
    state.accounts = served.accounts.clone();
    let account = *state.accounts.keys().next().unwrap();
    let device = app_testkit::remote::open_device(state, secrets, 1, options())
        .await
        .unwrap();
    let session = &device.session;
    // Through the link nothing goes out while it is down.
    let refused = session
        .connection()
        .submit(new_draft(account), Origin::Mcp)
        .await
        .unwrap_err();
    assert!(matches!(refused, SyncError::NotOnline { .. }), "{refused}");
    // The engine itself, in mirror mode: the send becomes an approval and no SMTP effect.
    let draft = session
        .submit(new_draft(account), Origin::Mcp)
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(draft.data["id"].clone()).unwrap();
    session
        .submit(Command::SendDraft { id: draft }, Origin::Mcp)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    let state = session.state();
    assert_eq!(state.approvals.len(), 1);
    assert_eq!(state.approvals.values().next().unwrap().origin, Origin::Mcp);
    assert!(!sent_mail(&device.backend));
    assert!(server.backend().sent().is_empty());
    assert!(server.session().unwrap().state().approvals.is_empty());
}

#[tokio::test]
async fn a_snapshot_persists_what_it_took_back() {
    let server = server("persist", Fixture::builtin("basic").unwrap()).await;
    let device = server.device(DeviceRole::User, 1, options()).await.unwrap();
    let session = &device.session;
    assert_eq!(
        session.remote().unwrap().wait_settled().await,
        Some(LinkStatus::Online)
    );
    let account = *session.state().accounts.keys().next().unwrap();
    // A draft only the mirror has, persisted locally.
    let local = session
        .submit(new_draft(account), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(local.data["id"].clone()).unwrap();
    session.flush().await.unwrap();
    let stored = |backend: &MemoryBackend| {
        backend
            .store()
            .snapshot()
            .is_some_and(|state| state.drafts.contains_key(&draft))
    };
    assert!(stored(&device.backend));
    // The server refuses it; the snapshot takes it back, in the store too.
    let refused = session
        .connection()
        .submit(
            Command::UpdateDraft {
                id: draft,
                to: None,
                cc: None,
                bcc: None,
                subject: Some("Edited".into()),
                body: None,
            },
            Origin::Cli,
        )
        .await
        .unwrap();
    assert!(refused.is_err());
    session.flush().await.unwrap();
    assert!(!session.state().drafts.contains_key(&draft));
    assert!(!stored(&device.backend));
    // The mirrored mail is stored.
    let snapshot = device.backend.store().snapshot().unwrap();
    assert_eq!(snapshot.threads.len(), session.state().threads.len());
}
