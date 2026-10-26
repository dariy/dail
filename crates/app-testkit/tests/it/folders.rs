//! Folder management (UX-14) through a `Session` on the memory backend: `CREATE`, `RENAME`
//! and `DELETE` on the fake server, then the listing that confirms them.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use app_core::{Command, CommandReceipt, MailboxId, Origin, ProviderPreset, Secret};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};

async fn run(session: &Session, cmd: Command) -> CommandReceipt {
    let receipt = session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
    receipt
}

async fn synced() -> (Arc<MemoryBackend>, Session) {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
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

fn server_paths(backend: &MemoryBackend) -> Vec<String> {
    backend.server().accounts[0]
        .mailboxes
        .iter()
        .map(|m| m.path.clone())
        .collect()
}

fn mailbox(session: &Session, path: &str) -> MailboxId {
    session
        .state()
        .mailboxes
        .values()
        .find(|m| m.path == path)
        .unwrap_or_else(|| panic!("no mailbox {path}"))
        .id
}

#[tokio::test(start_paused = true)]
async fn create_rename_and_delete_reach_the_server() {
    let (backend, session) = synced().await;
    let account = *session.state().accounts.keys().next().unwrap();
    run(
        &session,
        Command::CreateFolder {
            account,
            path: "Work".into(),
        },
    )
    .await;
    run(
        &session,
        Command::CreateFolder {
            account,
            path: "Work/Acme".into(),
        },
    )
    .await;
    assert!(server_paths(&backend).contains(&"Work/Acme".to_owned()));
    let work = mailbox(&session, "Work");
    let acme = mailbox(&session, "Work/Acme");

    let renamed = run(
        &session,
        Command::RenameFolder {
            id: work,
            path: "Clients".into(),
        },
    )
    .await;
    assert_eq!(renamed.undo_label.as_deref(), Some("Rename Work"));
    let paths = server_paths(&backend);
    assert!(paths.contains(&"Clients/Acme".to_owned()), "{paths:?}");
    // The listing kept the IDs: same paths after the rename.
    assert_eq!(mailbox(&session, "Clients"), work);
    assert_eq!(mailbox(&session, "Clients/Acme"), acme);
    run(&session, Command::Undo {}).await;
    assert!(server_paths(&backend).contains(&"Work".to_owned()));

    let refused = session
        .submit(
            Command::DeleteFolder {
                id: work,
                confirm: true,
            },
            Origin::Cli,
        )
        .await
        .unwrap();
    assert!(refused.is_err(), "it has a subfolder");
    run(
        &session,
        Command::DeleteFolder {
            id: acme,
            confirm: true,
        },
    )
    .await;
    assert!(!server_paths(&backend).contains(&"Work/Acme".to_owned()));
    assert!(session.state().mailboxes.get(&acme).is_none());
}

#[tokio::test(start_paused = true)]
async fn a_refused_change_is_put_back_by_the_listing() {
    let (backend, session) = synced().await;
    let account = *session.state().accounts.keys().next().unwrap();
    run(
        &session,
        Command::CreateFolder {
            account,
            path: "Later".into(),
        },
    )
    .await;
    let later = mailbox(&session, "Later");
    // Removed behind dail's back, then renamed by dail: the server says no.
    backend.edit_server(|server| server.accounts[0].mailboxes.retain(|m| m.path != "Later"));
    run(
        &session,
        Command::RenameFolder {
            id: later,
            path: "Sooner".into(),
        },
    )
    .await;
    let state = session.state();
    let error = &state.status.sync[&account].folder_error;
    assert!(
        error.as_ref().is_some_and(|e| e.error.contains("RENAME")),
        "{error:?}"
    );
    assert!(
        !state.mailboxes.values().any(|m| m.path == "Sooner"),
        "the listing put the server's folders back"
    );
}
