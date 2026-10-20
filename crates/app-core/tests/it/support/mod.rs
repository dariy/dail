//! Shared setup for the read & triage tests: an account with listed mailboxes and fetched mail.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AccountId, Addr, Command, Ctx, Effect, Engine, Event, Flags, MailboxId, MailboxRole, MessageId,
    Origin, Outcome, ProviderPreset, RemoteEnvelope, RemoteMailbox, ThreadId,
};

pub const T0: i64 = 1_700_000_000_000;

/// A context with seed `n << 64`, `n` seconds after [`T0`].
pub fn ctx(n: u128) -> Ctx {
    Ctx {
        now_ms: T0 + i64::try_from(n).unwrap() * 1000,
        id_seed: n << 64,
        origin: Origin::Cli,
    }
}

/// A mail account with its mailboxes listed and envelopes fetched.
pub struct Mail {
    pub engine: Engine,
    pub account: AccountId,
    /// Mailbox IDs by path.
    pub mailboxes: Vec<(String, MailboxId)>,
    /// Next seed for [`Mail::run`] / [`Mail::feed`].
    pub seed: u128,
}

impl Mail {
    /// An account of `preset` whose server lists `paths` (with SPECIAL-USE roles).
    pub fn new(preset: ProviderPreset, paths: &[(&str, Option<MailboxRole>)]) -> Self {
        let mut engine = Engine::default();
        let cmd = Command::AddAccount {
            preset,
            email: "me@example.com".into(),
            display_name: None,
            password: None,
            imap: Some(app_core::Endpoint {
                host: "imap.example.com".into(),
                port: 993,
                security: app_core::Security::Tls,
            }),
            smtp: Some(app_core::Endpoint {
                host: "smtp.example.com".into(),
                port: 465,
                security: app_core::Security::Tls,
            }),
        };
        engine.dispatch(cmd, &ctx(1)).unwrap();
        let account = AccountId::from_uuid(uuid::Uuid::from_u128(1 << 64));
        let listed = paths
            .iter()
            .map(|(path, special_use)| RemoteMailbox {
                path: (*path).into(),
                delimiter: Some("/".into()),
                special_use: *special_use,
                selectable: true,
            })
            .collect();
        engine
            .ingest(
                Event::MailboxesListed {
                    account,
                    mailboxes: listed,
                },
                &ctx(2),
            )
            .unwrap();
        let mailboxes = engine
            .state()
            .mailboxes
            .values()
            .map(|m| (m.path.clone(), m.id))
            .collect();
        Self {
            engine,
            account,
            mailboxes,
            seed: 3,
        }
    }

    /// Hostinger with INBOX, Archive, Trash and Sent.
    pub fn hostinger() -> Self {
        Self::new(
            ProviderPreset::Hostinger,
            &[
                ("INBOX", None),
                ("Archive", Some(MailboxRole::Archive)),
                ("Trash", Some(MailboxRole::Trash)),
                ("Sent", Some(MailboxRole::Sent)),
            ],
        )
    }

    /// The mailbox with `path`.
    pub fn mailbox(&self, path: &str) -> MailboxId {
        self.mailboxes
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, id)| *id)
            .unwrap_or_else(|| panic!("no mailbox {path}"))
    }

    fn next_ctx(&mut self) -> Ctx {
        let ctx = ctx(self.seed);
        self.seed += 1;
        ctx
    }

    /// Dispatches a command with the next seed.
    pub fn run(&mut self, cmd: Command) -> Result<Outcome, app_core::CoreError> {
        let ctx = self.next_ctx();
        self.engine.dispatch(cmd, &ctx)
    }

    /// Ingests an event with the next seed.
    pub fn feed(&mut self, event: Event) -> Outcome {
        let ctx = self.next_ctx();
        self.engine.ingest(event, &ctx).unwrap()
    }

    /// Fetches `envelopes` into the mailbox at `path` (UIDVALIDITY 7).
    pub fn fetch(&mut self, path: &str, envelopes: Vec<RemoteEnvelope>) -> Outcome {
        let mailbox = self.mailbox(path);
        let uid_next = envelopes.iter().map(|e| e.uid + 1).max();
        self.feed(Event::EnvelopesFetched {
            account: self.account,
            mailbox,
            uid_validity: 7,
            uid_next,
            total: u32::try_from(envelopes.len()).unwrap(),
            unread: 0,
            envelopes,
        })
    }

    /// The message with `uid` in the mailbox at `path`.
    pub fn message(&self, path: &str, uid: u32) -> MessageId {
        let mailbox = self.mailbox(path);
        self.engine
            .state()
            .messages
            .values()
            .find(|m| m.mailbox == mailbox && m.uid == uid)
            .unwrap_or_else(|| panic!("no UID {uid} in {path}"))
            .id
    }

    /// The thread of a message.
    pub fn thread_of(&self, message: MessageId) -> ThreadId {
        self.engine.state().messages[&message].thread
    }
}

/// An envelope with `Message-ID: <id>`, replying to `parent` if given.
pub fn mail(uid: u32, id: &str, parent: Option<&str>, subject: &str, seen: bool) -> RemoteEnvelope {
    RemoteEnvelope {
        uid,
        message_id_hdr: Some(id.into()),
        in_reply_to: parent.map(Into::into),
        references: parent.map(|p| vec![p.to_owned()]).unwrap_or_default(),
        from: vec![Addr {
            name: Some("Bob".into()),
            email: "bob@example.org".into(),
        }],
        to: Vec::new(),
        cc: Vec::new(),
        subject: subject.into(),
        date_ms: T0 - 86_400_000 + i64::from(uid) * 1000,
        flags: Flags {
            seen,
            ..Flags::default()
        },
        size: 100,
        has_attachments: false,
    }
}

/// Names of `effects`.
pub fn names(effects: &[Effect]) -> Vec<&'static str> {
    effects.iter().map(Effect::name).collect()
}

/// Effects named `name`.
pub fn of_kind<'a>(effects: &'a [Effect], name: &str) -> Vec<&'a Effect> {
    effects.iter().filter(|e| e.name() == name).collect()
}
