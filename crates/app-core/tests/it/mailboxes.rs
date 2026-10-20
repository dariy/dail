//! Mailbox listing and sync-failure events: reconciliation, role assignment, cleanup.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AccountId, Command, Ctx, Effect, Engine, EntityRef, Event, MailDelta, MailboxRole, Origin,
    PersistDelta, ProviderPreset, RemoteMailbox, SyncIssue,
};

const SEED: u128 = 0x1000_0000_0000_0000;

fn ctx(id_seed: u128) -> Ctx {
    Ctx {
        now_ms: 1_700_000_000_000,
        id_seed,
        origin: Origin::Runtime,
    }
}

fn remote(path: &str, special_use: Option<MailboxRole>) -> RemoteMailbox {
    RemoteMailbox {
        path: path.to_owned(),
        delimiter: Some("/".to_owned()),
        special_use,
        selectable: true,
    }
}

fn engine_with_gmail() -> (Engine, AccountId) {
    let mut engine = Engine::default();
    let cmd = Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "me@gmail.com".into(),
        display_name: None,
        password: None,
        imap: None,
        smtp: None,
    };
    let user = Ctx {
        origin: Origin::Cli,
        ..ctx(SEED)
    };
    engine.dispatch(cmd, &user).unwrap();
    (engine, AccountId::from_uuid(uuid::Uuid::from_u128(SEED)))
}

fn listed(account: AccountId, mailboxes: Vec<RemoteMailbox>) -> Event {
    Event::MailboxesListed { account, mailboxes }
}

#[test]
fn listing_creates_mailboxes_with_roles_and_persists_them() {
    let (mut engine, account) = engine_with_gmail();
    let mut parent = remote("[Gmail]", None);
    parent.selectable = false;
    let outcome = engine
        .ingest(
            listed(
                account,
                vec![
                    remote("INBOX", None),
                    parent,
                    remote("[Gmail]/Sent Mail", Some(MailboxRole::Sent)),
                    remote("[Gmail]/All Mail", None),
                    remote("Receipts", None),
                ],
            ),
            &ctx(2 << 64),
        )
        .unwrap();
    let state = engine.state();
    let roles: Vec<_> = state
        .mailboxes
        .values()
        .map(|m| (m.path.as_str(), m.role))
        .collect();
    assert_eq!(
        roles,
        [
            ("INBOX", Some(MailboxRole::Inbox)),
            ("[Gmail]/Sent Mail", Some(MailboxRole::Sent)),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
            ("Receipts", None),
        ]
    );
    let first = state.mailboxes.values().next().unwrap();
    assert_eq!(first.id.as_uuid().as_u128(), 2 << 64);
    assert_eq!(first.account, account);
    assert_eq!(first.uid_validity, None);
    assert_eq!(outcome.receipt.kind, None);
    assert_eq!(outcome.receipt.data["mailboxes"], 4);
    assert_eq!(
        outcome.effects[0],
        Effect::PersistState {
            delta: PersistDelta::Mail(MailDelta {
                mailboxes: state.mailboxes.values().cloned().collect(),
                ..MailDelta::default()
            }),
        }
    );
    let fetched: Vec<_> = outcome.effects[1..]
        .iter()
        .map(|effect| match effect {
            Effect::FetchEnvelopes { path, .. } => path.as_str(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        fetched,
        ["INBOX", "[Gmail]/Sent Mail", "[Gmail]/All Mail", "Receipts"]
    );
}

#[test]
fn relisting_keeps_ids_and_drops_vanished_mailboxes() {
    let (mut engine, account) = engine_with_gmail();
    engine
        .ingest(
            listed(account, vec![remote("INBOX", None), remote("Old", None)]),
            &ctx(2 << 64),
        )
        .unwrap();
    let inbox = *engine.state().mailboxes.keys().next().unwrap();
    let old = *engine.state().mailboxes.keys().nth(1).unwrap();

    // Same listing again: nothing to persist, only fetches.
    let outcome = engine
        .ingest(
            listed(account, vec![remote("INBOX", None), remote("Old", None)]),
            &ctx(3 << 64),
        )
        .unwrap();
    assert!(
        outcome
            .effects
            .iter()
            .all(|e| matches!(e, Effect::FetchEnvelopes { .. }))
    );

    let outcome = engine
        .ingest(
            listed(
                account,
                vec![
                    remote("INBOX", None),
                    remote("INBOX", None),
                    remote("New", None),
                ],
            ),
            &ctx(4 << 64),
        )
        .unwrap();
    let state = engine.state();
    assert_eq!(state.mailboxes.len(), 2);
    assert!(state.mailboxes.contains_key(&inbox), "INBOX keeps its ID");
    assert!(!state.mailboxes.contains_key(&old));
    let Effect::PersistState {
        delta: PersistDelta::Mail(delta),
    } = &outcome.effects[0]
    else {
        panic!("expected a mail delta");
    };
    assert_eq!(delta.removed_mailboxes, vec![old]);
    assert_eq!(delta.mailboxes.len(), 1);
    assert_eq!(delta.mailboxes[0].path, "New");
    assert!(outcome.receipt.affected.contains(&EntityRef::Mailbox(old)));
}

#[test]
fn events_for_unknown_accounts_change_nothing() {
    let (mut engine, _) = engine_with_gmail();
    let before = engine.state().clone();
    let stranger = AccountId::from_uuid(uuid::Uuid::from_u128(99));
    let outcome = engine
        .ingest(listed(stranger, vec![remote("INBOX", None)]), &ctx(2 << 64))
        .unwrap();
    assert!(outcome.effects.is_empty());
    let failed = Event::SyncFailed {
        account: stranger,
        mailbox: None,
        error: "boom".into(),
    };
    engine.ingest(failed, &ctx(3 << 64)).unwrap();
    assert_eq!(engine.state(), &before);
}

#[test]
fn sync_failures_surface_in_status_until_the_next_listing() {
    let (mut engine, account) = engine_with_gmail();
    let failed = Event::SyncFailed {
        account,
        mailbox: None,
        error: "IMAP LOGIN failed: invalid credentials".into(),
    };
    let outcome = engine.ingest(failed, &ctx(2 << 64)).unwrap();
    assert_eq!(outcome.receipt.name, "sync_failed");
    assert_eq!(
        engine.state().status.sync[&account].last_error,
        Some(SyncIssue {
            error: "IMAP LOGIN failed: invalid credentials".into(),
            mailbox: None,
            at_ms: 1_700_000_000_000,
        })
    );
    let outcome = engine
        .ingest(listed(account, vec![remote("INBOX", None)]), &ctx(3 << 64))
        .unwrap();
    assert_eq!(engine.state().status.sync[&account].last_error, None);
    assert!(outcome.receipt.affected.contains(&EntityRef::Status));
}

#[test]
fn removing_an_account_drops_its_mailboxes() {
    let (mut engine, account) = engine_with_gmail();
    engine
        .ingest(listed(account, vec![remote("INBOX", None)]), &ctx(2 << 64))
        .unwrap();
    let inbox = *engine.state().mailboxes.keys().next().unwrap();
    let outcome = engine
        .dispatch(
            Command::RemoveAccount {
                id: account,
                confirm: true,
            },
            &ctx(3 << 64),
        )
        .unwrap();
    assert!(engine.state().mailboxes.is_empty());
    assert!(engine.state().status.sync.is_empty());
    assert!(outcome.effects.contains(&Effect::PersistState {
        delta: PersistDelta::Mail(MailDelta {
            removed_mailboxes: vec![inbox],
            ..MailDelta::default()
        }),
    }));
}

#[test]
fn list_mailboxes_failure_maps_to_sync_failed() {
    let (engine, account) = engine_with_gmail();
    let imap = engine.state().accounts[&account].imap_login();
    assert_eq!(imap.login, "me@gmail.com");
    let effect = Effect::ListMailboxes { account, imap };
    assert_eq!(effect.name(), "list_mailboxes");
    assert_eq!(
        effect.failure_event("offline".into()),
        Event::SyncFailed {
            account,
            mailbox: None,
            error: "offline".into(),
        }
    );
}
