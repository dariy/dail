//! Offline triage (ROADMAP M3): with the server unreachable, triage still works locally and the
//! ops queue up; once the server is back, the queue drains and the server matches what the user
//! sees. A proptest checks this for random sessions, including undo of everything.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use app_core::{
    AppState, Command, CommandReceipt, MessageId, Origin, ProviderPreset, Secret, ThreadId,
};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{FakeAccount, FakeMailbox, FakeMessage, Fixture, MemoryBackend};
use proptest::prelude::*;

/// What a mailbox holds, as `(mailbox path, Message-ID, seen, flagged)`, sorted.
type Picture = Vec<(String, String, bool, bool)>;

fn raw(id: &str, parent: Option<&str>, subject: &str) -> String {
    let reply = parent
        .map(|p| format!("In-Reply-To: <{p}>\r\nReferences: <{p}>\r\n"))
        .unwrap_or_default();
    format!(
        "Message-ID: <{id}>\r\n{reply}From: Bob <bob@example.org>\r\nTo: alice@example.com\r\n\
         Subject: {subject}\r\nDate: Tue, 14 Nov 2023 09:41:00 +0000\r\n\r\nAbout {subject}.\r\n"
    )
}

/// Hostinger-style server: INBOX with four messages (1 and 2 form a thread), Archive, Trash.
fn fixture() -> Fixture {
    let message = |uid, flags: &[&str], id, parent, subject| FakeMessage {
        uid,
        flags: flags.iter().map(|f| (*f).to_owned()).collect(),
        raw: raw(id, parent, subject),
    };
    let mut inbox = FakeMailbox::new("INBOX");
    inbox.special_use = Some("inbox".into());
    inbox.messages = vec![
        message(1, &["\\Seen"], "a@x", None, "Plans"),
        message(2, &[], "b@x", Some("a@x"), "Re: Plans"),
        message(3, &[], "c@x", None, "Lunch"),
        message(4, &["\\Seen", "\\Flagged"], "d@x", None, "Report"),
    ];
    let mut archive = FakeMailbox::new("Archive");
    archive.special_use = Some("archive".into());
    let mut trash = FakeMailbox::new("Trash");
    trash.special_use = Some("trash".into());
    Fixture {
        accounts: vec![FakeAccount {
            email: "alice@example.com".into(),
            password: None,
            mailboxes: vec![inbox, archive, trash],
            smtp_saves_to: None,
        }],
    }
}

/// The server's mail.
fn server_picture(backend: &MemoryBackend) -> Picture {
    let mut picture: Picture = backend.server().accounts[0]
        .mailboxes
        .iter()
        .flat_map(|mailbox| {
            mailbox.messages.iter().map(|m| {
                let summary = app_io::mime::summarize_headers(m.raw.as_bytes());
                let flags = app_io::mime::parse_flags(m.flags.iter().map(String::as_str));
                (
                    mailbox.path.clone(),
                    summary.message_id.unwrap_or_default(),
                    flags.seen,
                    flags.flagged,
                )
            })
        })
        .collect();
    picture.sort();
    picture
}

/// What the user sees: every cached message where it is shown.
fn local_picture(state: &AppState) -> Picture {
    let mut picture: Picture = state
        .messages
        .values()
        .map(|m| {
            (
                state.mailboxes[&m.location()].path.clone(),
                m.message_id_hdr.clone().unwrap_or_default(),
                m.flags.seen,
                m.flags.flagged,
            )
        })
        .collect();
    picture.sort();
    picture
}

async fn run(session: &Session, cmd: Command) -> CommandReceipt {
    let receipt = session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
    receipt
}

/// A synced session on [`fixture`].
async fn synced() -> (Arc<MemoryBackend>, Session) {
    let backend = Arc::new(MemoryBackend::from_fixture(fixture()));
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
    (backend, session)
}

/// Back online: a sync shows the server is reachable, waiting ops run, and every op and fetch
/// they cause completes.
async fn reconnect(backend: &MemoryBackend, session: &Session) {
    backend.set_offline(false);
    run(session, Command::SyncNow { account: None }).await;
    // Ops blocked behind a placeholder wait for fetches that the drain itself started.
    for _ in 0..5 {
        if session.state().outbox.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(301)).await;
        session.flush().await.unwrap();
    }
}

fn messages(state: &AppState) -> Vec<(String, MessageId, ThreadId)> {
    let mut list: Vec<_> = state
        .messages
        .values()
        .map(|m| (m.subject.clone(), m.id, m.thread))
        .collect();
    list.sort();
    list
}

fn find(state: &AppState, subject: &str) -> (MessageId, ThreadId) {
    messages(state)
        .into_iter()
        .find(|(s, _, _)| s == subject)
        .map(|(_, id, thread)| (id, thread))
        .unwrap_or_else(|| panic!("no message {subject}"))
}

#[tokio::test(start_paused = true)]
async fn triage_offline_then_sync_when_the_server_is_back() {
    let (backend, session) = synced().await;
    let initial = server_picture(&backend);
    assert_eq!(local_picture(&session.state()), initial);

    backend.set_offline(true);
    let state = session.state();
    let (lunch, lunch_thread) = find(&state, "Lunch");
    let (_, plans) = find(&state, "Plans");
    let (report, _) = find(&state, "Report");
    run(
        &session,
        Command::MarkRead {
            ids: vec![lunch_thread],
            messages: vec![],
        },
    )
    .await;
    run(
        &session,
        Command::Star {
            ids: vec![],
            messages: vec![lunch],
        },
    )
    .await;
    run(
        &session,
        Command::ArchiveThreads {
            ids: vec![plans],
            messages: vec![],
        },
    )
    .await;
    run(
        &session,
        Command::TrashThreads {
            ids: vec![],
            messages: vec![report],
        },
    )
    .await;
    run(
        &session,
        Command::Unstar {
            ids: vec![],
            messages: vec![report],
        },
    )
    .await;
    // Changed my mind about the star.
    let undo = run(&session, Command::Undo {}).await;
    assert_eq!(undo.data["undone"], "Unstar (1 message)");

    // Everything is visible locally; nothing reached the server.
    assert_eq!(server_picture(&backend), initial);
    let local = local_picture(&session.state());
    assert_ne!(local, initial);
    let pending = run(&session, Command::ListPendingOps { account: None }).await;
    let ops = pending.data["ops"].as_array().unwrap();
    assert!(!ops.is_empty());
    assert_eq!(ops[0]["attempts"], 1);
    assert!(
        ops[0]["last_error"]
            .as_str()
            .unwrap()
            .contains("network is unreachable")
    );
    // The unstar was never sent: undo dropped it instead of queueing a second op.
    assert!(ops.iter().all(|op| op["detail"]["value"] != false));

    // Still offline: back-off retries keep failing without losing anything.
    tokio::time::sleep(Duration::from_secs(60)).await;
    session.flush().await.unwrap();
    assert_eq!(server_picture(&backend), initial);
    assert_eq!(local_picture(&session.state()), local);

    reconnect(&backend, &session).await;
    let state = session.state();
    assert!(state.outbox.is_empty(), "{:?}", state.outbox);
    assert_eq!(server_picture(&backend), local);
    assert_eq!(local_picture(&state), local);
    assert!(
        state
            .messages
            .values()
            .all(|m| m.uid != 0 && m.moved_to.is_none()),
        "every message was located on the server"
    );
    session.shutdown().await.unwrap();
}

/// One step of a random session.
#[derive(Debug, Clone)]
enum Step {
    Read(usize, bool),
    Star(usize, bool),
    Archive(usize),
    Trash(usize),
    ToInbox(usize),
    Undo,
    Offline(bool),
    /// Let virtual time pass (timers: retries, polls).
    Wait(u64),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0..4_usize, any::<bool>()).prop_map(|(i, v)| Step::Read(i, v)),
        (0..4_usize, any::<bool>()).prop_map(|(i, v)| Step::Star(i, v)),
        (0..3_usize).prop_map(Step::Archive),
        (0..3_usize).prop_map(Step::Trash),
        (0..3_usize).prop_map(Step::ToInbox),
        Just(Step::Undo),
        any::<bool>().prop_map(Step::Offline),
        (1..400_u64).prop_map(Step::Wait),
    ]
}

/// Applies one step; returns the change in undo depth (+1 recorded, -1 undone).
async fn apply(
    backend: &MemoryBackend,
    session: &Session,
    step: &Step,
    depth: usize,
    messages: &[MessageId],
    threads: &[ThreadId],
) -> isize {
    let one = |i: usize| vec![messages[i]];
    let cmd = match step {
        Step::Read(i, true) => Command::MarkRead {
            ids: vec![],
            messages: one(*i),
        },
        Step::Read(i, false) => Command::MarkUnread {
            ids: vec![],
            messages: one(*i),
        },
        Step::Star(i, true) => Command::Star {
            ids: vec![],
            messages: one(*i),
        },
        Step::Star(i, false) => Command::Unstar {
            ids: vec![],
            messages: one(*i),
        },
        Step::Archive(i) => Command::ArchiveThreads {
            ids: vec![threads[*i]],
            messages: vec![],
        },
        Step::Trash(i) => Command::TrashThreads {
            ids: vec![threads[*i]],
            messages: vec![],
        },
        Step::ToInbox(i) => {
            let state = session.state();
            let inbox = state
                .mailboxes
                .values()
                .find(|m| m.path == "INBOX")
                .unwrap()
                .id;
            Command::MoveThreads {
                ids: vec![threads[*i]],
                messages: vec![],
                to: inbox,
            }
        }
        // Never undo adding the account.
        Step::Undo if depth == 0 => return 0,
        Step::Undo => Command::Undo {},
        Step::Offline(offline) => {
            backend.set_offline(*offline);
            return 0;
        }
        Step::Wait(secs) => {
            tokio::time::sleep(Duration::from_secs(*secs)).await;
            session.flush().await.unwrap();
            return 0;
        }
    };
    let undo = matches!(cmd, Command::Undo {});
    let receipt = run(session, cmd).await;
    if undo {
        -1
    } else {
        isize::from(receipt.undo_label.is_some())
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn random_offline_sessions_converge_and_undo_restores_the_server(
        steps in prop::collection::vec(step(), 1..25)
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .start_paused(true)
            .build()
            .unwrap();
        runtime.block_on(async {
            let (backend, session) = synced().await;
            let initial = server_picture(&backend);
            let state = session.state();
            let messages: Vec<MessageId> = ["Plans", "Re: Plans", "Lunch", "Report"]
                .iter()
                .map(|s| find(&state, s).0)
                .collect();
            let threads: Vec<ThreadId> = ["Plans", "Lunch", "Report"]
                .iter()
                .map(|s| find(&state, s).1)
                .collect();
            let mut depth = 0_usize;
            for step in &steps {
                let delta = apply(&backend, &session, step, depth, &messages, &threads).await;
                depth = depth.saturating_add_signed(delta);
            }
            let local = local_picture(&session.state());
            reconnect(&backend, &session).await;
            let state = session.state();
            prop_assert!(state.outbox.is_empty(), "{:?}", state.outbox);
            prop_assert_eq!(&server_picture(&backend), &local);
            prop_assert_eq!(&local_picture(&state), &local);

            // Undo everything (except adding the account): the server is back where it began.
            for _ in 0..depth {
                run(&session, Command::Undo {}).await;
            }
            reconnect(&backend, &session).await;
            prop_assert!(session.state().outbox.is_empty());
            prop_assert_eq!(&local_picture(&session.state()), &initial);
            prop_assert_eq!(&server_picture(&backend), &initial);
            session.shutdown().await.unwrap();
            Ok(())
        })?;
    }
}
