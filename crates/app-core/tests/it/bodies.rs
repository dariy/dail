//! Flag resync of the cached window, lazy bodies and unread INBOX prefetch.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AccountId, BodyState, Command, Ctx, Effect, Engine, Event, Flags, MailboxId, MailboxRole,
    MessageId, Origin, PersistDelta, ProviderPreset, RemoteEnvelope, RemoteMailbox, UidFlags,
};

const T0: i64 = 1_700_000_000_000;

fn ctx(n: u128) -> Ctx {
    Ctx {
        now_ms: T0,
        id_seed: n << 64,
        origin: Origin::Runtime,
    }
}

fn remote(uid: u32, seen: bool) -> RemoteEnvelope {
    RemoteEnvelope {
        uid,
        message_id_hdr: Some(format!("m{uid}@x")),
        in_reply_to: None,
        references: Vec::new(),
        from: Vec::new(),
        to: Vec::new(),
        cc: Vec::new(),
        subject: format!("Message {uid}"),
        date_ms: i64::from(uid) * 1000,
        flags: Flags {
            seen,
            ..Flags::default()
        },
        size: 1,
        has_attachments: false,
    }
}

fn fetched(account: AccountId, mailbox: MailboxId, envelopes: Vec<RemoteEnvelope>) -> Event {
    Event::EnvelopesFetched {
        account,
        mailbox,
        uid_validity: 7,
        uid_next: None,
        total: 0,
        unread: 0,
        envelopes,
    }
}

/// An account with INBOX and Archive listed.
fn setup() -> (Engine, AccountId, MailboxId, MailboxId) {
    let mut engine = Engine::default();
    let cmd = Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email: "me@example.com".into(),
        display_name: None,
        password: None,
        imap: None,
        smtp: None,
    };
    // The user adds the account (`add_account` is user only).
    let user = Ctx {
        origin: Origin::Cli,
        ..ctx(1)
    };
    engine.dispatch(cmd, &user).unwrap();
    let account = AccountId::from_uuid(uuid::Uuid::from_u128(1 << 64));
    let listed = |path: &str, special_use| RemoteMailbox {
        path: path.into(),
        delimiter: Some("/".into()),
        special_use,
        selectable: true,
    };
    engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![
                    listed("INBOX", None),
                    listed("Archive", Some(MailboxRole::Archive)),
                ],
            },
            &ctx(2),
        )
        .unwrap();
    let ids: Vec<MailboxId> = engine.state().mailboxes.keys().copied().collect();
    (engine, account, ids[0], ids[1])
}

fn message(engine: &Engine, mailbox: MailboxId, uid: u32) -> MessageId {
    engine
        .state()
        .messages
        .values()
        .find(|m| m.mailbox == mailbox && m.uid == uid)
        .unwrap()
        .id
}

fn of_kind<'a>(effects: &'a [Effect], name: &str) -> Vec<&'a Effect> {
    effects.iter().filter(|e| e.name() == name).collect()
}

#[test]
fn unread_inbox_bodies_are_prefetched_newest_first_up_to_twenty() {
    let (mut engine, account, inbox, archive) = setup();
    let mut envelopes: Vec<RemoteEnvelope> = (1..=25).map(|uid| remote(uid, false)).collect();
    envelopes.push(remote(26, true));
    let outcome = engine
        .ingest(fetched(account, inbox, envelopes), &ctx(3))
        .unwrap();
    let bodies = of_kind(&outcome.effects, "fetch_body");
    assert_eq!(bodies.len(), 20);
    let Effect::FetchBody {
        uid,
        uid_validity,
        path,
        ..
    } = bodies[0]
    else {
        panic!("not a body fetch");
    };
    assert_eq!((*uid, *uid_validity, path.as_str()), (25, 7, "INBOX"));
    assert!(
        bodies
            .iter()
            .all(|e| !matches!(e, Effect::FetchBody { uid: 26, .. }))
    );
    assert_eq!(engine.state().status.bodies_in_flight.len(), 20);

    // In-flight bodies are not requested again.
    let outcome = engine
        .ingest(fetched(account, inbox, Vec::new()), &ctx(4))
        .unwrap();
    assert!(of_kind(&outcome.effects, "fetch_body").is_empty());

    // Other mailboxes are not prefetched.
    let outcome = engine
        .ingest(fetched(account, archive, vec![remote(1, false)]), &ctx(5))
        .unwrap();
    assert!(of_kind(&outcome.effects, "fetch_body").is_empty());
}

#[test]
fn bodies_land_in_state_and_failures_are_recorded() {
    let (mut engine, account, inbox, _) = setup();
    let outcome = engine
        .ingest(
            fetched(account, inbox, vec![remote(1, false), remote(2, false)]),
            &ctx(3),
        )
        .unwrap();
    let [first, second] = of_kind(&outcome.effects, "fetch_body")[..] else {
        panic!("expected two body fetches");
    };
    let Effect::FetchBody { message: m2, .. } = first else {
        unreachable!()
    };
    let m1 = message(&engine, inbox, 1);
    assert_eq!(*m2, message(&engine, inbox, 2));

    let outcome = engine
        .ingest(
            Event::BodyFetched {
                message: *m2,
                blob: "ab".repeat(32),
                has_attachments: true,
                snippet: None,
            },
            &ctx(4),
        )
        .unwrap();
    let envelope = &engine.state().messages[m2];
    assert_eq!(
        envelope.body,
        BodyState::Fetched {
            blob: "ab".repeat(32)
        }
    );
    assert!(envelope.has_attachments);
    assert!(!engine.state().status.bodies_in_flight.contains(m2));
    assert!(matches!(
        &outcome.effects[..],
        [Effect::PersistState { delta: PersistDelta::Mail(delta) }] if delta.messages.len() == 1
    ));

    let failure = second.failure_event("UID 1 is gone".into());
    assert_eq!(
        failure,
        Event::BodyFetchFailed {
            message: m1,
            error: "UID 1 is gone".into()
        }
    );
    engine.ingest(failure, &ctx(5)).unwrap();
    assert_eq!(
        engine.state().messages[&m1].body,
        BodyState::Failed {
            error: "UID 1 is gone".into()
        }
    );
    assert!(engine.state().status.bodies_in_flight.is_empty());

    // A body for a message that is gone only clears the in-flight entry.
    let ghost = MessageId::from_uuid(uuid::Uuid::from_u128(77));
    let outcome = engine
        .ingest(
            Event::BodyFetched {
                message: ghost,
                blob: "cd".repeat(32),
                has_attachments: false,
                snippet: None,
            },
            &ctx(6),
        )
        .unwrap();
    assert!(outcome.effects.is_empty());
}

#[test]
fn incremental_fetches_resync_the_cached_window() {
    let (mut engine, account, _, archive) = setup();
    let first = engine
        .ingest(
            fetched(
                account,
                archive,
                vec![remote(3, true), remote(5, false), remote(8, true)],
            ),
            &ctx(3),
        )
        .unwrap();
    assert!(
        of_kind(&first.effects, "resync_flags").is_empty(),
        "nothing cached before"
    );

    let outcome = engine
        .ingest(fetched(account, archive, vec![remote(9, false)]), &ctx(4))
        .unwrap();
    let [resync] = of_kind(&outcome.effects, "resync_flags")[..] else {
        panic!("expected one resync");
    };
    let Effect::ResyncFlags {
        from_uid,
        to_uid,
        uid_validity,
        ..
    } = resync
    else {
        unreachable!()
    };
    assert_eq!((*from_uid, *to_uid, *uid_validity), (3, 8, 7));
    assert_eq!(engine.state().status.sync[&account].pending, 1);

    let m3 = message(&engine, archive, 3);
    let m5 = message(&engine, archive, 5);
    let m8 = message(&engine, archive, 8);
    let thread5 = engine.state().messages[&m5].thread;
    let outcome = engine
        .ingest(
            Event::FlagsChanged {
                account,
                mailbox: archive,
                uid_validity: 7,
                from_uid: 3,
                to_uid: 8,
                flags: vec![
                    UidFlags {
                        uid: 3,
                        flags: Flags {
                            seen: false,
                            flagged: true,
                            ..Flags::default()
                        },
                    },
                    UidFlags {
                        uid: 8,
                        flags: Flags {
                            seen: true,
                            ..Flags::default()
                        },
                    },
                ],
                existing: None,
                highest_modseq: None,
            },
            &ctx(5),
        )
        .unwrap();
    let state = engine.state();
    assert!(state.messages[&m3].flags.flagged && !state.messages[&m3].flags.seen);
    assert!(!state.messages.contains_key(&m5), "UID 5 was expunged");
    assert!(!state.threads.contains_key(&thread5));
    assert!(state.messages.contains_key(&m8));
    assert_eq!(state.status.sync[&account].pending, 0);
    assert_eq!(outcome.receipt.data["expunged"], 1);
    assert_eq!(outcome.receipt.data["flags_updated"], 1);
    let Effect::PersistState {
        delta: PersistDelta::Mail(delta),
    } = &outcome.effects[0]
    else {
        panic!("expected a mail delta");
    };
    assert_eq!(delta.removed_messages, vec![m5]);
    assert_eq!(delta.removed_threads, vec![thread5]);
}

#[test]
fn stale_or_foreign_flag_resyncs_are_ignored() {
    let (mut engine, account, _, archive) = setup();
    engine
        .ingest(fetched(account, archive, vec![remote(3, true)]), &ctx(3))
        .unwrap();
    let before = engine.state().messages.clone();
    for (who, validity) in [
        (account, 8),
        (AccountId::from_uuid(uuid::Uuid::from_u128(5)), 7),
    ] {
        let outcome = engine
            .ingest(
                Event::FlagsChanged {
                    account: who,
                    mailbox: archive,
                    uid_validity: validity,
                    from_uid: 1,
                    to_uid: 10,
                    flags: Vec::new(),
                    existing: None,
                    highest_modseq: None,
                },
                &ctx(4),
            )
            .unwrap();
        assert!(outcome.effects.is_empty());
    }
    assert_eq!(engine.state().messages, before);
    let resync = Effect::ResyncFlags {
        account,
        mailbox: archive,
        path: "Archive".into(),
        imap: engine.state().accounts[&account].imap_login(),
        uid_validity: 7,
        from_uid: 1,
        to_uid: 2,
        changed_since: None,
    };
    assert!(matches!(
        resync.failure_event("x".into()),
        Event::SyncFailed { mailbox: Some(m), .. } if m == archive
    ));
}
