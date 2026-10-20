//! The undo-send window on a virtual clock: a `Session` on the memory backend with tokio's
//! paused time. Undo inside the window cancels the send; once the window is over the message
//! has left and nothing can unsend it.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use app_core::{
    AccountId, Addr, ApprovalId, Command, CommandReceipt, CoreError, DraftId, Origin,
    ProviderPreset, Secret, SettingKey,
};
use app_sync::{Backend, CounterSeeds, Session, SubmitResult, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};

async fn open(backend: &Arc<MemoryBackend>) -> Session {
    Session::open(
        Arc::clone(backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

async fn submit(session: &Session, cmd: Command, origin: Origin) -> SubmitResult {
    let result = session.submit(cmd, origin).await.unwrap();
    session.flush().await.unwrap();
    result
}

async fn run(session: &Session, cmd: Command) -> CommandReceipt {
    submit(session, cmd, Origin::Gui).await.unwrap()
}

/// Lets `ms` of virtual time pass and waits for the effects that timers caused.
async fn elapse(session: &Session, ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
    session.flush().await.unwrap();
}

/// alice's account (synced) and a draft to Bob.
async fn ready(backend: &Arc<MemoryBackend>) -> (Session, AccountId, DraftId) {
    let session = open(backend).await;
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
    let account: AccountId = serde_json::from_value(added.data["id"].clone()).unwrap();
    run(&session, Command::SyncNow { account: None }).await;
    let created = run(
        &session,
        Command::CreateDraft {
            account,
            to: vec![Addr {
                name: None,
                email: "bob@example.org".into(),
            }],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Lunch?".into(),
            body: "Noon?".into(),
        },
    )
    .await;
    let draft = serde_json::from_value(created.data["id"].clone()).unwrap();
    (session, account, draft)
}

fn backend() -> Arc<MemoryBackend> {
    Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ))
}

#[tokio::test(start_paused = true)]
async fn undo_inside_the_window_cancels_the_send() {
    let backend = backend();
    let (session, _, draft) = ready(&backend).await;
    run(&session, Command::SendDraft { id: draft }).await;
    elapse(&session, 9_999).await;
    assert!(backend.sent().is_empty());
    let undo = run(&session, Command::Undo {}).await;
    assert_eq!(undo.data["undone"], "Send");
    // The window's timer fires, but the send was cancelled.
    elapse(&session, 60_000).await;
    assert!(backend.sent().is_empty());
    let state = session.state();
    let back = &state.drafts[&draft];
    assert!(!back.is_outgoing());
    assert_eq!(back.send_error, None);

    // Editable again, and redo sends it with a fresh window.
    run(
        &session,
        Command::UpdateDraft {
            id: draft,
            to: None,
            cc: None,
            bcc: None,
            subject: None,
            body: Some("Noon, at the usual place?".into()),
        },
    )
    .await;
    run(&session, Command::Undo {}).await;
    run(&session, Command::Redo {}).await;
    run(&session, Command::SendDraft { id: draft }).await;
    elapse(&session, 10_000).await;
    let sent = backend.sent();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].raw.contains("Noon, at the usual place?"));
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn after_the_window_the_send_is_irreversible() {
    let backend = backend();
    let (session, _, draft) = ready(&backend).await;
    run(
        &session,
        Command::SetSetting {
            key: SettingKey::PollSecs,
            value: 3_600.into(),
        },
    )
    .await;
    run(&session, Command::SendDraft { id: draft }).await;
    elapse(&session, 10_000).await;
    assert_eq!(backend.sent().len(), 1);
    assert!(session.state().drafts.is_empty());

    // Undo skips everything about the sent draft: it reverts the step before it.
    let undo = run(&session, Command::Undo {}).await;
    assert_eq!(undo.data["undone"], "Set poll_secs");
    assert_eq!(backend.sent().len(), 1, "nothing is unsent");
    let err = submit(&session, Command::CancelSend { id: draft }, Origin::Gui)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "draft"));
    // Redo only re-applies the setting; the message does not go out twice.
    run(&session, Command::Redo {}).await;
    elapse(&session, 60_000).await;
    assert_eq!(backend.sent().len(), 1);
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn the_window_follows_the_undo_send_setting() {
    let backend = backend();
    let (session, _, draft) = ready(&backend).await;
    run(
        &session,
        Command::SetSetting {
            key: SettingKey::UndoSendMs,
            value: 3_000.into(),
        },
    )
    .await;
    run(&session, Command::SendDraft { id: draft }).await;
    elapse(&session, 2_999).await;
    assert!(backend.sent().is_empty());
    elapse(&session, 1).await;
    assert_eq!(backend.sent().len(), 1);
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn agents_can_cancel_but_only_ask_to_send() {
    let backend = backend();
    let (session, _, draft) = ready(&backend).await;
    let asked = submit(&session, Command::SendDraft { id: draft }, Origin::Mcp)
        .await
        .unwrap();
    assert_eq!(asked.data["state"], "awaiting_approval");
    elapse(&session, 60_000).await;
    assert!(backend.sent().is_empty(), "nothing leaves without the user");
    let approval: ApprovalId =
        serde_json::from_value(asked.data["approval"]["id"].clone()).unwrap();
    let err = submit(
        &session,
        Command::ApproveRequest { id: approval },
        Origin::Mcp,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }));
    submit(
        &session,
        Command::ApproveRequest { id: approval },
        Origin::Gui,
    )
    .await
    .unwrap();
    // An agent may stop a send the user scheduled.
    submit(&session, Command::CancelSend { id: draft }, Origin::Mcp)
        .await
        .unwrap();
    elapse(&session, 60_000).await;
    assert!(backend.sent().is_empty());
    run(&session, Command::SendDraft { id: draft }).await;
    elapse(&session, 10_000).await;
    assert_eq!(backend.sent().len(), 1);
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn closing_inside_the_window_never_sends() {
    let backend = backend();
    let (session, _, draft) = ready(&backend).await;
    run(&session, Command::SendDraft { id: draft }).await;
    elapse(&session, 5_000).await;
    session.shutdown().await.unwrap();
    assert!(backend.sent().is_empty(), "timers stop with the session");

    // The next session starts from the store: the send is cancelled, not made later.
    let session = open(&backend).await;
    elapse(&session, 60_000).await;
    assert!(backend.sent().is_empty());
    let state = session.state();
    let draft = &state.drafts[&draft];
    assert!(!draft.is_outgoing());
    assert!(
        draft
            .send_error
            .as_deref()
            .unwrap()
            .contains("cancelled by a restart")
    );
    session.shutdown().await.unwrap();
}
