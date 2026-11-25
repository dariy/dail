//! Single-instance lock and IPC attach (`ARCHITECTURE.md §5.4`) over a real local socket.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::{
    AppState, Command, CoreError, Effect, Event, Origin, ProviderPreset, Secret, SettingKey,
};
use app_sync::ipc::{self, Claim};
use app_sync::{
    Backend, BoxFuture, Connection, CounterSeeds, IpcClient, IpcEndpoint, IpcServer, ProfileLock,
    Session, SyncError, VirtualClock,
};
use serde_json::json;

#[derive(Default)]
struct RecordingBackend {
    executed: Mutex<Vec<Effect>>,
}

impl Backend for RecordingBackend {
    fn name(&self) -> &'static str {
        "recording"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        Box::pin(async { Ok(None) })
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            self.executed.lock().unwrap().push(effect);
            Ok(vec![Event::Persisted {}])
        })
    }
}

fn profile(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-ipc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn open(backend: Arc<RecordingBackend>) -> Session {
    Session::open(
        backend,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

fn poll_secs(value: u64) -> Command {
    Command::SetSetting {
        key: SettingKey::PollSecs,
        value: json!(value),
    }
}

#[tokio::test]
async fn a_second_process_attaches_to_the_owner_and_shares_its_state() {
    let dir = profile("attach");
    let Claim::Owned(_lock) = ipc::claim(&dir).await.unwrap() else {
        unreachable!("a fresh profile is owned by the first claimant");
    };
    let session = open(Arc::default()).await;
    let _server = IpcServer::listen(&dir, session.client()).unwrap();

    let Claim::Attached(client) = ipc::claim(&dir).await.unwrap() else {
        unreachable!("the second claimant attaches");
    };
    let attached = Connection::Attached(client);
    let receipt = attached
        .submit(poll_secs(300), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.name, "set_setting");
    attached.flush().await.unwrap();
    assert_eq!(
        session.state().settings.poll_secs,
        300,
        "the owner's state changed"
    );
    assert_eq!(attached.state().await.unwrap().settings.poll_secs, 300);

    // Rejections travel as structured engine errors, not as transport errors.
    let rejected = attached
        .submit(Command::Undo {}, Origin::Cli)
        .await
        .unwrap();
    assert!(rejected.is_ok(), "undo of set_setting works");
    let again = attached
        .submit(Command::Redo {}, Origin::Cli)
        .await
        .unwrap();
    assert!(again.is_ok());
    let nothing = attached
        .submit(Command::Redo {}, Origin::Cli)
        .await
        .unwrap();
    assert!(
        matches!(nothing, Err(CoreError::NothingToRedo)),
        "{nothing:?}"
    );
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn an_attached_terminal_ui_submits_as_tui() {
    let dir = profile("tui");
    let session = open(Arc::default()).await;
    let _server = IpcServer::listen(&dir, session.client()).unwrap();
    let client = IpcClient::connect(&IpcEndpoint::for_profile(&dir))
        .await
        .unwrap();
    assert!(
        client
            .submit(&poll_secs(300), Origin::Tui)
            .await
            .unwrap()
            .is_ok()
    );
    assert_eq!(session.state().settings.poll_secs, 300);
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn clients_cannot_pose_as_the_gui_or_the_runtime() {
    let dir = profile("origin");
    let session = open(Arc::default()).await;
    let _server = IpcServer::listen(&dir, session.client()).unwrap();
    let client = IpcClient::connect(&IpcEndpoint::for_profile(&dir))
        .await
        .unwrap();
    for origin in [Origin::Gui, Origin::Runtime] {
        let err = client.submit(&poll_secs(300), origin).await.unwrap_err();
        assert!(
            matches!(err, SyncError::Ipc(ref m) if m.contains("can't be claimed")),
            "{err}"
        );
    }
    assert_eq!(session.state().settings.poll_secs, 120);
    assert!(
        client
            .submit(&poll_secs(300), Origin::Mcp)
            .await
            .unwrap()
            .is_ok()
    );
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn the_password_of_add_account_reaches_the_owner() {
    let dir = profile("secret");
    let backend = Arc::new(RecordingBackend::default());
    let session = open(Arc::clone(&backend)).await;
    let _server = IpcServer::listen(&dir, session.client()).unwrap();
    let client = IpcClient::connect(&IpcEndpoint::for_profile(&dir))
        .await
        .unwrap();
    let cmd = Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "me@gmail.com".to_owned(),
        display_name: None,
        password: Some(Secret::new("app password")),
        imap: None,
        smtp: None,
    };
    client.submit(&cmd, Origin::Cli).await.unwrap().unwrap();
    client.flush().await.unwrap();
    let stored = backend
        .executed
        .lock()
        .unwrap()
        .iter()
        .find_map(|effect| match effect {
            Effect::StoreSecret { value, .. } => Some(value.expose().to_owned()),
            _ => None,
        });
    assert_eq!(stored.as_deref(), Some("app password"));
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn the_lock_is_exclusive_and_released_with_its_holder() {
    let dir = profile("lock");
    let path = dir.join("lock");
    let held = ProfileLock::try_acquire(&path).unwrap().unwrap();
    assert!(ProfileLock::try_acquire(&path).unwrap().is_none());
    drop(held);
    assert!(ProfileLock::try_acquire(&path).unwrap().is_some());
}

#[tokio::test]
async fn stopping_the_owner_disconnects_its_clients() {
    let dir = profile("stop");
    let session = open(Arc::default()).await;
    let server = IpcServer::listen(&dir, session.client()).unwrap();
    let endpoint = server.endpoint().clone();
    let client = IpcClient::connect(&endpoint).await.unwrap();
    drop(server);
    tokio::task::yield_now().await;
    let err = client
        .submit(&poll_secs(300), Origin::Cli)
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::SessionClosed), "{err}");
    assert!(IpcClient::connect(&endpoint).await.is_err());
    // A new owner can listen again (a stale socket file would be replaced too).
    let _again = IpcServer::listen(&dir, session.client()).unwrap();
    IpcClient::connect(&endpoint).await.unwrap();
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn clients_of_a_shut_down_session_see_it_closed() {
    let session = open(Arc::default()).await;
    let client = session.client();
    session.shutdown().await.unwrap();
    let err = client
        .submit(poll_secs(300), Origin::Cli)
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::SessionClosed));
}

#[tokio::test]
async fn an_attached_subscriber_sees_the_owners_changes_until_it_goes_away() {
    let dir = profile("watch");
    let session = open(Arc::default()).await;
    let server = IpcServer::listen(&dir, session.client()).unwrap();
    let client = IpcClient::connect(&IpcEndpoint::for_profile(&dir))
        .await
        .unwrap();
    let conn = Connection::Attached(client);
    let mut state = conn.subscribe().await.unwrap();
    assert_eq!(state.borrow_and_update().settings.poll_secs, 120);
    // The owner's own change (as the GUI would make it) reaches the attached front-end.
    session
        .submit(poll_secs(300), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while state.borrow_and_update().settings.poll_secs != 300 {
            state.changed().await.unwrap();
        }
    })
    .await
    .expect("the change arrived");
    // A local connection gives the session's own channel.
    let local = Connection::Local(session.client());
    assert_eq!(
        local.subscribe().await.unwrap().borrow().settings.poll_secs,
        300
    );
    drop(server);
    session.shutdown().await.unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        while state.changed().await.is_ok() {}
    })
    .await;
    assert!(ended.is_ok(), "the subscription ends with the owner");
}

async fn replies_of<T>(bridge: &app_sync::Bridge<T>, n: usize) -> Vec<app_sync::Reply<T>> {
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while out.len() < n {
            out.extend(bridge.replies());
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_owning_bridge_tags_replies_and_shuts_its_session_down() {
    let session = open(Arc::default()).await;
    let client = session.client();
    let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = Arc::clone(&woken);
    let (mut bridge, task) = app_sync::Bridge::own(
        &tokio::runtime::Handle::current(),
        session,
        Origin::Gui,
        Arc::new(move || {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }),
    );
    bridge.submit("first", poll_secs(300));
    bridge.submit("second", Command::Undo {});
    let replies = replies_of(&bridge, 2).await;
    let tags: Vec<&str> = replies.iter().map(|r| r.tag).collect();
    assert_eq!(tags, ["first", "second"]);
    assert_eq!(bridge.state().settings.poll_secs, 120, "undone");
    assert!(woken.load(std::sync::atomic::Ordering::SeqCst) >= 2);
    bridge.close();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        client.submit(poll_secs(200), Origin::Cli).await,
        Err(SyncError::SessionClosed)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_attached_bridge_submits_with_its_origin_and_leaves_the_owner_running() {
    let dir = profile("bridge");
    let session = open(Arc::default()).await;
    let server = IpcServer::listen(&dir, session.client()).unwrap();
    let client = IpcClient::connect(&IpcEndpoint::for_profile(&dir))
        .await
        .unwrap();
    let (mut bridge, task) =
        app_sync::Bridge::attach(Connection::Attached(client), Origin::Tui, Arc::new(|| {}))
            .await
            .unwrap();
    bridge.submit(7_u8, poll_secs(300));
    let replies = replies_of(&bridge, 1).await;
    assert_eq!(replies[0].tag, 7);
    assert!(replies[0].result.as_ref().unwrap().is_ok());
    assert_eq!(session.state().settings.poll_secs, 300);
    // The owner's own change shows up in the attached bridge's state.
    session
        .submit(poll_secs(400), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while bridge.state().settings.poll_secs != 400 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("the owner's change arrived");
    assert!(!bridge.is_gone());
    // Another attached bridge notices when the owner goes away.
    let other = IpcClient::connect(&IpcEndpoint::for_profile(&dir))
        .await
        .unwrap();
    let (watching, _watch_task) =
        app_sync::Bridge::<()>::attach(Connection::Attached(other), Origin::Tui, Arc::new(|| {}))
            .await
            .unwrap();
    bridge.close();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert!(
        session.submit(poll_secs(500), Origin::Cli).await.is_ok(),
        "closing an attached bridge leaves the owner's session running"
    );
    drop(server);
    session.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !watching.is_gone() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("the bridge saw the owner go away");
}

#[tokio::test]
async fn the_protocol_runs_over_any_byte_stream() {
    let session = open(Arc::default()).await;
    let (near, far) = tokio::io::duplex(1 << 16);
    tokio::spawn(ipc::serve_stream(far, session.client()));
    let rpc = app_sync::rpc::RpcClient::new(near);
    let reply = rpc
        .call(
            "submit",
            json!({ "origin": "cli", "command": { "cmd": "set_setting", "args": { "key": "poll_secs", "value": 300 } } }),
        )
        .await
        .unwrap();
    assert_eq!(reply["ok"]["name"], "set_setting");
    let state = rpc.call("state", json!(null)).await.unwrap();
    assert_eq!(state["settings"]["poll_secs"], 300);
    let refused = rpc
        .call(
            "submit",
            json!({ "origin": "gui", "command": { "cmd": "undo" } }),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            app_sync::rpc::CallError::Refused { code: -32602, .. }
        ),
        "{refused:?}"
    );

    // A second stream watches.
    let (near, far) = tokio::io::duplex(1 << 16);
    tokio::spawn(ipc::serve_stream(far, session.client()));
    let (result, mut changes) = app_sync::rpc::RpcClient::new(near)
        .into_notifications("watch", json!(null))
        .await
        .unwrap();
    assert!(result.is_null());
    session
        .submit(poll_secs(600), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let (method, _) = tokio::time::timeout(Duration::from_secs(5), changes.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(method, "changed");
    session.shutdown().await.unwrap();
}

/// An owner of another build on `dir`'s socket: it holds the lock and answers `hello` with
/// `answer` (a JSON-RPC `result` or `error` object), then whatever comes next with an error.
#[cfg(unix)]
async fn foreign_owner(dir: &std::path::Path, answer: serde_json::Value) -> ProfileLock {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
    let lock = ProfileLock::try_acquire(&dir.join("lock"))
        .unwrap()
        .unwrap();
    let listener = tokio::net::UnixListener::bind(dir.join("dail.sock")).unwrap();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let answer = answer.clone();
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut lines = BufReader::new(read).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
                    let mut reply = json!({ "jsonrpc": "2.0", "id": request["id"] });
                    for (key, value) in answer.as_object().unwrap() {
                        reply[key] = value.clone();
                    }
                    let mut text = reply.to_string();
                    text.push('\n');
                    if write.write_all(text.as_bytes()).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    lock
}

#[cfg(unix)]
#[tokio::test]
async fn an_owner_of_another_build_is_refused_at_once_with_both_versions() {
    let other = profile("other-build");
    let _lock = foreign_owner(
        &other,
        json!({ "result": { "version": "0.0.1", "schema": "0".repeat(32) } }),
    )
    .await;
    let started = std::time::Instant::now();
    let err = ipc::claim(&other).await.unwrap_err();
    assert!(
        started.elapsed() < ipc::CLAIM_TIMEOUT / 2,
        "no waiting for a busy profile"
    );
    let text = err.to_string();
    assert!(text.starts_with("ERR214: "), "{text}");
    assert!(text.contains("version 0.0.1;"), "{text}");
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
    assert!(text.contains(&other.display().to_string()), "{text}");

    // A build older than the check does not know `hello`.
    let older = profile("older-build");
    let _lock = foreign_owner(
        &older,
        json!({ "error": { "code": -32601, "message": "unknown method `hello`" } }),
    )
    .await;
    let err = IpcClient::connect(&IpcEndpoint::for_profile(&older))
        .await
        .unwrap_err();
    assert!(
        matches!(&err, SyncError::OwnerMismatch { owner, .. } if owner == "older"),
        "{err}"
    );
}

#[tokio::test]
async fn hello_answers_this_builds_version_and_wire_schema() {
    let dir = profile("hello");
    let session = open(Arc::default()).await;
    let _server = IpcServer::listen(&dir, session.client()).unwrap();
    // `connect` says hello and accepts its own build.
    IpcClient::connect(&IpcEndpoint::for_profile(&dir))
        .await
        .unwrap();
    let schema = ipc::wire_schema();
    assert_eq!(schema.len(), 32);
    assert!(schema.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(schema, ipc::wire_schema(), "stable within a build");
}
