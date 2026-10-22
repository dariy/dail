//! The host on the desktop, without JNI: a live profile with no accounts, and a synced memory
//! session driven through events and frames.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use app_core::{Command, MailboxRole, Origin};
use app_mobile::UiEvent;
use dail::{BackendChoice, BlobRequest, Config, Host, HostError, Output, Task};

fn temp_profile(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-android-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn live(dir: &std::path::Path) -> Config {
    Config {
        profile_dir: dir.to_path_buf(),
        cache_dir: None,
        backend: BackendChoice::Live,
        lang: None,
        version: None,
    }
}

fn counter() -> (Arc<AtomicUsize>, app_sync::Wake) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let inner = Arc::clone(&wakes);
    (
        wakes,
        Arc::new(move || {
            inner.fetch_add(1, Ordering::SeqCst);
        }),
    )
}

/// The fixture synced, its inbox open, under a host.
fn synced_host() -> (Host, Arc<AtomicUsize>) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let session = runtime.block_on(async {
        let fixture = app_testkit::Fixture::builtin("basic").unwrap();
        let synced = app_testkit::synced_session(fixture).await.unwrap();
        let inbox = synced
            .session
            .state()
            .mailboxes
            .values()
            .find(|m| m.role == Some(MailboxRole::Inbox))
            .unwrap()
            .id;
        let _ = synced
            .session
            .submit(Command::OpenMailbox { id: inbox }, Origin::Cli)
            .await
            .unwrap();
        synced.session
    });
    let (wakes, wake) = counter();
    (Host::with_session(runtime, session, wake), wakes)
}

/// Frames until `done` holds (the session answers on its own threads).
fn frame_until(host: &Host, done: impl Fn(&Output) -> bool) -> Output {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let out = host.frame();
        if done(&out) {
            return out;
        }
        assert!(Instant::now() < deadline, "no such frame: {out:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn configure(width_dp: u32) -> UiEvent {
    UiEvent::Configure {
        width_dp,
        height_dp: 800,
        dark: false,
        font_scale: 1.0,
        locale: Some("en-US".to_owned()),
        keyboard: false,
    }
}

#[test]
fn a_live_profile_opens_empty_and_is_locked_while_open() {
    let dir = temp_profile("live");
    let (_, wake) = counter();
    let host = Host::open(&live(&dir), Arc::clone(&wake)).unwrap();
    assert!(host.state().accounts.is_empty());
    let out = host.event(configure(360));
    assert!(out.frame.list.is_none_or(|list| list.total == 0), "no mail");
    assert!(dir.join("state.redb").exists());
    assert!(matches!(
        Host::open(&live(&dir), Arc::clone(&wake)),
        Err(HostError::Locked(_))
    ));
    // Nothing to wait for without accounts.
    assert!(
        host.background(Task::Check, Duration::from_secs(10))
            .unwrap()
    );
    assert!(
        host.background(Task::Flush, Duration::from_secs(10))
            .unwrap()
    );
    host.close();
    drop(host);
    let host = Host::open(&live(&dir), wake).unwrap();
    host.close();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_config_is_json() {
    let config: Config =
        serde_json::from_str(r#"{"profile_dir": "/p", "backend": {"memory": "basic"}}"#).unwrap();
    assert_eq!(config.backend, BackendChoice::Memory("basic".to_owned()));
    let config: Config = serde_json::from_str(r#"{"profile_dir": "/p"}"#).unwrap();
    assert_eq!(config.backend, BackendChoice::Live);
    let task: Task = serde_json::from_str(r#""check""#).unwrap();
    assert_eq!(task, Task::Check);
    let request: BlobRequest =
        serde_json::from_str(r#"{"inline": {"blob": "b", "cid": "logo"}}"#).unwrap();
    assert_eq!(
        request,
        BlobRequest::Inline {
            blob: "b".to_owned(),
            cid: "logo".to_owned()
        }
    );
}

#[cfg(not(feature = "demo"))]
#[test]
fn the_memory_backend_needs_the_demo_feature() {
    let config = Config {
        backend: BackendChoice::Memory("basic".to_owned()),
        ..live(&temp_profile("demo"))
    };
    let (_, wake) = counter();
    assert!(matches!(
        Host::open(&config, wake),
        Err(HostError::NoBackend("memory"))
    ));
}

#[test]
fn events_drive_the_session_and_wake_the_app() {
    let (host, wakes) = synced_host();
    let out = host.event(configure(360));
    let rows = out.frame.list.unwrap().rows;
    assert!(!rows.is_empty());
    let before = wakes.load(Ordering::SeqCst);
    host.event(UiEvent::Tap {
        action: rows[0].action,
    });
    let out = frame_until(&host, |out| out.frame.reader.is_some());
    assert_eq!(out.frame.reader.unwrap().thread, rows[0].thread);
    assert!(out.frame.back, "Back closes the thread");
    assert!(
        wakes.load(Ordering::SeqCst) > before,
        "the change woke the app"
    );
    host.event(UiEvent::Back);
    frame_until(&host, |out| out.frame.list.is_some());
    host.close();
}

#[test]
fn a_swipe_archives_and_asks_for_a_haptic() {
    let (host, _) = synced_host();
    let rows = host.event(configure(360)).frame.list.unwrap().rows;
    let out = host.event(UiEvent::Swipe {
        action: rows[0].swipe_start,
    });
    assert_eq!(out.externals, [app_mobile::External::Haptic]);
    frame_until(&host, |out| {
        out.frame
            .list
            .as_ref()
            .is_some_and(|list| list.rows.iter().all(|r| r.thread != rows[0].thread))
    });
    host.close();
}

#[test]
fn bad_json_and_unknown_blobs_are_errors_not_panics() {
    let (host, _) = synced_host();
    assert!(matches!(
        host.event_json("{\"event\": \"nope\"}"),
        Err(HostError::Json(_))
    ));
    assert!(host.event_json(r#"{"event": "refresh"}"#).is_ok());
    let missing = BlobRequest::Attachment {
        blob: "missing".to_owned(),
        part: 0,
    };
    assert_eq!(host.blob(&missing), None);
    assert!(
        host.background(Task::Check, Duration::from_secs(10))
            .unwrap()
    );
    assert!(
        host.background(Task::Flush, Duration::from_secs(10))
            .unwrap()
    );
    host.close();
}

#[cfg(feature = "demo")]
#[test]
fn the_demo_opens_the_fixture_synced() {
    let config = Config {
        backend: BackendChoice::Memory("basic".to_owned()),
        ..live(&temp_profile("demo"))
    };
    let (_, wake) = counter();
    let host = Host::open(&config, wake).unwrap();
    let out = host.event(configure(1280));
    assert!(out.frame.list.unwrap().total > 0);
    host.close();
}

/// The debug bridge's tasks (tests build with debug assertions): `cli` acts as the user, `mcp`
/// meets the agent policy, and no other origin is taken.
#[test]
fn the_debug_bridge_submits_as_cli_or_mcp_only() {
    let (host, _) = synced_host();
    let exec = |origin: &str, command: serde_json::Value| {
        let task = serde_json::json!({ "exec": { "origin": origin, "command": command } });
        host.debug(serde_json::from_value(task).unwrap())
    };
    let listed = exec("cli", serde_json::json!({ "cmd": "list_mailboxes" })).unwrap();
    assert_eq!(listed["receipt"]["ok"], true, "{listed}");
    assert_eq!(listed["receipt"]["name"], "list_mailboxes");

    let remove = serde_json::json!({ "cmd": "add_account", "args": { "preset": "hostinger", "email": "eve@example.com" } });
    let refused = exec("mcp", remove).unwrap();
    assert_eq!(
        refused["rejected"]["error"], "origin_not_allowed",
        "{refused}"
    );

    for origin in ["gui", "runtime"] {
        let err = exec(origin, serde_json::json!({ "cmd": "list_mailboxes" })).unwrap_err();
        assert!(matches!(err, HostError::Debug(_)), "{origin}: {err}");
    }
    let state = host.debug(dail::DebugTask::State).unwrap();
    assert!(state["mailboxes"].is_object(), "{state}");
    host.close();
}
