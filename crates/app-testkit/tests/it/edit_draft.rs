//! `edit_draft` on the memory backend (MISC-44d): the attachments of a server-only draft are
//! written to the fake file system and go out with the draft.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use app_core::draft::AttachmentState;
use app_core::{Command, CommandReceipt, DraftId, Origin, ProviderPreset, Secret};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};

async fn run(session: &Session, cmd: Command) -> CommandReceipt {
    let receipt = session.submit(cmd, Origin::Gui).await.unwrap().unwrap();
    session.flush().await.unwrap();
    receipt
}

#[tokio::test(start_paused = true)]
async fn a_server_drafts_attachments_are_carried_and_sent() {
    let mut fixture = Fixture::builtin("basic").unwrap();
    let drafts = fixture.accounts[0]
        .mailboxes
        .iter_mut()
        .find(|m| m.path == "Drafts")
        .unwrap();
    drafts.append(
        vec!["\\Seen".into(), "\\Draft".into()],
        "Message-ID: <plan@example.com>\r\nFrom: alice@example.com\r\n\
         To: bob@example.org\r\nSubject: Plan\r\nDate: Mon, 13 Nov 2023 10:00:00 +0000\r\n\
         Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\n\
         Content-Type: text/plain\r\n\r\nSee the plan.\r\n--b\r\n\
         Content-Type: text/csv\r\nContent-Disposition: attachment; filename=\"../plan.csv\"\r\n\r\n\
         a,b\r\n--b--\r\n"
            .into(),
    );
    let backend = Arc::new(MemoryBackend::from_fixture(fixture));
    let session = Session::open(
        Arc::clone(&backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();
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
    let message = session
        .state()
        .messages
        .values()
        .find(|m| m.message_id_hdr.as_deref() == Some("plan@example.com"))
        .unwrap()
        .id;
    let edited = run(&session, Command::EditDraft { message }).await;
    let draft: DraftId = serde_json::from_value(edited.data["draft"]["id"].clone()).unwrap();
    session.flush().await.unwrap();

    let state = session.state();
    let local = &state.drafts[&draft];
    assert_eq!(local.loading_from, None);
    assert_eq!(local.body_text.trim(), "See the plan.");
    assert_eq!(local.attachments.len(), 1);
    let attachment = &local.attachments[0];
    assert_eq!(attachment.name, "plan.csv");
    assert_eq!(
        attachment.path,
        format!("/draft-files/{}/plan.csv", draft.as_uuid())
    );
    assert!(matches!(
        attachment.state,
        AttachmentState::Ready { size: 3, .. }
    ));
    assert_eq!(backend.file(&attachment.path).unwrap(), b"a,b");

    run(&session, Command::SendDraft { id: draft }).await;
    tokio::time::sleep(Duration::from_millis(10_000)).await;
    session.flush().await.unwrap();
    let sent = backend.sent();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].raw.contains("plan.csv"), "{}", sent[0].raw);
    assert!(sent[0].raw.contains("See the plan."));
    session.shutdown().await.unwrap();
}
