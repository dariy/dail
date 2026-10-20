//! Compose & send through a `Session` on the memory backend.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use app_core::{
    AccountId, AttachmentState, Command, CommandReceipt, DraftId, Origin, ProviderPreset, Secret,
};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{FakeAccount, FakeMailbox, Fixture, MemoryBackend};

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

/// A session on the `basic` fixture with alice's (Hostinger) account added and synced.
async fn synced() -> (Arc<MemoryBackend>, Session, AccountId) {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = open(&backend).await;
    let added = run(
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
    let account = serde_json::from_value(added.data["id"].clone()).unwrap();
    run(&session, Command::SyncNow { account: None }).await;
    (backend, session, account)
}

async fn new_draft(session: &Session, account: AccountId) -> DraftId {
    let receipt = run(
        session,
        Command::CreateDraft {
            account,
            to: vec![app_core::Addr {
                name: None,
                email: "bob@example.org".into(),
            }],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Q3".into(),
            body: "Numbers attached.".into(),
        },
    )
    .await;
    serde_json::from_value(receipt.data["id"].clone()).unwrap()
}

#[tokio::test(start_paused = true)]
async fn attached_files_are_checked_by_the_runtime() {
    let (backend, session, account) = synced().await;
    backend.add_file("/home/alice/q3.pdf", b"%PDF-1.4 numbers".to_vec());
    let draft = new_draft(&session, account).await;
    for path in ["/home/alice/q3.pdf", "/home/alice/gone.txt"] {
        run(
            &session,
            Command::AttachFile {
                draft,
                path: path.into(),
                index: None,
            },
        )
        .await;
    }
    let state = session.state();
    let attachments = &state.drafts[&draft].attachments;
    assert_eq!(
        attachments[0].state,
        AttachmentState::Ready {
            size: 16,
            content_type: "application/pdf".into()
        }
    );
    assert!(matches!(
        &attachments[1].state,
        AttachmentState::Failed { error } if error.contains("No such file")
    ));
    // The draft and its attachment states are persisted.
    let saved = backend.store().snapshot().unwrap();
    assert_eq!(saved.drafts[&draft].attachments, *attachments);
    session.shutdown().await.unwrap();
}

/// The raw messages in alice's Drafts folder on the fake server.
fn server_drafts(backend: &MemoryBackend) -> Vec<(Vec<String>, String)> {
    backend
        .server()
        .mailbox("alice@example.com", "Drafts")
        .unwrap()
        .messages
        .iter()
        .map(|m| (m.flags.clone(), m.raw.clone()))
        .collect()
}

async fn idle(session: &Session) {
    tokio::time::sleep(Duration::from_millis(5_001)).await;
    session.flush().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn idle_drafts_are_saved_to_the_drafts_mailbox_and_replaced_on_change() {
    let (backend, session, account) = synced().await;
    let draft = new_draft(&session, account).await;
    assert!(server_drafts(&backend).is_empty(), "not before it is idle");
    idle(&session).await;
    let saved = server_drafts(&backend);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].0, ["\\Seen", "\\Draft"]);
    let message_id = session.state().drafts[&draft].message_id_hdr.clone();
    let parsed = app_io::mime::parse_message(saved[0].1.as_bytes());
    assert_eq!(
        parsed.headers.message_id.as_deref(),
        Some(message_id.as_str())
    );
    assert_eq!(parsed.headers.subject, "Q3");
    assert!(!session.state().drafts[&draft].needs_save());

    run(
        &session,
        Command::UpdateDraft {
            id: draft,
            to: None,
            cc: None,
            bcc: None,
            subject: Some("Q3, final".into()),
            body: None,
        },
    )
    .await;
    idle(&session).await;
    let saved = server_drafts(&backend);
    assert_eq!(saved.len(), 1, "the old copy is replaced");
    assert_eq!(
        app_io::mime::parse_message(saved[0].1.as_bytes())
            .headers
            .subject,
        "Q3, final"
    );

    run(&session, Command::DiscardDraft { id: draft }).await;
    assert!(server_drafts(&backend).is_empty(), "discard deletes it");
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn shutdown_saves_unsaved_drafts() {
    let (backend, session, account) = synced().await;
    new_draft(&session, account).await;
    session.shutdown().await.unwrap();
    assert_eq!(server_drafts(&backend).len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_reply_is_saved_with_its_quote_and_attachments() {
    let (backend, session, _account) = synced().await;
    backend.add_file("/home/alice/q3.csv", b"q,revenue\n3,42\n".to_vec());
    let q3 = session
        .state()
        .messages
        .values()
        .find(|m| m.subject == "Q3 numbers")
        .unwrap()
        .id;
    let reply = run(
        &session,
        Command::Reply {
            message: q3,
            body: Some("Thanks!".into()),
        },
    )
    .await;
    let draft: DraftId = serde_json::from_value(reply.data["id"].clone()).unwrap();
    run(
        &session,
        Command::AttachFile {
            draft,
            path: "/home/alice/q3.csv".into(),
            index: None,
        },
    )
    .await;
    idle(&session).await;
    let saved = server_drafts(&backend);
    let parsed = app_io::mime::parse_message(saved[0].1.as_bytes());
    assert_eq!(
        parsed.headers.in_reply_to.as_deref(),
        Some("q3@example.org")
    );
    assert!(
        parsed.text.starts_with(
            "Thanks!\n\nOn 2023-11-14 09:41 UTC, Bob <bob@example.org> wrote:\n> Hi Alice"
        ),
        "{}",
        parsed.text
    );
    assert_eq!(parsed.attachments.len(), 1);
    assert_eq!(parsed.attachments[0].filename.as_deref(), Some("q3.csv"));
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_failed_save_is_retried_when_the_server_is_back() {
    let (backend, session, account) = synced().await;
    backend.set_offline(true);
    let draft = new_draft(&session, account).await;
    idle(&session).await;
    let state = session.state();
    assert!(state.drafts[&draft].save_error.is_some());
    assert!(server_drafts(&backend).is_empty());
    backend.set_offline(false);
    tokio::time::sleep(Duration::from_secs(61)).await;
    session.flush().await.unwrap();
    assert_eq!(server_drafts(&backend).len(), 1);
    assert_eq!(session.state().drafts[&draft].save_error, None);
    session.shutdown().await.unwrap();
}

fn addr(email: &str) -> app_core::Addr {
    app_core::Addr {
        name: None,
        email: email.into(),
    }
}

/// A draft to Bob, Cc Carol, Bcc the boss.
async fn full_draft(session: &Session, account: AccountId) -> DraftId {
    let receipt = run(
        session,
        Command::CreateDraft {
            account,
            to: vec![addr("bob@example.org")],
            cc: vec![addr("carol@example.org")],
            bcc: vec![addr("boss@example.org")],
            subject: "Q3".into(),
            body: "Numbers attached.".into(),
        },
    )
    .await;
    serde_json::from_value(receipt.data["id"].clone()).unwrap()
}

fn folder(backend: &MemoryBackend, email: &str, path: &str) -> Vec<String> {
    backend
        .server()
        .mailbox(email, path)
        .unwrap()
        .messages
        .iter()
        .map(|m| m.raw.clone())
        .collect()
}

async fn window(session: &Session) {
    tokio::time::sleep(Duration::from_millis(10_001)).await;
    session.flush().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_sent_message_reaches_every_recipient_and_a_copy_lands_in_sent() {
    let (backend, session, account) = synced().await;
    let draft = full_draft(&session, account).await;
    // Saved to Drafts while it is being written.
    idle(&session).await;
    assert_eq!(server_drafts(&backend).len(), 1);
    run(&session, Command::SendDraft { id: draft }).await;
    assert!(backend.sent().is_empty(), "not inside the undo window");
    window(&session).await;

    let sent = backend.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].login, "alice@example.com");
    assert_eq!(sent[0].from, "alice@example.com");
    assert_eq!(
        sent[0].recipients,
        ["bob@example.org", "carol@example.org", "boss@example.org"]
    );
    assert!(
        !sent[0].raw.contains("boss@example.org"),
        "Bcc is not in the message"
    );
    let copies = folder(&backend, "alice@example.com", "Sent");
    assert_eq!(copies.len(), 1, "Hostinger: the client stores the copy");
    assert!(copies[0].contains("Bcc: <boss@example.org>"));
    let parsed = app_io::mime::parse_message(copies[0].as_bytes());
    assert_eq!(parsed.headers.subject, "Q3");
    let state = session.state();
    assert!(state.drafts.is_empty());
    assert!(
        server_drafts(&backend).is_empty(),
        "the Drafts copy is deleted"
    );
    session.shutdown().await.unwrap();
}

fn gmail() -> Fixture {
    let mut sent = FakeMailbox::new("[Gmail]/Sent Mail");
    sent.special_use = Some("sent".into());
    let mut inbox = FakeMailbox::new("INBOX");
    inbox.special_use = Some("inbox".into());
    Fixture {
        accounts: vec![FakeAccount {
            email: "me@gmail.com".into(),
            password: Some("app-password".into()),
            mailboxes: vec![inbox, sent],
            smtp_saves_to: Some("[Gmail]/Sent Mail".into()),
        }],
    }
}

async fn gmail_session(password: &str) -> (Arc<MemoryBackend>, Session, AccountId) {
    let backend = Arc::new(MemoryBackend::from_fixture(gmail()));
    let session = open(&backend).await;
    let added = run(
        &session,
        Command::AddAccount {
            preset: ProviderPreset::Gmail,
            email: "me@gmail.com".into(),
            display_name: Some("Me".into()),
            password: Some(Secret::new(password)),
            imap: None,
            smtp: None,
        },
    )
    .await;
    let account = serde_json::from_value(added.data["id"].clone()).unwrap();
    run(&session, Command::SyncNow { account: None }).await;
    (backend, session, account)
}

#[tokio::test(start_paused = true)]
async fn gmail_files_the_sent_copy_itself_so_there_is_exactly_one() {
    let (backend, session, account) = gmail_session("app-password").await;
    let draft = full_draft(&session, account).await;
    run(&session, Command::SendDraft { id: draft }).await;
    window(&session).await;
    assert_eq!(backend.sent().len(), 1);
    assert!(
        backend.sent()[0]
            .raw
            .contains("From: \"Me\" <me@gmail.com>"),
        "{}",
        backend.sent()[0].raw
    );
    assert_eq!(
        folder(&backend, "me@gmail.com", "[Gmail]/Sent Mail").len(),
        1
    );
    assert!(
        !backend.executed().iter().any(|e| matches!(
            e,
            app_core::Effect::SmtpSend {
                sent_copy: Some(_),
                ..
            }
        )),
        "no APPEND requested"
    );
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_failed_send_gives_the_draft_back_with_the_error() {
    let (backend, session, account) = gmail_session("app-password").await;
    let draft = full_draft(&session, account).await;
    run(&session, Command::SendDraft { id: draft }).await;
    backend.set_offline(true);
    window(&session).await;
    assert!(backend.sent().is_empty());
    let state = session.state();
    let failed = &state.drafts[&draft];
    assert!(!failed.is_outgoing());
    assert!(
        failed
            .send_error
            .as_deref()
            .unwrap()
            .contains("network is unreachable"),
        "{failed:?}"
    );
    // Back online, the user sends it again.
    backend.set_offline(false);
    run(&session, Command::SendDraft { id: draft }).await;
    window(&session).await;
    assert_eq!(backend.sent().len(), 1);
    assert!(session.state().drafts.is_empty());
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_wrong_password_fails_the_send() {
    let (backend, session, account) = gmail_session("wrong").await;
    let draft = full_draft(&session, account).await;
    run(&session, Command::SendDraft { id: draft }).await;
    window(&session).await;
    assert!(backend.sent().is_empty());
    let error = session.state().drafts[&draft].send_error.clone().unwrap();
    assert!(
        error.starts_with("SMTP smtp.gmail.com:465: AUTH failed"),
        "{error}"
    );
    assert!(!error.contains("wrong"), "never the password");
    session.shutdown().await.unwrap();
}
