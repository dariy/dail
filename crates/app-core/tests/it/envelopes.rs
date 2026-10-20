//! Fetched envelopes: caching, threading, flag updates, UIDVALIDITY reset, cleanup.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AccountId, AppState, Command, Ctx, Effect, Engine, Event, Flags, MailboxId, MailboxRole,
    Origin, PersistDelta, ProviderPreset, RemoteEnvelope, RemoteMailbox,
};

const SEED: u128 = 0x1000_0000_0000_0000;
const DAY: i64 = 86_400_000;

fn ctx(n: u128) -> Ctx {
    Ctx {
        now_ms: 1_700_000_000_000,
        id_seed: n << 64,
        origin: Origin::Runtime,
    }
}

fn remote(uid: u32, id: &str, parents: &[&str], subject: &str, day: i64) -> RemoteEnvelope {
    RemoteEnvelope {
        uid,
        message_id_hdr: Some(id.to_owned()),
        in_reply_to: parents.last().map(|p| (*p).to_owned()),
        references: parents.iter().map(|p| (*p).to_owned()).collect(),
        from: Vec::new(),
        to: Vec::new(),
        cc: Vec::new(),
        subject: subject.to_owned(),
        date_ms: day * DAY,
        flags: Flags::default(),
        size: 100,
        has_attachments: false,
    }
}

fn fetched(
    account: AccountId,
    mailbox: MailboxId,
    uid_validity: u32,
    envelopes: Vec<RemoteEnvelope>,
) -> Event {
    Event::EnvelopesFetched {
        account,
        mailbox,
        uid_validity,
        uid_next: None,
        total: 10,
        unread: 4,
        envelopes,
    }
}

/// A Gmail account with INBOX and Sent listed.
fn setup() -> (Engine, AccountId, MailboxId, MailboxId) {
    let mut engine = Engine::default();
    let cmd = Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "me@gmail.com".into(),
        display_name: None,
        password: None,
        imap: None,
        smtp: None,
    };
    engine
        .dispatch(
            cmd,
            &Ctx {
                id_seed: SEED,
                origin: Origin::Cli,
                ..ctx(0)
            },
        )
        .unwrap();
    let account = AccountId::from_uuid(uuid::Uuid::from_u128(SEED));
    let listed = |path: &str, special_use| RemoteMailbox {
        path: path.to_owned(),
        delimiter: Some("/".to_owned()),
        special_use,
        selectable: true,
    };
    engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![
                    listed("INBOX", None),
                    listed("[Gmail]/Sent Mail", Some(MailboxRole::Sent)),
                ],
            },
            &ctx(1),
        )
        .unwrap();
    let ids: Vec<MailboxId> = engine.state().mailboxes.keys().copied().collect();
    (engine, account, ids[0], ids[1])
}

fn thread_of(state: &AppState, mailbox: MailboxId, uid: u32) -> app_core::ThreadId {
    state
        .messages
        .values()
        .find(|m| m.mailbox == mailbox && m.uid == uid)
        .unwrap()
        .thread
}

#[test]
fn listing_requests_a_fetch_per_mailbox() {
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
        ..ctx(0)
    };
    engine.dispatch(cmd, &user).unwrap();
    let account = *engine.state().accounts.keys().next().unwrap();
    let outcome = engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![RemoteMailbox {
                    path: "INBOX".into(),
                    delimiter: None,
                    special_use: None,
                    selectable: true,
                }],
            },
            &ctx(1),
        )
        .unwrap();
    let inbox = *engine.state().mailboxes.keys().next().unwrap();
    let fetch = outcome
        .effects
        .iter()
        .find(|e| e.name() == "fetch_envelopes")
        .unwrap();
    let Effect::FetchEnvelopes {
        mailbox,
        path,
        uid_validity,
        since_uid,
        limit,
        imap,
        ..
    } = fetch
    else {
        panic!("not a fetch");
    };
    assert_eq!(*mailbox, inbox);
    assert_eq!(path, "INBOX");
    assert_eq!((*uid_validity, *since_uid, *limit), (None, None, 500));
    assert_eq!(imap.server.host, "imap.gmail.com");
}

#[test]
fn envelopes_are_cached_threaded_and_persisted() {
    let (mut engine, account, inbox, sent) = setup();
    let outcome = engine
        .ingest(
            fetched(
                account,
                inbox,
                7,
                vec![
                    remote(1, "a@x", &[], "Plan", 1),
                    remote(2, "c@x", &["a@x", "b@x"], "Re: Plan", 3),
                    remote(3, "z@x", &[], "Other", 2),
                ],
            ),
            &ctx(2),
        )
        .unwrap();
    // My reply in Sent links the thread from the middle.
    engine
        .ingest(
            fetched(
                account,
                sent,
                3,
                vec![remote(40, "b@x", &["a@x"], "Re: Plan", 2)],
            ),
            &ctx(3),
        )
        .unwrap();
    let state = engine.state();
    assert_eq!(state.messages.len(), 4);
    assert_eq!(state.threads.len(), 2);
    let plan = thread_of(state, inbox, 1);
    assert_eq!(thread_of(state, inbox, 2), plan);
    assert_eq!(thread_of(state, sent, 40), plan);
    let thread = &state.threads[&plan];
    assert_eq!(thread.subject, "Plan");
    assert_eq!(thread.last_date_ms, 3 * DAY);
    let dates: Vec<i64> = thread
        .messages
        .iter()
        .map(|m| state.messages[m].date_ms)
        .collect();
    assert_eq!(dates, [DAY, 2 * DAY, 3 * DAY], "oldest first");

    let mailbox = &state.mailboxes[&inbox];
    assert_eq!(
        (
            mailbox.uid_validity,
            mailbox.uid_next,
            mailbox.total,
            mailbox.unread
        ),
        (Some(7), Some(4), 10, 4)
    );
    assert_eq!(outcome.receipt.data["added"], 3);
    let [
        Effect::PersistState {
            delta: PersistDelta::Mail(delta),
        },
        prefetch @ ..,
    ] = &outcome.effects[..]
    else {
        panic!("expected a mail delta first");
    };
    // INBOX: bodies of the (all unread) new messages are prefetched, the INBOX is watched, and
    // the older messages the server has (total 10) are backfilled.
    let names: Vec<&str> = prefetch.iter().map(Effect::name).collect();
    assert_eq!(
        names,
        [
            "fetch_body",
            "fetch_body",
            "fetch_body",
            "watch_mailbox",
            "fetch_older"
        ]
    );
    assert_eq!(delta.messages.len(), 3);
    assert_eq!(delta.threads.len(), 2);
    assert_eq!(delta.mailboxes.len(), 1);
}

#[test]
fn the_next_fetch_is_incremental() {
    let (mut engine, account, inbox, _) = setup();
    engine
        .ingest(
            fetched(account, inbox, 7, vec![remote(5, "a@x", &[], "Plan", 1)]),
            &ctx(2),
        )
        .unwrap();
    let outcome = engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![RemoteMailbox {
                    path: "INBOX".into(),
                    delimiter: Some("/".into()),
                    special_use: None,
                    selectable: true,
                }],
            },
            &ctx(3),
        )
        .unwrap();
    let fetch = outcome
        .effects
        .iter()
        .find_map(|e| match e {
            Effect::FetchEnvelopes {
                uid_validity,
                since_uid,
                ..
            } => Some((*uid_validity, *since_uid)),
            _ => None,
        })
        .unwrap();
    assert_eq!(fetch, (Some(7), Some(6)));
}

#[test]
fn known_uids_only_update_flags() {
    let (mut engine, account, inbox, _) = setup();
    engine
        .ingest(
            fetched(account, inbox, 7, vec![remote(1, "a@x", &[], "Plan", 1)]),
            &ctx(2),
        )
        .unwrap();
    let before = engine.state().messages.values().next().unwrap().clone();
    let mut again = remote(1, "a@x", &[], "Changed?", 9);
    again.flags.seen = true;
    let outcome = engine
        .ingest(fetched(account, inbox, 7, vec![again]), &ctx(3))
        .unwrap();
    let after = engine.state().messages.values().next().unwrap();
    assert_eq!(engine.state().messages.len(), 1);
    assert_eq!(after.id, before.id);
    assert!(after.flags.seen);
    assert_eq!(after.subject, "Plan");
    assert_eq!(outcome.receipt.data["flags_updated"], 1);
    assert_eq!(outcome.receipt.data["added"], 0);
}

#[test]
fn a_new_uid_validity_drops_the_cached_mailbox() {
    let (mut engine, account, inbox, _) = setup();
    engine
        .ingest(
            fetched(
                account,
                inbox,
                7,
                vec![
                    remote(1, "a@x", &[], "Plan", 1),
                    remote(2, "b@x", &[], "Old", 1),
                ],
            ),
            &ctx(2),
        )
        .unwrap();
    let old_ids: Vec<_> = engine.state().messages.keys().copied().collect();
    let outcome = engine
        .ingest(
            fetched(account, inbox, 8, vec![remote(1, "a@x", &[], "Plan", 1)]),
            &ctx(3),
        )
        .unwrap();
    let state = engine.state();
    assert_eq!(state.messages.len(), 1);
    assert!(!old_ids.iter().any(|id| state.messages.contains_key(id)));
    assert_eq!(state.threads.len(), 1);
    assert_eq!(state.mailboxes[&inbox].uid_validity, Some(8));
    assert_eq!(outcome.receipt.data["uid_validity_reset"], true);
    let Effect::PersistState {
        delta: PersistDelta::Mail(delta),
    } = &outcome.effects[0]
    else {
        panic!("expected a mail delta");
    };
    assert_eq!(delta.removed_messages.len(), 2);
    assert_eq!(delta.removed_threads.len(), 2);
}

#[test]
fn vanished_mailboxes_and_removed_accounts_take_their_messages() {
    let (mut engine, account, inbox, sent) = setup();
    engine
        .ingest(
            fetched(account, inbox, 7, vec![remote(1, "a@x", &[], "Plan", 1)]),
            &ctx(2),
        )
        .unwrap();
    engine
        .ingest(
            fetched(
                account,
                sent,
                3,
                vec![remote(1, "b@x", &["a@x"], "Re: Plan", 2)],
            ),
            &ctx(3),
        )
        .unwrap();
    // Sent disappears from the listing.
    engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![RemoteMailbox {
                    path: "INBOX".into(),
                    delimiter: Some("/".into()),
                    special_use: None,
                    selectable: true,
                }],
            },
            &ctx(4),
        )
        .unwrap();
    let state = engine.state();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.threads.values().next().unwrap().messages.len(), 1);

    let outcome = engine
        .dispatch(
            Command::RemoveAccount {
                id: account,
                confirm: true,
            },
            &ctx(5),
        )
        .unwrap();
    let state = engine.state();
    assert!(state.messages.is_empty() && state.threads.is_empty() && state.mailboxes.is_empty());
    let delta = outcome
        .effects
        .iter()
        .find_map(|e| match e {
            Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            } => Some(delta),
            _ => None,
        })
        .unwrap();
    assert_eq!(delta.removed_messages.len(), 1);
    assert_eq!(delta.removed_threads.len(), 1);
}

#[test]
fn events_for_unknown_or_foreign_mailboxes_are_ignored() {
    let (mut engine, account, inbox, _) = setup();
    let before = engine.state().clone();
    let stranger = AccountId::from_uuid(uuid::Uuid::from_u128(5));
    let nowhere = MailboxId::from_uuid(uuid::Uuid::from_u128(6));
    let one = || vec![remote(1, "a@x", &[], "Plan", 1)];
    engine
        .ingest(fetched(account, nowhere, 1, one()), &ctx(2))
        .unwrap();
    engine
        .ingest(fetched(stranger, inbox, 1, one()), &ctx(3))
        .unwrap();
    // Only the in-flight counter moves (the fetch is over); the cache is untouched.
    let after = engine.state();
    assert_eq!(after.mailboxes, before.mailboxes);
    assert!(after.messages.is_empty() && after.threads.is_empty());
}

#[test]
fn fetch_failures_name_the_mailbox_and_clear_on_success() {
    let (mut engine, account, inbox, _) = setup();
    let effect = engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![RemoteMailbox {
                    path: "INBOX".into(),
                    delimiter: Some("/".into()),
                    special_use: None,
                    selectable: true,
                }],
            },
            &ctx(2),
        )
        .unwrap()
        .effects
        .into_iter()
        .find(|e| e.name() == "fetch_envelopes")
        .unwrap();
    let failure = effect.failure_event("IMAP EXAMINE INBOX failed: timeout".into());
    engine.ingest(failure, &ctx(3)).unwrap();
    let issue = engine.state().status.sync[&account]
        .last_error
        .clone()
        .unwrap();
    assert_eq!(issue.mailbox, Some(inbox));
    engine
        .ingest(fetched(account, inbox, 7, Vec::new()), &ctx(4))
        .unwrap();
    assert_eq!(engine.state().status.sync[&account].last_error, None);
}
