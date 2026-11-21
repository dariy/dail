//! Remote profiles offline (ADR 0017, SRV-5b): the persisted queue and its replay in order,
//! refusals on replay and `retry_offline`, commands submitted during a replay, idempotent
//! resends across a server restart (persisted answers), `attach_file` uploaded at replay,
//! pre-loaded bodies read offline, and the offline refusals.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AppState, Command, DeviceRole, DraftId, LinkStatus, MailboxRole, Origin, RequestId, ThreadId,
};
use app_sync::Connection;
use app_sync::remote::{RemoteClient, RemoteTarget};
use app_testkit::{Fixture, TestDevice, TestServer};

use crate::remote::{options, preloading, server, starred, temp_dir, thread, until};

/// A user device of `server` with `options`, online.
async fn online_device(server: &TestServer, options: app_sync::RemoteOptions) -> TestDevice {
    let device = server.device(DeviceRole::User, 1, options).await.unwrap();
    assert_eq!(
        device.session.remote().unwrap().wait_settled().await,
        Some(LinkStatus::Online)
    );
    device
}

fn conn(device: &TestDevice) -> Connection {
    device.session.connection()
}

/// The names of the queued commands, oldest first.
fn queued(state: &AppState) -> Vec<&'static str> {
    state
        .link
        .as_ref()
        .unwrap()
        .queued
        .iter()
        .map(|q| q.command.name())
        .collect()
}

/// Whether every message of thread `id` is in the Archive mailbox.
fn archived(state: &AppState, id: ThreadId) -> bool {
    state.threads[&id].messages.iter().all(|m| {
        let message = &state.messages[m];
        let mailbox = message.moved_to.unwrap_or(message.mailbox);
        state.mailboxes[&mailbox].role == Some(MailboxRole::Archive)
    })
}

async fn submit(conn: &Connection, cmd: Command) {
    conn.submit(cmd, Origin::Cli).await.unwrap().unwrap();
}

fn star(id: ThreadId) -> Command {
    Command::Star {
        ids: vec![id],
        messages: Vec::new(),
    }
}

fn unstar(id: ThreadId) -> Command {
    Command::Unstar {
        ids: vec![id],
        messages: Vec::new(),
    }
}

async fn offline(device: &TestDevice) {
    until(&device.session, "the link is offline", |state| {
        state.link.as_ref().unwrap().status == LinkStatus::Offline
    })
    .await;
}

async fn replayed(device: &TestDevice) {
    until(&device.session, "the queue is replayed", |state| {
        let link = state.link.as_ref().unwrap();
        link.status == LinkStatus::Online && link.queued.is_empty()
    })
    .await;
}

#[tokio::test]
async fn an_offline_triage_is_persisted_and_replayed_in_order() {
    let mut server = server("replay-order", Fixture::builtin("basic").unwrap()).await;
    let device = online_device(&server, options()).await;
    let state = device.session.state();
    let (q3, lunch) = (thread(&state, "Q3 numbers"), thread(&state, "Lunch?"));
    let account = *state.accounts.keys().next().unwrap();
    server.stop_listening();
    offline(&device).await;
    let conn = conn(&device);
    // The order matters: the last star command wins.
    submit(&conn, star(q3)).await;
    submit(
        &conn,
        Command::ArchiveThreads {
            ids: vec![lunch],
            messages: Vec::new(),
        },
    )
    .await;
    submit(&conn, unstar(q3)).await;
    // Destructive: refused offline, nothing queued.
    let refused = conn
        .submit(
            Command::EmptyTrash {
                account,
                confirm: true,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.code(), "ERR014", "{refused}");
    assert_eq!(
        queued(&device.session.state()),
        ["star", "archive_threads", "unstar"]
    );
    assert!(archived(&device.session.state(), lunch), "applied at once");
    let before: Vec<RequestId> = device
        .session
        .state()
        .link
        .as_ref()
        .unwrap()
        .queued
        .iter()
        .map(|q| q.request_id)
        .collect();

    // A restart keeps the queue (the device's store has it).
    device.session.flush().await.unwrap();
    let device = device.restart(1_000, options()).await.unwrap();
    let after: Vec<RequestId> = device
        .session
        .state()
        .link
        .as_ref()
        .unwrap()
        .queued
        .iter()
        .map(|q| q.request_id)
        .collect();
    assert_eq!(after, before);

    // The server returns: replayed in order, after a fresh snapshot.
    server.listen_again().await.unwrap();
    replayed(&device).await;
    let session = server.session().unwrap();
    session.flush().await.unwrap();
    let served = session.state();
    assert!(!starred(&served, q3));
    assert!(archived(&served, lunch));
    device.session.connection().flush().await.unwrap();
    let mirror = device.session.state();
    assert!(!starred(&mirror, q3));
    assert!(archived(&mirror, lunch));
    assert!(mirror.link.as_ref().unwrap().rejected.is_empty());
    // The empty queue is in the store too.
    let stored = device.backend.store().snapshot().unwrap();
    assert!(stored.link.unwrap().queued.is_empty());
}

#[tokio::test]
async fn a_refused_command_is_kept_aside_and_the_others_are_delivered() {
    let mut server = server("replay-refused", Fixture::builtin("basic").unwrap()).await;
    let device = online_device(&server, options()).await;
    let state = device.session.state();
    let (q3, lunch) = (thread(&state, "Q3 numbers"), thread(&state, "Lunch?"));
    let account = *state.accounts.keys().next().unwrap();
    let conn = conn(&device);
    let receipt = conn
        .submit(crate::remote::new_draft(account), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    conn.flush().await.unwrap();
    server.stop_listening();
    offline(&device).await;
    submit(&conn, star(q3)).await;
    submit(
        &conn,
        Command::UpdateDraft {
            id: draft,
            to: None,
            cc: None,
            bcc: None,
            subject: Some("Edited offline".into()),
            body: None,
        },
    )
    .await;
    submit(&conn, star(lunch)).await;
    // Meanwhile the draft goes on the server.
    let session = server.session().unwrap().client();
    session
        .submit(
            Command::DiscardDraft {
                id: draft,
                keep_server_copy: false,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    server.listen_again().await.unwrap();
    replayed(&device).await;
    let link = device.session.state().link.clone().unwrap();
    assert_eq!(link.rejected.len(), 1);
    let rejection = &link.rejected[0];
    assert_eq!(rejection.command.name(), "update_draft");
    assert_eq!(rejection.code, "ERR004", "{}", rejection.error);
    assert_eq!(rejection.origin, Origin::Cli);
    assert!(rejection.at_ms > 0);
    session.flush().await.unwrap();
    let served = session.state();
    assert!(starred(&served, q3) && starred(&served, lunch));
    // The snapshot after the refusal leaves no trace of it in the mirror.
    device.session.connection().flush().await.unwrap();
    assert!(!device.session.state().drafts.contains_key(&draft));

    // `retry_offline` queues it again under a new request ID; refused again.
    let first = rejection.request_id;
    submit(&conn, Command::RetryOffline {}).await;
    until(&device.session, "the retry is refused", |state| {
        let link = state.link.as_ref().unwrap();
        link.queued.is_empty() && link.rejected.len() == 1 && link.rejected[0].request_id != first
    })
    .await;
    let rejected = &device.session.state().link.clone().unwrap().rejected;
    assert_eq!(rejected[0].seed, rejection.seed);
}

#[tokio::test]
async fn a_command_submitted_during_the_replay_goes_after_the_queue() {
    let mut server = server("replay-during", Fixture::builtin("basic").unwrap()).await;
    let device = online_device(&server, options()).await;
    let q3 = thread(&device.session.state(), "Q3 numbers");
    server.stop_listening();
    offline(&device).await;
    let conn = conn(&device);
    // A long queue that ends starred.
    for _ in 0..20 {
        submit(&conn, unstar(q3)).await;
        submit(&conn, star(q3)).await;
    }
    server.listen_again().await.unwrap();
    until(&device.session, "the replay has started", |state| {
        state.link.as_ref().unwrap().queued.len() < 40
    })
    .await;
    let replaying = device.session.state().link.as_ref().unwrap().status != LinkStatus::Online;
    // Last: it must win.
    submit(&conn, unstar(q3)).await;
    if replaying {
        assert_eq!(queued(&device.session.state()).last(), Some(&"unstar"));
    }
    replayed(&device).await;
    let session = server.session().unwrap();
    session.flush().await.unwrap();
    assert!(!starred(&session.state(), q3));
    conn.flush().await.unwrap();
    assert!(!starred(&device.session.state(), q3));
}

#[tokio::test]
async fn a_command_whose_connection_is_lost_is_sent_again_once() {
    let mut server = server("replay-lost", Fixture::builtin("basic").unwrap()).await;
    let device = online_device(&server, options()).await;
    let q3 = thread(&device.session.state(), "Q3 numbers");
    let conn = conn(&device);
    // The device may not have noticed yet: the send fails and is queued, or the link is down
    // already and the engine queues it. Either way it waits, applied.
    server.stop_listening();
    submit(&conn, star(q3)).await;
    assert!(starred(&device.session.state(), q3));
    assert_eq!(queued(&device.session.state()), ["star"]);
    server.listen_again().await.unwrap();
    replayed(&device).await;
    let session = server.session().unwrap();
    session.flush().await.unwrap();
    assert!(starred(&session.state(), q3));
}

/// A connection of a device of `server` with `token`, as the link makes one.
async fn client(server: &TestServer, token: &app_core::Secret) -> RemoteClient {
    RemoteClient::connect(RemoteTarget {
        host: "127.0.0.1".into(),
        port: server.addr().port(),
        fingerprint: server.fingerprint().to_owned(),
        token: token.clone(),
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn a_resend_after_a_server_restart_is_answered_from_disk() {
    let mut server = server("answers", Fixture::builtin("basic").unwrap()).await;
    let q3 = thread(&server.session().unwrap().state(), "Q3 numbers");
    let token = server.pair(DeviceRole::User).await.unwrap();
    let first = client(&server, &token).await;
    let device = first.hello().device.id;
    let star_id = RequestId::from_uuid(uuid::Uuid::from_u128(7 << 64));
    let answered = first
        .submit(&star(q3), Origin::Cli, star_id, 7 << 64)
        .await
        .unwrap()
        .unwrap();
    first
        .submit(
            &unstar(q3),
            Origin::Cli,
            RequestId::from_uuid(uuid::Uuid::from_u128(8 << 64)),
            8 << 64,
        )
        .await
        .unwrap()
        .unwrap();
    // A pairing code never goes to disk.
    let pairing = first
        .submit(
            &Command::StartPairing {
                role: DeviceRole::User,
                name: "Tablet".into(),
            },
            Origin::Cli,
            RequestId::from_uuid(uuid::Uuid::from_u128(9 << 64)),
            9 << 64,
        )
        .await
        .unwrap()
        .unwrap();
    let code = pairing.data["code"].as_str().unwrap().to_owned();
    let file = server
        .dir()
        .join("serve")
        .join("answers")
        .join(format!("{}.json", device.as_uuid()));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains(&star_id.to_string()));
    assert!(!text.contains(&code), "{text}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
    }
    drop(first);

    // A new server process: the resend of the star is answered from disk, not run again
    // (that would star the thread the unstar left).
    server.restart().await.unwrap();
    let again = client(&server, &token).await;
    let resent = again
        .submit(&star(q3), Origin::Cli, star_id, 7 << 64)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resent, answered);
    let session = server.session().unwrap();
    session.flush().await.unwrap();
    assert!(!starred(&session.state(), q3));
    // A new request ID runs.
    again
        .submit(
            &star(q3),
            Origin::Cli,
            RequestId::from_uuid(uuid::Uuid::from_u128(10 << 64)),
            10 << 64,
        )
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    assert!(starred(&session.state(), q3));
}

#[tokio::test]
async fn attach_file_offline_is_uploaded_at_replay() {
    let mut server = server("replay-upload", Fixture::builtin("basic").unwrap()).await;
    let device = online_device(&server, options()).await;
    let account = *device.session.state().accounts.keys().next().unwrap();
    let conn = conn(&device);
    let receipt = conn
        .submit(crate::remote::new_draft(account), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    conn.flush().await.unwrap();
    server.stop_listening();
    offline(&device).await;
    let file = temp_dir("replay-upload-file").join("minutes.txt");
    std::fs::write(&file, b"offline minutes").unwrap();
    let local = file.display().to_string();
    submit(
        &conn,
        Command::AttachFile {
            draft,
            path: local.clone(),
            index: None,
        },
    )
    .await;
    // Queued with the local path.
    let link = device.session.state().link.clone().unwrap();
    assert!(matches!(&link.queued[0].command, Command::AttachFile { path, .. } if *path == local));
    server.listen_again().await.unwrap();
    replayed(&device).await;
    let session = server.session().unwrap();
    session.flush().await.unwrap();
    let served = session.state();
    let attached = &served.drafts[&draft].attachments;
    assert_eq!(attached.len(), 1);
    let path = &attached[0].path;
    assert_ne!(path, &local);
    assert!(
        path.contains("uploads") && path.ends_with("minutes.txt"),
        "{path}"
    );
    assert_eq!(std::fs::read(path).unwrap(), b"offline minutes");
}

#[tokio::test]
async fn preloaded_bodies_open_offline() {
    let mut server = server("preload", Fixture::builtin("basic").unwrap()).await;
    let session = server.session().unwrap();
    let state = session.state();
    let q3 = thread(&state, "Q3 numbers");
    // The server has the body (a query fetches it).
    session
        .submit(
            Command::GetThread {
                id: q3,
                include_body: true,
                rich_body: false,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    let message = session.state().threads[&q3].messages[0];
    let app_core::BodyState::Fetched { blob } = session.state().messages[&message].body.clone()
    else {
        panic!("the server has no body");
    };
    let device = online_device(&server, preloading(200)).await;
    // Pre-loaded into the device's own store in the background.
    let wait = async {
        while app_io::Store::get_blob(device.backend.store(), &blob)
            .unwrap()
            .is_none()
        {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), wait)
        .await
        .expect("the body is pre-loaded");
    server.stop_listening();
    offline(&device).await;
    // Offline the mirror answers with the body from the device's store.
    let answer = conn(&device)
        .submit(
            Command::GetMessage {
                id: message,
                include_body: true,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    let text = answer.data["message"]["untrusted_content"]["body"]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{}", answer.data));
    assert!(text.contains("Q3 numbers are attached"), "{text}");
}

#[tokio::test]
async fn after_a_restart_with_the_server_down_the_cached_mail_opens() {
    let mut server = server("restart-cached", Fixture::builtin("basic").unwrap()).await;
    let session = server.session().unwrap();
    let q3 = thread(&session.state(), "Q3 numbers");
    session
        .submit(
            Command::GetThread {
                id: q3,
                include_body: true,
                rich_body: false,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    let message = session.state().threads[&q3].messages[0];
    let app_core::BodyState::Fetched { blob } = session.state().messages[&message].body.clone()
    else {
        panic!("the server has no body");
    };
    let device = online_device(&server, preloading(200)).await;
    let wait = async {
        while app_io::Store::get_blob(device.backend.store(), &blob)
            .unwrap()
            .is_none()
        {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), wait)
        .await
        .expect("the body is pre-loaded");
    let mirrored = device.session.state();
    server.stop_listening();
    offline(&device).await;
    device.session.flush().await.unwrap();
    // dail starts again while the server is unreachable.
    let device = device.restart(1_000, options()).await.unwrap();
    let state = device.session.state();
    assert_eq!(
        state.accounts.keys().collect::<Vec<_>>(),
        mirrored.accounts.keys().collect::<Vec<_>>()
    );
    assert_eq!(state.threads.len(), mirrored.threads.len());
    assert_eq!(state.messages.len(), mirrored.messages.len());
    let answer = conn(&device)
        .submit(
            Command::GetMessage {
                id: message,
                include_body: true,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    let text = answer.data["message"]["untrusted_content"]["body"]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{}", answer.data));
    assert!(text.contains("Q3 numbers are attached"), "{text}");
}

#[tokio::test]
async fn an_offline_draft_stays_in_the_mirror_across_the_reconnect() {
    let mut server = server("replay-draft", Fixture::builtin("basic").unwrap()).await;
    let device = online_device(&server, options()).await;
    let account = *device.session.state().accounts.keys().next().unwrap();
    server.stop_listening();
    offline(&device).await;
    let receipt = conn(&device)
        .submit(crate::remote::new_draft(account), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    let draft: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    // Every state the mirror publishes from now on still has the draft.
    let mut states = device.session.subscribe();
    let watcher = tokio::spawn(async move {
        let mut always = true;
        loop {
            always &= states.borrow_and_update().drafts.contains_key(&draft);
            let online =
                states.borrow().link.as_ref().is_some_and(|link| {
                    link.status == LinkStatus::Online && link.queued.is_empty()
                });
            if online || states.changed().await.is_err() {
                return always;
            }
        }
    });
    server.listen_again().await.unwrap();
    replayed(&device).await;
    assert!(
        watcher.await.unwrap(),
        "the draft vanished at the reconnect"
    );
    conn(&device).flush().await.unwrap();
    assert!(device.session.state().drafts.contains_key(&draft));
    assert!(
        server
            .session()
            .unwrap()
            .state()
            .drafts
            .contains_key(&draft)
    );
}
