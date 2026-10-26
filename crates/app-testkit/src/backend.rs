//! `MemoryBackend`: a fake IMAP/SMTP server, a fake local file system (attachments) and a
//! `MemoryStore`, with an effect log and fault injection for tests.

use std::collections::{BTreeMap, VecDeque};

use app_core::search::{Condition, Query};
use app_core::{
    AppState, Effect, Endpoint, Event, IdGen, ImapLogin, MailboxRole, OutgoingMessage,
    RemoteEnvelope, RemoteMailbox, RemoteStep, Secret, SecretRef, Security, SmtpLogin, UidFlags,
};
use app_io::Store;
use app_io::compose::Bcc;
use app_sync::{Backend, BoxFuture, SyncError};
use parking_lot::Mutex;

/// UIDVALIDITY, UIDNEXT, total, unread and envelopes of a fake fetch.
type Fetched = (u32, Option<u32>, u32, u32, Vec<RemoteEnvelope>);

use crate::fixture::{FakeMailbox, FakeMessage, Fixture};
use crate::store::MemoryStore;

/// A message accepted by the fake SMTP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMail {
    /// SMTP login it was submitted with.
    pub login: String,
    /// `MAIL FROM`.
    pub from: String,
    /// `RCPT TO`, in order.
    pub recipients: Vec<String>,
    /// The message as submitted.
    pub raw: String,
}

/// Offline backend. Mail effects run against the fixture's fake server (they arrive with the
/// milestones that add them); `PersistState` goes to the in-memory store and secrets go to an
/// in-memory keyring.
#[derive(Debug, Default)]
pub struct MemoryBackend {
    store: MemoryStore,
    server: Mutex<Fixture>,
    // `Secret` keeps the values out of `Debug` output.
    keyring: Mutex<BTreeMap<SecretRef, Secret>>,
    executed: Mutex<Vec<Effect>>,
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    sent: Mutex<Vec<SentMail>>,
    faults: Mutex<VecDeque<String>>,
    offline: Mutex<bool>,
    /// Whether `WatchMailbox` waits for changes (IDLE); otherwise the server "has no IDLE".
    push: bool,
    /// Wakes the watches on every server change.
    changes: tokio::sync::Notify,
    /// The full-text index of stored bodies, in memory, made on first use.
    fulltext: std::sync::OnceLock<Option<app_io::fulltext::FullText>>,
}

impl MemoryBackend {
    /// An empty server and an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// A server populated from `fixture`.
    pub fn from_fixture(fixture: Fixture) -> Self {
        Self {
            server: Mutex::new(fixture),
            ..Self::default()
        }
    }

    /// A fake server with IDLE: a `WatchMailbox` waits until the next [`MemoryBackend::edit_server`]
    /// (without it, watches report that the server has no IDLE).
    #[must_use]
    pub fn with_push(mut self) -> Self {
        self.push = true;
        self
    }

    /// The in-memory full-text index (made on first use; `None` if tantivy failed).
    fn fulltext(&self) -> Option<&app_io::fulltext::FullText> {
        self.fulltext
            .get_or_init(|| app_io::fulltext::FullText::in_memory().ok())
            .as_ref()
    }

    /// Uses `store` (e.g. with pre-persisted state) instead of an empty one.
    #[must_use]
    pub fn with_store(mut self, store: MemoryStore) -> Self {
        self.store = store;
        self
    }

    /// The in-memory store.
    pub fn store(&self) -> &MemoryStore {
        &self.store
    }

    /// Current content of the fake server.
    pub fn server(&self) -> Fixture {
        self.server.lock().clone()
    }

    /// Changes the fake server, e.g. to simulate new mail, flag changes or expunges.
    pub fn edit_server(&self, edit: impl FnOnce(&mut Fixture)) {
        edit(&mut self.server.lock());
        self.changes.notify_waiters();
    }

    /// Whether the in-memory keyring holds an entry for `secret`. The value itself is not exposed.
    pub fn has_secret(&self, secret: &SecretRef) -> bool {
        self.keyring.lock().contains_key(secret)
    }

    /// Puts a file into the fake local file system (for attachments).
    pub fn add_file(&self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.files.lock().insert(path.into(), bytes.into());
    }

    /// A file of the fake local file system.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        self.files.lock().get(path).cloned()
    }

    /// Every message the fake SMTP server accepted, in order.
    pub fn sent(&self) -> Vec<SentMail> {
        self.sent.lock().clone()
    }

    /// Every effect executed so far, in order (including failed ones).
    pub fn executed(&self) -> Vec<Effect> {
        self.executed.lock().clone()
    }

    /// Makes the next executed effect fail with `reason`. Calls queue up.
    pub fn fail_next(&self, reason: impl Into<String>) {
        self.faults.lock().push_back(reason.into());
    }

    /// Takes the fake servers off the network (or back): while offline every effect that talks
    /// to a server fails with a connection error; the store and the keyring keep working.
    pub fn set_offline(&self, offline: bool) {
        *self.offline.lock() = offline;
    }

    fn run(&self, effect: Effect) -> Result<Vec<Event>, SyncError> {
        self.executed.lock().push(effect.clone());
        if let Some(reason) = self.faults.lock().pop_front() {
            return Err(SyncError::Effect {
                effect: effect.name(),
                reason,
            });
        }
        let networked = !matches!(
            effect,
            Effect::PersistState { .. }
                | Effect::StoreSecret { .. }
                | Effect::DeleteSecret { .. }
                | Effect::ScheduleTick { .. }
                | Effect::InspectAttachment { .. }
                | Effect::SaveAttachments { .. }
                | Effect::CompactStore { .. }
                | Effect::SearchIndex { .. }
        );
        if networked && *self.offline.lock() {
            return Err(SyncError::Effect {
                effect: effect.name(),
                reason: "connect failed: network is unreachable".to_owned(),
            });
        }
        match effect {
            Effect::PersistState { delta } => {
                if let (app_core::PersistDelta::Mail(mail), Some(Some(index))) =
                    (&delta, self.fulltext.get())
                {
                    index.remove(&mail.removed_messages);
                }
                self.store.apply(&delta)?;
                Ok(vec![Event::Persisted {}])
            }
            Effect::StoreSecret {
                account,
                secret,
                value,
            } => {
                self.keyring.lock().insert(secret, value);
                Ok(vec![Event::SecretStored { account }])
            }
            Effect::DeleteSecret { account, secret } => {
                self.keyring.lock().remove(&secret);
                Ok(vec![Event::SecretDeleted { account }])
            }
            Effect::VerifyAccount {
                account,
                login,
                secret,
                ..
            } => Ok(vec![match self.verify(&login, &secret) {
                Ok(()) => Event::AccountVerified { account },
                Err(error) => Event::AccountVerifyFailed { account, error },
            }]),
            Effect::ListMailboxes { account, imap } => Ok(vec![match self.list(&imap) {
                Ok(mailboxes) => Event::MailboxesListed { account, mailboxes },
                Err(error) => Event::SyncFailed {
                    account,
                    mailbox: None,
                    error,
                },
            }]),
            Effect::FetchEnvelopes {
                account,
                mailbox,
                path,
                imap,
                uid_validity,
                since_uid,
                limit,
            } => Ok(vec![
                match self.fetch(&imap, &path, uid_validity, since_uid, limit) {
                    Ok((uid_validity, uid_next, total, unread, envelopes)) => {
                        Event::EnvelopesFetched {
                            account,
                            mailbox,
                            uid_validity,
                            uid_next,
                            total,
                            unread,
                            envelopes,
                        }
                    }
                    Err(error) => Event::SyncFailed {
                        account,
                        mailbox: Some(mailbox),
                        error,
                    },
                },
            ]),
            Effect::ResyncFlags {
                account,
                mailbox,
                path,
                imap,
                uid_validity: _,
                from_uid,
                to_uid,
                // The fake server has no CONDSTORE: it always lists every flag.
                changed_since: _,
            } => Ok(vec![match self.flags(&imap, &path, from_uid, to_uid) {
                Ok((uid_validity, flags)) => Event::FlagsChanged {
                    account,
                    mailbox,
                    uid_validity,
                    from_uid,
                    to_uid,
                    flags,
                    existing: None,
                    highest_modseq: None,
                },
                Err(error) => Event::SyncFailed {
                    account,
                    mailbox: Some(mailbox),
                    error,
                },
            }]),
            Effect::SearchMailbox {
                account,
                mailbox,
                path,
                imap,
                search,
                started_ms,
                mailboxes,
                query,
                gmail_raw: _,
                limit,
            } => Ok(vec![match self.search(&imap, &path, &query, limit) {
                Ok((uid_validity, envelopes)) => Event::SearchResults {
                    account,
                    mailbox,
                    uid_validity,
                    search,
                    started_ms,
                    mailboxes,
                    envelopes,
                },
                Err(error) => Event::SearchFailed {
                    account,
                    mailbox,
                    search,
                    started_ms,
                    mailboxes,
                    error,
                },
            }]),
            Effect::FetchBody {
                message,
                path,
                imap,
                uid_validity,
                uid,
                ..
            } => Ok(vec![match self.body(&imap, &path, uid_validity, uid) {
                Ok(raw) => {
                    let blob = self.store.put_blob(&raw)?;
                    if let Some(index) = self.fulltext() {
                        index.add(message, &raw)?;
                    }
                    Event::BodyFetched {
                        message,
                        blob,
                        has_attachments: app_io::mime::has_attachments(&raw),
                        snippet: app_io::mime::snippet(&raw),
                    }
                }
                Err(error) => Event::BodyFetchFailed { message, error },
            }]),
            Effect::RunRemoteOp {
                op, imap, steps, ..
            } => Ok(vec![match self.remote(&imap, &steps) {
                Ok(()) => Event::RemoteOpSucceeded { op },
                // The fake server answered: a refusal is a rejection, not a lost connection.
                Err(error) => Event::RemoteOpFailed {
                    op,
                    error,
                    permanent: true,
                },
            }]),
            Effect::SaveDraft {
                draft,
                imap,
                path,
                revision,
                message,
                ..
            } => Ok(vec![match self
                .render(&message, Bcc::Keep)
                .and_then(|raw| self.save_draft(&imap, &path, &message.message_id, raw))
            {
                Ok(()) => Event::DraftSaved { draft, revision },
                Err(error) => Event::DraftSaveFailed { draft, error },
            }]),
            Effect::DeleteDraft {
                draft,
                account,
                imap,
                path,
                message_id,
            } => Ok(vec![match self.delete_draft(&imap, &path, &message_id) {
                Ok(()) => Event::DraftDeleted { draft },
                Err(error) => Event::DraftDeleteFailed {
                    account,
                    draft,
                    error,
                },
            }]),
            Effect::SearchIndex {
                search,
                words,
                limit,
            } => {
                let limit = usize::try_from(limit).unwrap_or(usize::MAX);
                let found = self
                    .fulltext()
                    .map_or(Ok(Vec::new()), |index| index.search(&words, limit));
                Ok(vec![match found {
                    Ok(hits) => Event::IndexSearched { search, hits },
                    Err(err) => Event::IndexSearchFailed {
                        search,
                        error: err.to_string(),
                    },
                }])
            }
            Effect::CompactStore { keep } => {
                let keep = keep.into_iter().collect();
                Ok(vec![
                    match self.store.compact(&keep, std::time::SystemTime::now()) {
                        Ok(done) => Event::StoreCompacted {
                            removed_blobs: done.removed_blobs,
                            freed_bytes: done.freed_bytes,
                        },
                        Err(err) => Event::CompactFailed {
                            error: err.to_string(),
                        },
                    },
                ])
            }
            Effect::FetchOlder {
                account,
                mailbox,
                path,
                imap,
                uid_validity,
                before_uid,
                limit,
            } => Ok(vec![
                match self.older(&imap, &path, uid_validity, before_uid, limit) {
                    Ok((uid_validity, envelopes, complete)) => Event::OlderFetched {
                        account,
                        mailbox,
                        uid_validity,
                        envelopes,
                        complete,
                    },
                    Err(error) => Event::BackfillFailed {
                        account,
                        mailbox,
                        error,
                    },
                },
            ]),
            // `execute` waits for the change; here only the login and IDLE are checked.
            Effect::WatchMailbox {
                account,
                mailbox,
                imap,
                ..
            } => Ok(match self.verify(&imap.login, &imap.secret) {
                Err(error) => vec![Event::WatchFailed {
                    account,
                    mailbox,
                    error,
                    unsupported: false,
                }],
                Ok(()) if !self.push => vec![Event::WatchFailed {
                    account,
                    mailbox,
                    error: "the server does not support IDLE".to_owned(),
                    unsupported: true,
                }],
                Ok(()) => Vec::new(),
            }),
            Effect::SaveAttachments {
                message,
                blob,
                parts,
                dir,
            } => Ok(vec![match self.save_attachments(&blob, &parts, &dir) {
                Ok(files) => Event::AttachmentsSaved { message, files },
                Err(error) => Event::AttachmentsSaveFailed {
                    message,
                    files: Vec::new(),
                    error,
                },
            }]),
            // The fake world's web: the `example.*` domains publish autoconfig documents.
            Effect::DiscoverServers { email, domain } => {
                Ok(vec![
                    if ["example.com", "example.org", "example.net"].contains(&domain.as_str()) {
                        Event::ServersDiscovered {
                            email,
                            imap: Endpoint {
                                host: format!("imap.{domain}"),
                                port: 993,
                                security: Security::Tls,
                            },
                            smtp: Endpoint {
                                host: format!("smtp.{domain}"),
                                port: 587,
                                security: Security::StartTls,
                            },
                            source: format!("https://autoconfig.{domain}/mail/config-v1.1.xml"),
                        }
                    } else {
                        Event::DiscoveryFailed {
                            email,
                            error: format!("autoconfig.{domain}: no such host"),
                        }
                    },
                ])
            }
            Effect::InspectAttachment { draft, path } => Ok(vec![match self.file(&path) {
                Some(bytes) => Event::AttachmentInspected {
                    draft,
                    content_type: app_io::mime::content_type_for(app_core::draft::file_name(&path))
                        .to_owned(),
                    path,
                    size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                },
                None => Event::AttachmentFailed {
                    error: format!("cannot read {path}: No such file or directory"),
                    draft,
                    path,
                },
            }]),
            Effect::SmtpSend {
                draft,
                smtp,
                message,
                sent_copy,
                ..
            } => Ok(vec![match self.submit(&smtp, &message) {
                Ok(()) => Event::MessageSent {
                    draft,
                    sent_copy_error: sent_copy.and_then(|copy| {
                        self.render(&message, Bcc::Keep)
                            .and_then(|raw| self.store_copy(&copy.imap, &copy.path, raw))
                            .err()
                    }),
                },
                Err(error) => Event::SendFailed { draft, error },
            }]),
            Effect::ScheduleTick { .. } => Err(SyncError::Unsupported {
                backend: self.name(),
                effect: effect.name(),
            }),
        }
    }

    /// Fake `SELECT` + `UID STORE` (also `1:*`) / `UID MOVE` / `UID COPY`, step by step. A step whose
    /// UIDVALIDITY no longer matches fails; steps before it stay applied (as on a real server).
    fn remote(&self, imap: &ImapLogin, steps: &[RemoteStep]) -> Result<(), String> {
        self.verify(&imap.login, &imap.secret)?;
        let mut server = self.server.lock();
        let account = server
            .accounts
            .iter_mut()
            .find(|account| account.email.eq_ignore_ascii_case(&imap.login))
            .ok_or_else(|| "IMAP LOGIN failed: unknown user".to_owned())?;
        for step in steps {
            match step {
                RemoteStep::Store {
                    path,
                    uid_validity,
                    uids,
                    flag,
                    value,
                } => {
                    let mailbox = selected(&mut account.mailboxes, path, *uid_validity)?;
                    let name = flag.imap();
                    for message in mailbox
                        .messages
                        .iter_mut()
                        .filter(|m| uids.contains(&m.uid))
                    {
                        message.flags.retain(|f| !f.eq_ignore_ascii_case(name));
                        if *value {
                            message.flags.push(name.to_owned());
                        }
                    }
                }
                RemoteStep::StoreAll {
                    path,
                    uid_validity,
                    flag,
                    value,
                } => {
                    let mailbox = selected(&mut account.mailboxes, path, *uid_validity)?;
                    let name = flag.imap();
                    for message in &mut mailbox.messages {
                        message.flags.retain(|f| !f.eq_ignore_ascii_case(name));
                        if *value {
                            message.flags.push(name.to_owned());
                        }
                    }
                }
                RemoteStep::Move {
                    path,
                    uid_validity,
                    uids,
                    to,
                    create,
                }
                | RemoteStep::Copy {
                    path,
                    uid_validity,
                    uids,
                    to,
                    create,
                } => {
                    let source = selected(&mut account.mailboxes, path, *uid_validity)?;
                    let picked: Vec<FakeMessage> = source
                        .messages
                        .iter()
                        .filter(|m| uids.contains(&m.uid))
                        .cloned()
                        .collect();
                    if matches!(step, RemoteStep::Move { .. }) {
                        // UIDs are never reused, even when the newest message leaves.
                        source.uid_next = Some(source.next_uid());
                        source.messages.retain(|m| !uids.contains(&m.uid));
                    }
                    if *create && !account.mailboxes.iter().any(|m| m.path == *to) {
                        account.mailboxes.push(FakeMailbox::new(to.clone()));
                    }
                    let target = account
                        .mailboxes
                        .iter_mut()
                        .find(|m| m.path == *to)
                        .ok_or_else(|| format!("IMAP MOVE to {to} failed: no such mailbox"))?;
                    for message in picked {
                        target.append(message.flags, message.raw);
                    }
                }
            }
        }
        Ok(())
    }

    /// Writes attachments of a stored body into the fake file system, like
    /// `app_sync::saves` does on disk (never over an existing file).
    fn save_attachments(
        &self,
        blob: &str,
        parts: &[usize],
        dir: &str,
    ) -> Result<Vec<String>, String> {
        let raw = self
            .store
            .get_blob(blob)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "the message is not in the local store any more".to_owned())?;
        let attachments = app_io::mime::attachment_files(&raw, parts)?;
        if attachments.is_empty() {
            return Err("the message has no attachments".to_owned());
        }
        let mut files = self.files.lock();
        let mut written = Vec::new();
        for (name, bytes) in attachments {
            let dir = dir.trim_end_matches('/');
            let mut path = format!("{dir}/{name}");
            let mut n = 1;
            while files.contains_key(&path) {
                path = format!("{dir}/{}", app_io::mime::numbered_file_name(&name, n));
                n += 1;
            }
            files.insert(path.clone(), bytes);
            written.push(path);
        }
        Ok(written)
    }

    /// Renders an outgoing message with the quoted blob from the store and the attached files
    /// from the fake file system.
    fn render(&self, message: &OutgoingMessage, bcc: Bcc) -> Result<Vec<u8>, String> {
        let original = match message.quote.as_ref().and_then(|q| q.blob.as_deref()) {
            Some(blob) => self.store.get_blob(blob).map_err(|err| err.to_string())?,
            None => None,
        };
        let files = message
            .attachments
            .iter()
            .map(|a| {
                self.file(&a.path)
                    .ok_or_else(|| format!("cannot read {}: No such file or directory", a.path))
            })
            .collect();
        app_sync::outgoing::render(message, original.as_deref(), files, bcc)
    }

    /// Fake SMTP submission: AUTH like `verify`, then the message (rendered without Bcc) is
    /// recorded in [`MemoryBackend::sent`] and, if the account files sent mail itself
    /// (`smtp_saves_to`), appended there.
    fn submit(&self, smtp: &SmtpLogin, message: &OutgoingMessage) -> Result<(), String> {
        let at = format!("SMTP {}:{}", smtp.server.host, smtp.server.port);
        self.verify(&smtp.login, &smtp.secret)
            .map_err(|err| format!("{at}: {}", err.replace("IMAP LOGIN", "AUTH")))?;
        let raw = self.render(message, Bcc::Hide)?;
        let recipients = app_sync::smtp::recipients(message);
        if recipients.is_empty() {
            return Err(format!("{at}: 554 no valid recipients"));
        }
        let raw = String::from_utf8_lossy(&raw).into_owned();
        let mut server = self.server.lock();
        if let Some(account) = server
            .accounts
            .iter_mut()
            .find(|account| account.email.eq_ignore_ascii_case(&smtp.login))
            && let Some(path) = account.smtp_saves_to.clone()
            && let Some(mailbox) = account.mailboxes.iter_mut().find(|m| m.path == path)
        {
            mailbox.append(vec!["\\Seen".to_owned()], raw.clone());
        }
        self.sent.lock().push(SentMail {
            login: smtp.login.clone(),
            from: message.from.email.clone(),
            recipients,
            raw,
        });
        Ok(())
    }

    /// Fake `APPEND` of a sent message with `\Seen`.
    fn store_copy(&self, imap: &ImapLogin, path: &str, raw: Vec<u8>) -> Result<(), String> {
        let at = path.to_owned();
        self.with_mailbox(imap, path, |mailbox| {
            mailbox.append(
                vec!["\\Seen".to_owned()],
                String::from_utf8_lossy(&raw).into_owned(),
            );
            Ok(())
        })
        .map_err(|err| format!("IMAP APPEND to {at} failed: {err}"))
    }

    /// Fake `APPEND` of a draft (`\Seen \Draft`) after which older copies with the same
    /// `Message-ID` are expunged.
    fn save_draft(
        &self,
        imap: &ImapLogin,
        path: &str,
        message_id: &str,
        raw: Vec<u8>,
    ) -> Result<(), String> {
        let mailbox_path = path.to_owned();
        self.with_mailbox(imap, path, |mailbox| {
            mailbox
                .messages
                .retain(|m| !has_message_id(&m.raw, message_id));
            mailbox.append(
                vec!["\\Seen".to_owned(), "\\Draft".to_owned()],
                String::from_utf8_lossy(&raw).into_owned(),
            );
            Ok(())
        })
        .map_err(|err| format!("IMAP APPEND to {mailbox_path} failed: {err}"))
    }

    /// Fake delete of every copy of a draft.
    fn delete_draft(&self, imap: &ImapLogin, path: &str, message_id: &str) -> Result<(), String> {
        self.with_mailbox(imap, path, |mailbox| {
            // UIDs are never reused, even when the newest message leaves.
            mailbox.uid_next = Some(mailbox.next_uid());
            mailbox
                .messages
                .retain(|m| !has_message_id(&m.raw, message_id));
            Ok(())
        })
    }

    /// Runs `change` on the mailbox at `path` of the account `imap` logs in to.
    fn with_mailbox(
        &self,
        imap: &ImapLogin,
        path: &str,
        change: impl FnOnce(&mut FakeMailbox) -> Result<(), String>,
    ) -> Result<(), String> {
        self.verify(&imap.login, &imap.secret)?;
        let mut server = self.server.lock();
        let mailbox = server
            .accounts
            .iter_mut()
            .find(|account| account.email.eq_ignore_ascii_case(&imap.login))
            .ok_or_else(|| "IMAP LOGIN failed: unknown user".to_owned())?
            .mailboxes
            .iter_mut()
            .find(|m| m.path == path)
            .ok_or_else(|| format!("IMAP SELECT {path} failed: no such mailbox"))?;
        change(mailbox)
    }

    /// Fake `LIST`: the account's mailboxes with `/` as delimiter.
    fn list(&self, imap: &ImapLogin) -> Result<Vec<RemoteMailbox>, String> {
        self.verify(&imap.login, &imap.secret)?;
        let server = self.server.lock();
        let account = server
            .account(&imap.login)
            .ok_or_else(|| "IMAP LOGIN failed: unknown user".to_owned())?;
        Ok(account
            .mailboxes
            .iter()
            .map(|mailbox| RemoteMailbox {
                path: mailbox.path.clone(),
                delimiter: Some("/".to_owned()),
                special_use: mailbox
                    .special_use
                    .as_deref()
                    .and_then(MailboxRole::from_special_use),
                selectable: mailbox.selectable,
            })
            .collect())
    }

    /// Fake history backfill: up to `limit` envelopes just below `before_uid`, like
    /// `app_sync::imap::fetch_older`.
    fn older(
        &self,
        imap: &ImapLogin,
        path: &str,
        uid_validity: u32,
        before_uid: u32,
        limit: u32,
    ) -> Result<(u32, Vec<RemoteEnvelope>, bool), String> {
        self.verify(&imap.login, &imap.secret)?;
        let server = self.server.lock();
        let mailbox = server
            .account(&imap.login)
            .and_then(|account| account.mailboxes.iter().find(|m| m.path == path))
            .ok_or_else(|| format!("IMAP EXAMINE {path} failed: no such mailbox"))?;
        if mailbox.uid_validity != uid_validity {
            return Ok((mailbox.uid_validity, Vec::new(), false));
        }
        let older: Vec<&crate::fixture::FakeMessage> = mailbox
            .messages
            .iter()
            .filter(|message| message.uid < before_uid)
            .collect();
        let keep = usize::try_from(limit).unwrap_or(usize::MAX);
        let start = older.len().saturating_sub(keep);
        let envelopes = older[start..]
            .iter()
            .map(|message| {
                app_io::mime::remote_envelope(
                    message.uid,
                    message.flags.iter().map(String::as_str),
                    u32::try_from(message.raw.len()).unwrap_or(u32::MAX),
                    message.raw.as_bytes(),
                )
            })
            .collect();
        Ok((uid_validity, envelopes, start == 0))
    }

    /// Fake `EXAMINE` + `FETCH`, with the same window rules as the live backend: UIDVALIDITY,
    /// UIDNEXT, total, unread and the envelopes.
    fn fetch(
        &self,
        imap: &ImapLogin,
        path: &str,
        uid_validity: Option<u32>,
        since_uid: Option<u32>,
        limit: u32,
    ) -> Result<Fetched, String> {
        self.verify(&imap.login, &imap.secret)?;
        let server = self.server.lock();
        let mailbox = server
            .account(&imap.login)
            .and_then(|account| account.mailboxes.iter().find(|m| m.path == path))
            .ok_or_else(|| format!("IMAP EXAMINE {path} failed: no such mailbox"))?;
        let incremental = since_uid.filter(|_| uid_validity == Some(mailbox.uid_validity));
        let mut envelopes: Vec<RemoteEnvelope> = mailbox
            .messages
            .iter()
            .filter(|message| incremental.is_none_or(|since| message.uid >= since))
            .map(|message| {
                app_io::mime::remote_envelope(
                    message.uid,
                    message.flags.iter().map(String::as_str),
                    u32::try_from(message.raw.len()).unwrap_or(u32::MAX),
                    message.raw.as_bytes(),
                )
            })
            .collect();
        let keep = usize::try_from(limit).unwrap_or(usize::MAX);
        if envelopes.len() > keep {
            envelopes.drain(..envelopes.len() - keep);
        }
        let unread = mailbox
            .messages
            .iter()
            .filter(|message| {
                !message
                    .flags
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case("\\Seen"))
            })
            .count();
        Ok((
            mailbox.uid_validity,
            Some(mailbox.next_uid()),
            u32::try_from(mailbox.messages.len()).unwrap_or(u32::MAX),
            u32::try_from(unread).unwrap_or(u32::MAX),
            envelopes,
        ))
    }

    /// Fake `UID SEARCH` (IMAP criteria and Gmail's `X-GM-RAW` alike): text terms match the
    /// raw message (headers and body, case-insensitively), every other term is checked like
    /// the local search does. Returns UIDVALIDITY and the newest `limit` hits.
    fn search(
        &self,
        imap: &ImapLogin,
        path: &str,
        query: &Query,
        limit: u32,
    ) -> Result<(u32, Vec<RemoteEnvelope>), String> {
        self.verify(&imap.login, &imap.secret)?;
        let server = self.server.lock();
        let mailbox = server
            .account(&imap.login)
            .and_then(|account| account.mailboxes.iter().find(|m| m.path == path))
            .ok_or_else(|| format!("IMAP EXAMINE {path} failed: no such mailbox"))?;
        let mut ids = IdGen::new(0);
        let mut hits: Vec<RemoteEnvelope> = mailbox
            .messages
            .iter()
            .filter_map(|message| {
                let remote = app_io::mime::remote_envelope(
                    message.uid,
                    message.flags.iter().map(String::as_str),
                    u32::try_from(message.raw.len()).unwrap_or(u32::MAX),
                    message.raw.as_bytes(),
                );
                let envelope = remote.clone().into_envelope(
                    ids.next_id(),
                    ids.next_id(),
                    ids.next_id(),
                    ids.next_id(),
                );
                let raw = message.raw.to_lowercase();
                let found = query.terms.iter().all(|term| match &term.condition {
                    Condition::Text(text) => raw.contains(&text.to_lowercase()) != term.negated,
                    Condition::In(_) => true,
                    _ => Query {
                        terms: vec![term.clone()],
                    }
                    .matches(&envelope, None),
                });
                found.then_some(remote)
            })
            .collect();
        let keep = usize::try_from(limit).unwrap_or(usize::MAX);
        if hits.len() > keep {
            hits.drain(..hits.len() - keep);
        }
        Ok((mailbox.uid_validity, hits))
    }

    /// Fake `UID FETCH from:to (FLAGS)`.
    fn flags(
        &self,
        imap: &ImapLogin,
        path: &str,
        from: u32,
        to: u32,
    ) -> Result<(u32, Vec<UidFlags>), String> {
        self.verify(&imap.login, &imap.secret)?;
        let server = self.server.lock();
        let mailbox = server
            .account(&imap.login)
            .and_then(|account| account.mailboxes.iter().find(|m| m.path == path))
            .ok_or_else(|| format!("IMAP EXAMINE {path} failed: no such mailbox"))?;
        let flags = mailbox
            .messages
            .iter()
            .filter(|m| m.uid >= from && m.uid <= to)
            .map(|m| UidFlags {
                uid: m.uid,
                flags: app_io::mime::parse_flags(m.flags.iter().map(String::as_str)),
            })
            .collect();
        Ok((mailbox.uid_validity, flags))
    }

    /// Fake `UID FETCH uid (BODY.PEEK[])`.
    fn body(
        &self,
        imap: &ImapLogin,
        path: &str,
        uid_validity: u32,
        uid: u32,
    ) -> Result<Vec<u8>, String> {
        self.verify(&imap.login, &imap.secret)?;
        let server = self.server.lock();
        let mailbox = server
            .account(&imap.login)
            .and_then(|account| account.mailboxes.iter().find(|m| m.path == path))
            .ok_or_else(|| format!("IMAP EXAMINE {path} failed: no such mailbox"))?;
        if mailbox.uid_validity != uid_validity {
            return Err(format!(
                "{path}: UIDVALIDITY changed; the message must be re-synced"
            ));
        }
        mailbox
            .messages
            .iter()
            .find(|m| m.uid == uid)
            .map(|m| m.raw.clone().into_bytes())
            .ok_or_else(|| format!("{path}: message UID {uid} is no longer on the server"))
    }

    /// Fake IMAP LOGIN + SMTP AUTH: the login must exist on the fake server and the stored
    /// password must match (any password if the fixture sets none).
    fn verify(&self, login: &str, secret: &SecretRef) -> Result<(), String> {
        let Some(password) = self.keyring.lock().get(secret).cloned() else {
            return Err(format!(
                "no password stored in the keyring ({}/{})",
                secret.service, secret.user
            ));
        };
        let server = self.server.lock();
        let account = server
            .account(login)
            .ok_or_else(|| "IMAP LOGIN failed: unknown user".to_owned())?;
        match &account.password {
            Some(expected) if expected != password.expose() => {
                Err("IMAP LOGIN failed: invalid credentials".to_owned())
            }
            _ => Ok(()),
        }
    }
}

impl Backend for MemoryBackend {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        Box::pin(async move { Ok(self.store.load_snapshot()?) })
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            let Effect::WatchMailbox {
                account, mailbox, ..
            } = &effect
            else {
                return self.run(effect);
            };
            let (account, mailbox) = (*account, *mailbox);
            // Created before the checks, so a change made meanwhile is not missed.
            let changed = self.changes.notified();
            let events = self.run(effect)?;
            if !events.is_empty() {
                // The login failed, or the server has no IDLE.
                return Ok(events);
            }
            changed.await;
            Ok(vec![Event::MailboxWatched {
                account,
                mailbox,
                changed: true,
            }])
        })
    }

    fn get_blob<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Option<Vec<u8>>, SyncError>> {
        Box::pin(async move { Ok(self.store.get_blob(id)?) })
    }
}

/// Whether the raw message has `Message-ID: <message_id>`.
fn has_message_id(raw: &str, message_id: &str) -> bool {
    app_io::mime::summarize_headers(raw.as_bytes())
        .message_id
        .is_some_and(|id| id == message_id)
}

/// The mailbox at `path`, as `SELECT` would open it: it must exist and have `uid_validity`.
fn selected<'a>(
    mailboxes: &'a mut [FakeMailbox],
    path: &str,
    uid_validity: u32,
) -> Result<&'a mut FakeMailbox, String> {
    let mailbox = mailboxes
        .iter_mut()
        .find(|m| m.path == path)
        .ok_or_else(|| format!("IMAP SELECT {path} failed: no such mailbox"))?;
    if mailbox.uid_validity != uid_validity {
        return Err(format!("{path}: UIDVALIDITY changed"));
    }
    Ok(mailbox)
}
