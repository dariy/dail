//! The host on the desktop, without JNI: a live profile with no accounts, and a synced memory
//! session driven through events and frames.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use app_core::{Command, MailboxRole, Origin};
use app_host::{BackendChoice, BlobRequest, Config, DESKTOP, Host, HostError, Output, Task};
use app_mobile::UiEvent;

fn temp_profile(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn live(dir: &std::path::Path) -> Config {
    Config {
        profile_dir: dir.to_path_buf(),
        cache_dir: None,
        outgoing_dir: None,
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

/// The alerts a host handed out, as JSON.
type Alerts = Arc<std::sync::Mutex<Vec<String>>>;

/// Collects the alerts.
fn alerts() -> (Alerts, app_host::Alert) {
    let got = Arc::new(std::sync::Mutex::new(Vec::new()));
    let inner = Arc::clone(&got);
    (got, Arc::new(move |json| inner.lock().unwrap().push(json)))
}

/// The fixture synced, its inbox open, under a host.
fn synced_host() -> (Host, Arc<AtomicUsize>) {
    let (host, wakes, _, _) = synced_host_with_alerts();
    (host, wakes)
}

/// [`synced_host`], with the fake server and the alerts.
fn synced_host_with_alerts() -> (
    Host,
    Arc<AtomicUsize>,
    Arc<app_testkit::MemoryBackend>,
    Alerts,
) {
    synced_host_over("basic")
}

/// [`synced_host_with_alerts`] over the built-in fixture `name`.
fn synced_host_over(
    name: &str,
) -> (
    Host,
    Arc<AtomicUsize>,
    Arc<app_testkit::MemoryBackend>,
    Alerts,
) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (session, backend) = runtime.block_on(async {
        let fixture = app_testkit::Fixture::builtin(name).unwrap();
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
        (synced.session, synced.backend)
    });
    let (wakes, wake) = counter();
    let (got, alert) = alerts();
    let outgoing = temp_profile(&format!("outgoing-{wakes:p}"));
    (
        Host::with_session(runtime, session, wake, alert, outgoing),
        wakes,
        backend,
        got,
    )
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
    let host = Host::open(&live(&dir), DESKTOP, Arc::clone(&wake), alerts().1).unwrap();
    assert!(host.state().accounts.is_empty());
    let out = host.event(configure(360));
    assert!(out.frame.list.is_none_or(|list| list.total == 0), "no mail");
    assert!(dir.join("state.redb").exists());
    assert!(matches!(
        Host::open(&live(&dir), DESKTOP, Arc::clone(&wake), alerts().1),
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
    let host = Host::open(&live(&dir), DESKTOP, wake, alerts().1).unwrap();
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
    let request: BlobRequest = serde_json::from_str(
        r#"{"pdf_page": {"blob": "b", "part": 1, "page": 0, "width": 800, "tile": {"x": 0, "y": 8, "w": 16, "h": 32}}}"#,
    )
    .unwrap();
    assert_eq!(
        request,
        BlobRequest::PdfPage {
            blob: "b".to_owned(),
            part: 1,
            page: 0,
            width: 800,
            tile: Some(app_host::PdfTile {
                x: 0,
                y: 8,
                w: 16,
                h: 32
            }),
        }
    );
    let request: BlobRequest =
        serde_json::from_str(r#"{"pdf_pages": {"blob": "b", "part": 1}}"#).unwrap();
    assert_eq!(
        request,
        BlobRequest::PdfPages {
            blob: "b".to_owned(),
            part: 1
        }
    );
    let request: BlobRequest =
        serde_json::from_str(r#"{"text": {"blob": "b", "part": 2}}"#).unwrap();
    assert_eq!(
        request,
        BlobRequest::Text {
            blob: "b".to_owned(),
            part: 2
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
        Host::open(&config, DESKTOP, wake, alerts().1),
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

/// Width, height and RGBA length of an image `blob` answered.
fn image_size(bytes: &[u8]) -> (u32, u32) {
    let width = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let height = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(bytes.len(), 8 + (width * height * 4) as usize);
    (width, height)
}

#[test]
fn media_blobs_decode_upright_and_render_pdf_pages() {
    let (host, _, _, _) = synced_host_over("media");
    let state = host.state();
    let message = state
        .messages
        .values()
        .find(|m| m.subject == "Photos and the report")
        .unwrap();
    let id = message.id;
    let thread = state
        .threads
        .values()
        .find(|t| t.messages.contains(&id))
        .unwrap()
        .id;
    let open = serde_json::to_value(Command::OpenThread { id: thread }).unwrap();
    host.debug(app_host::DebugTask::Exec {
        origin: Origin::Cli,
        command: open,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let blob = loop {
        if let app_core::BodyState::Fetched { blob } = &host.state().messages[&id].body {
            break blob.clone();
        }
        assert!(Instant::now() < deadline, "the body was not fetched");
        std::thread::sleep(Duration::from_millis(10));
    };
    let request = |json: serde_json::Value| -> Option<Vec<u8>> {
        let mut json = json;
        let inner = json.as_object_mut().unwrap().values_mut().next().unwrap();
        inner["blob"] = blob.clone().into();
        host.blob(&serde_json::from_value(json).unwrap())
    };

    let photo = request(serde_json::json!({"image": {"part": 2}})).unwrap();
    let (width, height) = image_size(&photo);
    assert!(height > width, "upright: portrait");
    assert!(width.max(height) <= 4096);

    let pages = request(serde_json::json!({"pdf_pages": {"part": 3}})).unwrap();
    let pages: serde_json::Value = serde_json::from_slice(&pages).unwrap();
    assert_eq!(pages["pages"].as_array().unwrap().len(), 3);
    assert_eq!(pages["max_side"], 4096);
    assert_eq!(pages["max_width"], 16384);
    assert_eq!(pages["tile_max"], 1024);

    let page = request(serde_json::json!({"pdf_page": {"part": 3, "page": 0, "width": 800}}));
    assert_eq!(image_size(&page.unwrap()).0, 800);
    let tile = serde_json::json!({"x": 100, "y": 200, "w": 512, "h": 256});
    let page = request(
        serde_json::json!({"pdf_page": {"part": 3, "page": 2, "width": 8000, "tile": tile}}),
    );
    assert_eq!(image_size(&page.unwrap()), (512, 256));

    let past = serde_json::json!({"pdf_page": {"part": 3, "page": 3, "width": 800}});
    assert_eq!(request(past), None);
    let outside = serde_json::json!({"x": 7900, "y": 0, "w": 512, "h": 256});
    let outside =
        serde_json::json!({"pdf_page": {"part": 3, "page": 0, "width": 8000, "tile": outside}});
    assert_eq!(request(outside), None);
    assert_eq!(request(serde_json::json!({"pdf_pages": {"part": 4}})), None);
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
    let host = Host::open(&config, DESKTOP, wake, alerts().1).unwrap();
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
    let state = host.debug(app_host::DebugTask::State).unwrap();
    assert!(state["mailboxes"].is_object(), "{state}");
    host.close();
}

/// The outgoing folder a copy lies in: its folder's parent.
fn host_outgoing(copy: &std::path::Path) -> PathBuf {
    let folder = copy.parent().unwrap();
    assert_eq!(
        folder.file_name().unwrap().len(),
        32,
        "a random folder per file"
    );
    folder.parent().unwrap().to_path_buf()
}

/// A new draft from the composer's button, opened.
fn composing(host: &Host) -> app_core::DraftId {
    host.event(configure(360));
    let out = host.frame();
    host.event(UiEvent::Tap {
        action: out.frame.fab.unwrap().action,
    });
    frame_until(host, |out| out.frame.composer.is_some())
        .frame
        .composer
        .unwrap()
        .draft
}

#[test]
fn a_picked_file_is_copied_under_a_safe_name_and_attached() {
    let (host, _) = synced_host();
    let draft = composing(&host);
    let source = temp_profile("picked");
    std::fs::create_dir_all(&source).unwrap();
    let file = source.join("in");
    std::fs::write(&file, b"hello").unwrap();
    let copy = host.import(draft, "../.no:tes.txt", &file).unwrap();
    assert_eq!(copy.file_name().unwrap(), "notes.txt");
    assert!(copy.starts_with(host_outgoing(&copy)));
    assert_eq!(std::fs::read(&copy).unwrap(), b"hello");
    // The memory backend checks files in its fake file system, so only the chip is asserted.
    let out = frame_until(&host, |out| {
        out.frame
            .composer
            .as_ref()
            .is_some_and(|c| !c.attachments.is_empty())
    });
    assert_eq!(out.frame.composer.unwrap().attachments[0].name, "notes.txt");
    let path = copy.to_string_lossy().into_owned();
    assert!(host.state().drafts[&draft].attachments[0].path == path);
    host.close();
}

#[test]
fn a_file_over_25_mib_is_not_copied_and_says_so() {
    let (host, _) = synced_host();
    let draft = composing(&host);
    let source = temp_profile("large");
    std::fs::create_dir_all(&source).unwrap();
    let file = source.join("big");
    let big = std::fs::File::create(&file).unwrap();
    big.set_len(25 * 1024 * 1024 + 1).unwrap();
    let err = host.import(draft, "big.iso", &file).unwrap_err();
    assert!(matches!(err, HostError::Import(_)), "{err}");
    let out = host.frame();
    assert!(
        out.frame
            .snackbar
            .unwrap()
            .text
            .contains("larger than 25 MiB")
    );
    assert!(host.state().drafts[&draft].attachments.is_empty());
    host.close();
}

#[test]
fn copies_no_draft_names_are_swept() {
    let dir = temp_profile("sweep");
    let named = dir.join("aaaa");
    let orphan = dir.join("bbbb");
    std::fs::create_dir_all(&named).unwrap();
    std::fs::create_dir_all(&orphan).unwrap();
    std::fs::write(named.join("a.txt"), b"a").unwrap();
    std::fs::write(orphan.join("b.txt"), b"b").unwrap();
    let ctx = app_core::Ctx {
        now_ms: 0,
        id_seed: 1 << 64,
        origin: Origin::Cli,
    };
    let fixture = app_testkit::Fixture::builtin("basic").unwrap();
    let synced = app_testkit::synced_state(fixture).unwrap();
    let mut engine = app_core::Engine::new(synced);
    let account = *engine.state().accounts.keys().next().unwrap();
    let created = engine
        .dispatch(
            Command::CreateDraft {
                account,
                to: Vec::new(),
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: String::new(),
                body: String::new(),
            },
            &ctx,
        )
        .unwrap();
    let id = serde_json::from_value(created.receipt.data["id"].clone()).unwrap();
    engine
        .dispatch(
            Command::AttachFile {
                draft: id,
                path: named.join("a.txt").to_string_lossy().into_owned(),
                index: None,
            },
            &ctx,
        )
        .unwrap();
    assert_eq!(app_host::outgoing::sweep(&dir, engine.state()), 1);
    assert!(named.exists() && !orphan.exists());
}

#[test]
fn the_sends_task_waits_until_no_send_is_scheduled() {
    let (host, _) = synced_host();
    assert!(
        host.background(Task::Sends, Duration::from_secs(5))
            .unwrap()
    );
    let draft = composing(&host);
    let exec = |command: serde_json::Value| {
        host.debug(app_host::DebugTask::Exec {
            origin: Origin::Cli,
            command,
        })
        .unwrap()
    };
    exec(
        serde_json::json!({"cmd": "update_draft", "args": {"id": draft, "to": [{"email": "bob@example.org"}]}}),
    );
    exec(serde_json::json!({"cmd": "send_draft", "args": {"id": draft}}));
    assert!(host.state().drafts[&draft].is_outgoing());
    assert!(
        !host
            .background(Task::Sends, Duration::from_millis(200))
            .unwrap()
    );
    exec(serde_json::json!({"cmd": "cancel_send", "args": {"id": draft}}));
    assert!(
        host.background(Task::Sends, Duration::from_secs(5))
            .unwrap()
    );
    host.close();
}

/// A message for the fake server's INBOX.
fn arrive(backend: &app_testkit::MemoryBackend, subject: &str) {
    backend.edit_server(|server| {
        let inbox = server.accounts[0]
            .mailboxes
            .iter_mut()
            .find(|m| m.path == "INBOX")
            .unwrap();
        inbox.append(
            Vec::new(),
            format!(
                "From: Dana <dana@example.org>\r\nTo: alice@example.com\r\nSubject: {subject}\r\n\
                 Message-ID: <{subject}@example.org>\r\nDate: Tue, 14 Nov 2023 12:00:00 +0000\r\n\r\nHi\r\n"
            ),
        );
    });
}

#[test]
fn new_mail_alerts_only_while_no_activity_is_visible() {
    let (host, _, backend, got) = synced_host_with_alerts();
    host.event(UiEvent::Visible { visible: true });
    arrive(&backend, "Seen");
    assert!(
        host.background(Task::Check, Duration::from_secs(10))
            .unwrap()
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(got.lock().unwrap().is_empty(), "on screen: no notification");
    host.event(UiEvent::Visible { visible: false });
    arrive(&backend, "Lunch");
    assert!(
        host.background(Task::Check, Duration::from_secs(10))
            .unwrap()
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while got.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "no alert");
        std::thread::sleep(Duration::from_millis(10));
    }
    let note: serde_json::Value = serde_json::from_str(&got.lock().unwrap()[0]).unwrap();
    assert_eq!(note["channel"], "new_mail");
    assert_eq!(note["body"], "Lunch");
    assert_eq!(note["public_title"], "New mail");
    // Its *Mark read* button, passed back as `background` would get it.
    let act: app_mobile::NoteAct =
        serde_json::from_value(note["actions"][1]["act"].clone()).unwrap();
    assert!(host.notification(&act, Duration::from_secs(10)).unwrap());
    let thread = match &act {
        app_mobile::NoteAct::MarkRead { threads } => threads[0],
        other => panic!("{other:?}"),
    };
    let state = host.state();
    assert!(
        state.threads[&thread]
            .messages
            .iter()
            .all(|m| state.messages[m].flags.seen)
    );
    host.close();
}

#[test]
fn push_shows_its_notification_again_when_the_service_asks() {
    let (host, _, _, got) = synced_host_with_alerts();
    assert!(host.background(Task::Push, Duration::from_secs(1)).unwrap());
    assert!(got.lock().unwrap().is_empty(), "push is off");
    host.event(UiEvent::Visible { visible: true });
    let client_state = host.state();
    assert_eq!(client_state.settings.background_sync, 1);
    let out = host.debug(app_host::DebugTask::Exec {
        origin: Origin::Cli,
        command: serde_json::json!({"cmd": "set_setting", "args": {"key": "background_sync", "value": 2}}),
    });
    assert!(out.unwrap()["receipt"]["ok"].as_bool().unwrap());
    // The change itself shows it (visible or not), and so does the service's request.
    let deadline = Instant::now() + Duration::from_secs(10);
    while got.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "no push notification");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(host.background(Task::Push, Duration::from_secs(1)).unwrap());
    let notes = got.lock().unwrap().clone();
    assert_eq!(notes.len(), 2, "{notes:?}");
    let note: serde_json::Value = serde_json::from_str(&notes[1]).unwrap();
    assert_eq!(note["tag"], "push");
    assert_eq!(note["actions"][0]["act"]["act"], "stop_push");
    // The schedule tells Kotlin to start the service.
    let out = frame_until(&host, |out| {
        out.externals
            .iter()
            .any(|e| matches!(e, app_mobile::External::Schedule { sync: 2, .. }))
    });
    assert!(out.frame.revision > 0);
    host.close();
}
