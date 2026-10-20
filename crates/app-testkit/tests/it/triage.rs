//! Read & triage through a `Session` on the memory backend: bodies in query receipts, remote ops
//! against the fake server.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{Command, CommandReceipt, Origin, ProviderPreset, Secret, ThreadId};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};
use serde_json::json;

async fn open(backend: &Arc<MemoryBackend>) -> Session {
    Session::open(
        Arc::clone(backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

async fn run(session: &Session, cmd: Command) -> CommandReceipt {
    let receipt = session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
    receipt
}

/// A session on the `basic` fixture with alice's account synced.
async fn synced() -> (Arc<MemoryBackend>, Session) {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = open(&backend).await;
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

fn thread(session: &Session, subject: &str) -> ThreadId {
    session
        .state()
        .threads
        .values()
        .find(|t| t.subject == subject)
        .unwrap_or_else(|| panic!("no thread {subject}"))
        .id
}

#[tokio::test(start_paused = true)]
async fn include_body_returns_the_parsed_text_as_untrusted_content() {
    let (_backend, session) = synced().await;
    let q3 = thread(&session, "Q3 numbers");
    // Read mail is not prefetched: the first query starts the download.
    let first = run(
        &session,
        Command::GetThread {
            id: q3,
            include_body: true,
            rich_body: false,
        },
    )
    .await;
    let message = &first.data["messages"][0];
    assert_eq!(message["body"], json!({ "state": "not_fetched" }));
    assert!(message["untrusted_content"].get("body").is_none());

    let second = run(
        &session,
        Command::GetThread {
            id: q3,
            include_body: true,
            rich_body: false,
        },
    )
    .await;
    let message = &second.data["messages"][0];
    assert_eq!(message["body"], json!({ "state": "fetched" }), "no blob id");
    let body = &message["untrusted_content"]["body"];
    assert_eq!(body["text_source"], "plain");
    assert!(
        body["text"]
            .as_str()
            .unwrap()
            .contains("the Q3 numbers are attached"),
        "{body}"
    );
    assert_eq!(body["attachments"], json!([]));

    // Without `include_body` nothing is read from the store.
    let plain = run(
        &session,
        Command::GetMessage {
            id: serde_json::from_value(message["id"].clone()).unwrap(),
            include_body: false,
        },
    )
    .await;
    assert!(
        plain.data["message"]["untrusted_content"]
            .get("body")
            .is_none()
    );
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn rich_body_adds_the_document_of_html_bodies() {
    let mut fixture = Fixture::builtin("basic").unwrap();
    let inbox = fixture.accounts[0]
        .mailboxes
        .iter_mut()
        .find(|m| m.path == "INBOX")
        .unwrap();
    inbox.append(
        vec!["\\Seen".into()],
        "Message-ID: <news@example.net>\r\nFrom: News <news@example.net>\r\n\
         To: alice@example.com\r\nSubject: Weekly\r\nDate: Mon, 13 Nov 2023 10:00:00 +0000\r\n\
         Content-Type: multipart/related; boundary=b\r\n\r\n--b\r\n\
         Content-Type: text/html; charset=utf-8\r\n\r\n\
         <h1>Weekly</h1><p>Read <a href=\"https://example.net/a\">more</a></p>\r\n--b\r\n\
         Content-Type: image/png\r\nContent-ID: <logo@news>\r\n\
         Content-Transfer-Encoding: base64\r\n\r\niVBORw0K\r\n--b--\r\n"
            .into(),
    );
    let backend = Arc::new(MemoryBackend::from_fixture(fixture));
    let session = open(&backend).await;
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
    let weekly = thread(&session, "Weekly");
    let q3 = thread(&session, "Q3 numbers");
    let body =
        |receipt: &CommandReceipt| receipt.data["messages"][0]["untrusted_content"]["body"].clone();
    for id in [weekly, q3] {
        // The first query starts the download.
        run(&session, Command::OpenThread { id }).await;
    }

    let rich = |id| Command::GetThread {
        id,
        include_body: true,
        rich_body: true,
    };
    let html = body(&run(&session, rich(weekly)).await);
    assert_eq!(html["text_source"], "html");
    assert_eq!(
        html["document"],
        json!({"blocks": [
            {"type": "heading", "level": 1, "spans": [{"text": "Weekly"}]},
            {"type": "paragraph", "spans": [
                {"text": "Read "},
                {"text": "more", "link": "https://example.net/a"},
            ]},
        ]})
    );
    // Plain text bodies have no document, and agents don't get one unless they ask.
    assert!(
        body(&run(&session, rich(q3)).await)
            .get("document")
            .is_none()
    );
    let plain = body(
        &run(
            &session,
            Command::GetThread {
                id: weekly,
                include_body: true,
                rich_body: false,
            },
        )
        .await,
    );
    assert!(plain["text"].as_str().unwrap().contains("Weekly"));
    assert!(plain.get("document").is_none());

    // The reader reads inline parts of a stored body by Content-ID.
    let state = session.state();
    let message = state.threads[&weekly].messages[0];
    let app_core::BodyState::Fetched { blob } = &state.messages[&message].body else {
        panic!("not downloaded")
    };
    let client = session.client();
    let part = client
        .inline_part(blob, "logo@news")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(part.content_type, "image/png");
    assert_eq!(part.bytes, b"\x89PNG\r\n");
    assert!(client.inline_part(blob, "other").await.unwrap().is_none());
    assert!(
        client
            .inline_part(&"0".repeat(64), "logo@news")
            .await
            .unwrap()
            .is_none()
    );
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn open_thread_downloads_bodies_into_the_store() {
    let (backend, session) = synced().await;
    let q3 = thread(&session, "Q3 numbers");
    let before = backend.store().blob_count();
    let receipt = run(&session, Command::OpenThread { id: q3 }).await;
    assert_eq!(receipt.data["fetching_bodies"].as_array().unwrap().len(), 1);
    assert_eq!(backend.store().blob_count(), before + 1);
    let state = session.state();
    assert_eq!(state.view.thread, Some(q3));
    assert!(state.status.bodies_in_flight.is_empty());
    session.shutdown().await.unwrap();
}

fn server_flags(backend: &MemoryBackend, path: &str, uid: u32) -> Vec<String> {
    backend
        .server()
        .mailbox("alice@example.com", path)
        .unwrap()
        .messages
        .iter()
        .find(|m| m.uid == uid)
        .unwrap()
        .flags
        .clone()
}

#[tokio::test(start_paused = true)]
async fn flag_changes_reach_the_fake_server_and_undo_reverts_them() {
    let (backend, session) = synced().await;
    let lunch = thread(&session, "Lunch?");
    let receipt = run(
        &session,
        Command::MarkRead {
            ids: vec![lunch],
            messages: vec![],
        },
    )
    .await;
    assert_eq!(receipt.undo_label.as_deref(), Some("Mark read (1 message)"));
    assert_eq!(server_flags(&backend, "INBOX", 2), ["\\Seen"]);
    run(
        &session,
        Command::Star {
            ids: vec![lunch],
            messages: vec![],
        },
    )
    .await;
    assert_eq!(server_flags(&backend, "INBOX", 2), ["\\Seen", "\\Flagged"]);
    assert!(session.state().outbox.is_empty(), "every op completed");

    run(&session, Command::Undo {}).await;
    run(&session, Command::Undo {}).await;
    assert!(server_flags(&backend, "INBOX", 2).is_empty());
    let state = session.state();
    let message = state.messages.values().find(|m| m.uid == 2).unwrap();
    assert!(!message.flags.seen && !message.flags.flagged);
    let persisted = backend.store().snapshot().unwrap();
    assert!(persisted.outbox.is_empty());
    assert_eq!(persisted.messages[&message.id].flags, message.flags);
    session.shutdown().await.unwrap();
}

fn server_subjects(backend: &MemoryBackend, path: &str) -> Vec<String> {
    backend
        .server()
        .mailbox("alice@example.com", path)
        .map(|mailbox| {
            mailbox
                .messages
                .iter()
                .map(|m| app_io::mime::summarize_headers(m.raw.as_bytes()).subject)
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(start_paused = true)]
async fn archive_and_undo_move_mail_on_the_fake_server() {
    let (backend, session) = synced().await;
    let q3 = thread(&session, "Q3 numbers");
    let receipt = run(
        &session,
        Command::ArchiveThreads {
            ids: vec![q3],
            messages: vec![],
        },
    )
    .await;
    assert_eq!(receipt.undo_label.as_deref(), Some("Archive (1 message)"));
    assert_eq!(server_subjects(&backend, "INBOX"), ["Lunch?"]);
    assert_eq!(server_subjects(&backend, "Archive"), ["Q3 numbers"]);
    // The op's fetch of Archive located the moved message under its new UID.
    let state = session.state();
    let archive = state
        .mailboxes
        .values()
        .find(|m| m.path == "Archive")
        .unwrap()
        .id;
    let moved = state
        .messages
        .values()
        .find(|m| m.subject == "Q3 numbers")
        .unwrap();
    assert_eq!(
        (moved.mailbox, moved.uid, moved.moved_to),
        (archive, 1, None)
    );
    assert!(state.outbox.is_empty());

    run(&session, Command::Undo {}).await;
    assert_eq!(server_subjects(&backend, "INBOX"), ["Lunch?", "Q3 numbers"]);
    assert!(server_subjects(&backend, "Archive").is_empty());
    let state = session.state();
    let back = state
        .messages
        .values()
        .find(|m| m.subject == "Q3 numbers")
        .unwrap();
    assert_eq!(back.id, moved.id, "the message keeps its ID");
    assert_eq!(back.uid, 3, "a new UID: UIDs are never reused");
    assert_eq!(state.messages.len(), 2);
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn hostinger_archive_folder_is_created_on_first_use() {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    backend.edit_server(|server| {
        server.accounts[0].mailboxes.retain(|m| m.path != "Archive");
    });
    let session = open(&backend).await;
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
    let lunch = thread(&session, "Lunch?");
    run(
        &session,
        Command::ArchiveThreads {
            ids: vec![lunch],
            messages: vec![],
        },
    )
    .await;
    assert_eq!(server_subjects(&backend, "Archive"), ["Lunch?"]);
    let state = session.state();
    let archive = state
        .mailboxes
        .values()
        .find(|m| m.path == "Archive")
        .unwrap();
    assert_eq!(archive.role, Some(app_core::MailboxRole::Archive));
    assert!(archive.uid_validity.is_some(), "fetched after the move");
    // A later listing keeps the folder (and its ID).
    run(&session, Command::SyncNow { account: None }).await;
    assert!(session.state().mailboxes.contains_key(&archive.id));
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn trash_without_a_trash_folder_is_a_structured_error() {
    let (_backend, session) = synced().await;
    let q3 = thread(&session, "Q3 numbers");
    let err = session
        .submit(
            Command::TrashThreads {
                ids: vec![q3],
                messages: vec![],
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(&err).unwrap()["error"],
        "no_mailbox_for_role"
    );
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_failed_op_is_retried_by_the_session_timer() {
    let (backend, session) = synced().await;
    let lunch = thread(&session, "Lunch?");
    backend.set_offline(true);
    run(
        &session,
        Command::MarkRead {
            ids: vec![lunch],
            messages: vec![],
        },
    )
    .await;
    let op = session.state().outbox.values().next().unwrap().clone();
    assert_eq!(op.attempts, 1);
    assert!(op.last_error.unwrap().contains("network is unreachable"));
    assert!(server_flags(&backend, "INBOX", 2).is_empty());

    // Back online: the retry timer fires after 2 s (virtual time) and the op goes through.
    backend.set_offline(false);
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    session.flush().await.unwrap();
    assert_eq!(server_flags(&backend, "INBOX", 2), ["\\Seen"]);
    assert!(session.state().outbox.is_empty());
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn attachments_are_saved_into_the_fake_file_system() {
    let mut fixture = Fixture::builtin("basic").unwrap();
    let inbox = fixture.accounts[0]
        .mailboxes
        .iter_mut()
        .find(|m| m.path == "INBOX")
        .unwrap();
    inbox.append(
        vec!["\\Seen".into()],
        "Message-ID: <files@example.net>\r\nFrom: Bob <bob@example.org>\r\n\
         To: alice@example.com\r\nSubject: Files\r\nDate: Mon, 13 Nov 2023 10:00:00 +0000\r\n\
         Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\n\
         Content-Type: text/plain\r\n\r\nHere.\r\n--b\r\n\
         Content-Type: text/csv\r\nContent-Disposition: attachment; filename=q3.csv\r\n\r\n\
         a,b\r\n--b--\r\n"
            .into(),
    );
    let backend = Arc::new(MemoryBackend::from_fixture(fixture));
    backend.add_file("/home/alice/q3.csv", "older");
    let session = open(&backend).await;
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
    let files = thread(&session, "Files");
    run(&session, Command::OpenThread { id: files }).await;
    let message = session.state().threads[&files].messages[0];
    run(
        &session,
        Command::SaveAttachments {
            message,
            parts: vec![],
            dir: "/home/alice".into(),
        },
    )
    .await;
    // Previews and "open" read the part from the store; opening writes a private copy.
    let app_core::BodyState::Fetched { blob } = session.state().messages[&message].body.clone()
    else {
        panic!("not downloaded")
    };
    let read = run(
        &session,
        Command::GetMessage {
            id: message,
            include_body: true,
        },
    )
    .await;
    let part = read.data["message"]["untrusted_content"]["body"]["attachments"][0]["part"]
        .as_u64()
        .and_then(|p| usize::try_from(p).ok())
        .unwrap();
    let client = session.client();
    let (name, bytes) = client.attachment_part(&blob, part).await.unwrap().unwrap();
    assert_eq!(
        (name.as_str(), bytes.bytes.as_slice()),
        ("q3.csv", b"a,b".as_slice())
    );
    let copy = client.open_attachment(&blob, part).await.unwrap().unwrap();
    assert_eq!(std::fs::read(&copy).unwrap(), b"a,b");
    assert!(
        copy.file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-q3.csv")
    );
    std::fs::remove_file(&copy).unwrap();
    assert!(client.attachment_part(&blob, 0).await.unwrap().is_none());
    assert_eq!(backend.file("/home/alice/q3.csv").unwrap(), b"older");
    assert_eq!(backend.file("/home/alice/q3 (1).csv").unwrap(), b"a,b");
    let state = session.state();
    assert_eq!(
        state.status.saves[0].state,
        app_core::SaveState::Saved {
            files: vec!["/home/alice/q3 (1).csv".into()]
        }
    );
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn new_mail_arrives_by_push() {
    let backend =
        Arc::new(MemoryBackend::from_fixture(Fixture::builtin("basic").unwrap()).with_push());
    let session = open(&backend).await;
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
    let account = *session.state().accounts.keys().next().unwrap();
    assert_eq!(
        session.state().status.sync[&account].watch,
        app_core::WatchState::Watching
    );
    backend.edit_server(|server| {
        let inbox = server.accounts[0]
            .mailboxes
            .iter_mut()
            .find(|m| m.path == "INBOX")
            .unwrap();
        inbox.append(
            Vec::new(),
            "Message-ID: <push@example.org>\r\nFrom: Dan <dan@example.org>\r\n\
             To: alice@example.com\r\nSubject: Pushed\r\nDate: Tue, 14 Nov 2023 12:00:00 +0000\r\n\r\nHi\r\n"
                .into(),
        );
    });
    // No poll is due: the watch reports the change and the INBOX is fetched.
    let poll = session.state().status.next_poll_ms.unwrap();
    let mut arrived = false;
    for _ in 0..50 {
        tokio::task::yield_now().await;
        session.flush().await.unwrap();
        if session
            .state()
            .threads
            .values()
            .any(|t| t.subject == "Pushed")
        {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "the new message is fetched");
    assert!(
        session.state().status.next_poll_ms == Some(poll),
        "not by the poll"
    );
    assert_eq!(
        session.state().status.sync[&account].watch,
        app_core::WatchState::Watching,
        "watching again"
    );
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_server_without_idle_is_polled() {
    let (backend, session) = synced().await;
    let account = *session.state().accounts.keys().next().unwrap();
    assert_eq!(
        session.state().status.sync[&account].watch,
        app_core::WatchState::Unsupported
    );
    assert!(
        backend
            .executed()
            .iter()
            .any(|e| e.name() == "watch_mailbox")
    );
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_whole_mailbox_is_backfilled_in_the_background() {
    let mut fixture = Fixture::builtin("basic").unwrap();
    let inbox = fixture.accounts[0]
        .mailboxes
        .iter_mut()
        .find(|m| m.path == "INBOX")
        .unwrap();
    for n in 0..300 {
        inbox.append(
            vec!["\\Seen".into()],
            format!(
                "Message-ID: <old{n}@example.org>\r\nFrom: Bob <bob@example.org>\r\n\
                 Subject: Old {n}\r\nDate: Mon, 13 Nov 2023 10:00:00 +0000\r\n\r\nOld\r\n"
            ),
        );
    }
    let backend = Arc::new(MemoryBackend::from_fixture(fixture));
    let session = open(&backend).await;
    run(
        &session,
        Command::SetSetting {
            key: app_core::SettingKey::InitialWindow,
            value: json!(50),
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
    let inbox = session
        .state()
        .mailboxes
        .values()
        .find(|m| m.path == "INBOX")
        .unwrap()
        .id;
    let cached = |session: &Session| app_core::listing::history(&session.state(), inbox).unwrap();
    assert_eq!((cached(&session).cached, cached(&session).wanted), (50, 50));
    run(
        &session,
        Command::SetSyncWindow {
            mailbox: inbox,
            window: Some(0),
        },
    )
    .await;
    let history = cached(&session);
    assert_eq!(
        (history.cached, history.wanted),
        (302, 302),
        "every message"
    );
    assert!(session.state().mailboxes[&inbox].history_complete);
    let batches = backend
        .executed()
        .iter()
        .filter(|e| e.name() == "fetch_older")
        .count();
    assert_eq!(batches, 2, "200, then the last 52");
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn compaction_deletes_only_unused_bodies() {
    let (backend, session) = synced().await;
    let q3 = thread(&session, "Q3 numbers");
    run(&session, Command::OpenThread { id: q3 }).await;
    let orphan =
        app_io::Store::put_blob(backend.store(), b"Subject: gone\r\n\r\nexpunged\r\n").unwrap();
    let before = backend.store().blob_count();
    run(&session, Command::CompactStore {}).await;
    assert_eq!(backend.store().blob_count(), before - 1);
    assert!(
        app_io::Store::get_blob(backend.store(), &orphan)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        session.state().status.compaction,
        Some(app_core::Compaction::Done {
            removed_blobs: 1,
            ..
        })
    ));
    session.shutdown().await.unwrap();
}
