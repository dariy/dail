//! `LiveBackend`: the real world — local store, OS keyring and IMAP/SMTP servers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use app_core::{
    AccountId, AppState, Effect, Event, ImapLogin, MessageId, OutgoingMessage, Secret, SecretRef,
    SentCopy, SmtpLogin, UidFlags,
};
use app_io::Store;
use app_io::compose::Bcc;
use app_io::fulltext::FullText;
use parking_lot::Mutex;

use crate::backend::{Backend, BoxFuture};
use crate::error::SyncError;
use crate::imap::{self, ImapConnector, ImapSession};
use crate::outgoing;
use crate::secrets::SecretStore;
use crate::smtp::{self, NetSmtp, SmtpSender, Submission};
use crate::tls;
use crate::verify::AccountVerifier;

/// Limit for one IMAP operation on an open session.
const OP_TIMEOUT: Duration = Duration::from_secs(120);
/// Bodies written more recently than this survive a compaction even if no message refers to
/// them yet.
const COMPACT_GRACE: Duration = Duration::from_secs(60 * 60);

/// An open IMAP session, kept for the next effect of the same account.
struct CachedSession {
    login: ImapLogin,
    session: ImapSession,
}

/// Production backend: local store, OS keyring and IMAP/SMTP servers. Keeps one IMAP session
/// per account open between effects.
pub struct LiveBackend {
    store: Arc<dyn Store>,
    secrets: Arc<dyn SecretStore>,
    verifier: Arc<dyn AccountVerifier>,
    imap: Arc<dyn ImapConnector>,
    smtp: Arc<dyn SmtpSender>,
    sessions: Mutex<HashMap<AccountId, CachedSession>>,
    /// Connections that IDLE on an INBOX (`WatchMailbox`), one per account, kept between
    /// watches.
    watchers: Mutex<HashMap<AccountId, CachedSession>>,
    idle_timeout: Duration,
    /// The full-text index of downloaded bodies (ADR 0005), if the profile has one.
    fulltext: Option<Arc<FullText>>,
    /// Where updates of dail are downloaded (the profile's `updates/`); none: no downloads.
    updates_dir: Option<std::path::PathBuf>,
}

impl LiveBackend {
    /// A backend over the given store, credential store, verifier and IMAP connector
    /// (`ProfileStore`, `KeyringSecrets`, `NetVerifier` and `NetImap` in production). Mail is
    /// submitted with [`NetSmtp`] unless [`LiveBackend::with_smtp`] says otherwise.
    pub fn new(
        store: Arc<dyn Store>,
        secrets: Arc<dyn SecretStore>,
        verifier: Arc<dyn AccountVerifier>,
        imap: Arc<dyn ImapConnector>,
    ) -> Self {
        Self {
            store,
            secrets,
            verifier,
            imap,
            smtp: Arc::new(NetSmtp::default()),
            sessions: Mutex::new(HashMap::new()),
            watchers: Mutex::new(HashMap::new()),
            idle_timeout: imap::IDLE_TIMEOUT,
            fulltext: None,
            updates_dir: None,
        }
    }

    /// Downloads updates of dail into `dir` (`ProfileDirs::updates_dir`, ADR 0011).
    #[must_use]
    pub fn with_updates_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.updates_dir = Some(dir);
        self
    }

    /// Keeps `index` up to date with the bodies it stores and searches it for `SearchIndex`.
    #[must_use]
    pub fn with_fulltext(mut self, index: Arc<FullText>) -> Self {
        self.fulltext = Some(index);
        self
    }

    /// Ends each IDLE after `timeout` instead of [`imap::IDLE_TIMEOUT`] (tests).
    #[must_use]
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// `WatchMailbox`: one IDLE on the account's watch connection (reused between watches; a
    /// reused one that fails is replaced once).
    async fn watch(
        &self,
        account: AccountId,
        mailbox: app_core::MailboxId,
        path: &str,
        login: &ImapLogin,
    ) -> Event {
        let failed = |error: String, unsupported: bool| Event::WatchFailed {
            account,
            mailbox,
            error,
            unsupported,
        };
        let cached = self
            .watchers
            .lock()
            .remove(&account)
            .filter(|cached| cached.login == *login);
        let mut reused = cached.is_some();
        let mut session = match cached {
            Some(cached) => cached.session,
            None => match self.open_imap(login).await {
                Ok(session) => session,
                Err(error) => return failed(error, false),
            },
        };
        loop {
            match imap::idle_once(session, path, self.idle_timeout).await {
                Ok((session, changed)) => {
                    self.watchers.lock().insert(
                        account,
                        CachedSession {
                            login: login.clone(),
                            session,
                        },
                    );
                    return Event::MailboxWatched {
                        account,
                        mailbox,
                        changed,
                    };
                }
                Err(imap::IdleError::Unsupported) => {
                    return failed("the server does not support IDLE".to_owned(), true);
                }
                Err(imap::IdleError::Failed(error)) if !reused => return failed(error, false),
                Err(imap::IdleError::Failed(error)) => {
                    tracing::debug!(%error, "IDLE failed on a reused connection; reconnecting");
                    reused = false;
                    session = match self.open_imap(login).await {
                        Ok(session) => session,
                        Err(error) => return failed(error, false),
                    };
                }
            }
        }
    }

    /// Uses `smtp` to submit mail (tests use a fake).
    #[must_use]
    pub fn with_smtp(mut self, smtp: Arc<dyn SmtpSender>) -> Self {
        self.smtp = smtp;
        self
    }

    /// Renders and submits a message; on success stores the copy in `sent_copy` (if given).
    /// Returns whether it was sent and, if so, the Sent copy's error.
    async fn send_message(
        &self,
        account: AccountId,
        smtp: &SmtpLogin,
        message: &OutgoingMessage,
        sent_copy: Option<SentCopy>,
    ) -> Result<Option<String>, String> {
        let raw = self.render(message, Bcc::Hide).await?;
        let password = self.password(smtp.secret.clone()).await?;
        let recipients = smtp::recipients(message);
        self.smtp
            .send(
                smtp,
                &password,
                Submission {
                    from: &message.from.email,
                    recipients: &recipients,
                    raw: &raw,
                },
            )
            .await?;
        let Some(copy) = sent_copy else {
            return Ok(None);
        };
        let stored = match self.render(message, Bcc::Keep).await {
            Ok(raw) => {
                self.with_imap(account, &copy.imap, move |session| {
                    let (path, raw) = (copy.path.clone(), raw.clone());
                    Box::pin(
                        async move { imap::append_message(session, &path, "(\\Seen)", &raw).await },
                    )
                })
                .await
            }
            Err(error) => Err(error),
        };
        Ok(stored.err())
    }

    /// Stores a raw message as a blob; returns its ID and whether it has attachments.
    async fn store_blob(&self, message: MessageId, raw: Vec<u8>) -> Result<Stored, String> {
        let store = Arc::clone(&self.store);
        let fulltext = self.fulltext.clone();
        tokio::task::spawn_blocking(move || {
            let blob = store.put_blob(&raw).map_err(|err| err.to_string())?;
            if let Some(index) = fulltext
                && let Err(err) = index.add(message, &raw)
            {
                tracing::warn!(error = %err, "could not index a body");
            }
            Ok(Stored {
                has_attachments: app_io::mime::has_attachments(&raw),
                snippet: app_io::mime::snippet(&raw),
                blob,
            })
        })
        .await
        .map_err(|err| format!("blob store task failed: {err}"))?
    }

    /// Renders an outgoing message with the quoted original from the blob store and the
    /// attached files from disk.
    async fn render(&self, message: &OutgoingMessage, bcc: Bcc) -> Result<Vec<u8>, String> {
        let original = match message.quote.as_ref().and_then(|q| q.blob.clone()) {
            Some(blob) => {
                let store = Arc::clone(&self.store);
                tokio::task::spawn_blocking(move || store.get_blob(&blob))
                    .await
                    .map_err(|err| format!("blob store task failed: {err}"))?
                    .map_err(|err| err.to_string())?
            }
            None => None,
        };
        let mut files = Vec::with_capacity(message.attachments.len());
        for attachment in &message.attachments {
            files.push(outgoing::read_file(&attachment.path).await);
        }
        outgoing::render(message, original.as_deref(), files, bcc)
    }

    /// Opens a new authenticated session for `login`.
    async fn open_imap(&self, login: &ImapLogin) -> Result<ImapSession, String> {
        let password = self.password(login.secret.clone()).await?;
        self.imap
            .connect(&login.server, &login.login, &password)
            .await
    }

    /// Runs `op` on the account's cached session (or a new one). A failure on a reused session
    /// is retried once on a fresh connection, since servers drop idle connections. A session
    /// that saw an error is discarded.
    async fn with_imap<T, F>(
        &self,
        account: AccountId,
        login: &ImapLogin,
        op: F,
    ) -> Result<T, String>
    where
        F: for<'s> Fn(&'s mut ImapSession) -> BoxFuture<'s, Result<T, String>>,
    {
        let cached = self
            .sessions
            .lock()
            .remove(&account)
            .filter(|cached| cached.login == *login);
        let mut reused = false;
        let mut session = match cached {
            // A cached session may have been dropped by the server meanwhile; a lost connection
            // can look like an empty response, so check it before use.
            Some(mut cached) => match cached.session.noop().await {
                Ok(()) => {
                    reused = true;
                    cached.session
                }
                Err(err) => {
                    tracing::debug!(error = %err, "cached IMAP session is gone; reconnecting");
                    self.open_imap(login).await?
                }
            },
            None => self.open_imap(login).await?,
        };
        loop {
            let error = match tokio::time::timeout(OP_TIMEOUT, op(&mut session)).await {
                Ok(Ok(value)) => {
                    self.sessions.lock().insert(
                        account,
                        CachedSession {
                            login: login.clone(),
                            session,
                        },
                    );
                    return Ok(value);
                }
                Ok(Err(error)) => error,
                Err(_) => format!("IMAP {}: timed out", imap::address(&login.server)),
            };
            if !reused {
                return Err(error);
            }
            tracing::debug!(%error, "IMAP operation failed on a reused session; reconnecting");
            reused = false;
            session = self.open_imap(login).await?;
        }
    }

    /// Runs a blocking credential-store call off the async workers.
    async fn with_secrets<T, F>(&self, call: F) -> Result<T, SyncError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn SecretStore) -> Result<T, SyncError> + Send + 'static,
    {
        let secrets = Arc::clone(&self.secrets);
        tokio::task::spawn_blocking(move || call(secrets.as_ref()))
            .await
            .map_err(|err| SyncError::Keyring(format!("keyring task failed: {err}")))?
    }

    async fn password(&self, secret: SecretRef) -> Result<Secret, String> {
        let user = secret.user.clone();
        match self.with_secrets(move |store| store.get(&secret)).await {
            Ok(Some(password)) => Ok(password),
            Ok(None) => Err(format!("no password stored in the keyring for {user}")),
            Err(err) => Err(err.to_string()),
        }
    }

    async fn run(&self, effect: Effect) -> Result<Vec<Event>, SyncError> {
        match effect {
            Effect::PersistState { delta } => {
                if let (Some(index), app_core::PersistDelta::Mail(mail)) = (&self.fulltext, &delta)
                {
                    index.remove(&mail.removed_messages);
                }
                let store = Arc::clone(&self.store);
                tokio::task::spawn_blocking(move || store.apply(&delta))
                    .await
                    .map_err(|err| SyncError::Effect {
                        effect: "persist_state",
                        reason: err.to_string(),
                    })??;
                Ok(vec![Event::Persisted {}])
            }
            Effect::StoreSecret {
                account,
                secret,
                value,
            } => {
                self.with_secrets(move |store| store.set(&secret, &value))
                    .await?;
                Ok(vec![Event::SecretStored { account }])
            }
            Effect::DeleteSecret { account, secret } => {
                self.with_secrets(move |store| store.delete(&secret))
                    .await?;
                Ok(vec![Event::SecretDeleted { account }])
            }
            Effect::VerifyAccount {
                account,
                login,
                imap,
                smtp,
                secret,
            } => {
                let result = match self.password(secret).await {
                    Ok(password) => self.verifier.verify(&login, &password, &imap, &smtp).await,
                    Err(err) => Err(err),
                };
                Ok(vec![match result {
                    Ok(()) => Event::AccountVerified { account },
                    Err(error) => Event::AccountVerifyFailed { account, error },
                }])
            }
            Effect::ListMailboxes { account, imap } => {
                let listed = self
                    .with_imap(account, &imap, |session| {
                        Box::pin(imap::list_mailboxes(session))
                    })
                    .await;
                Ok(vec![match listed {
                    Ok(mailboxes) => Event::MailboxesListed { account, mailboxes },
                    Err(error) => Event::SyncFailed {
                        account,
                        mailbox: None,
                        error,
                    },
                }])
            }
            Effect::FetchEnvelopes {
                account,
                mailbox,
                path,
                imap,
                uid_validity,
                since_uid,
                limit,
            } => {
                let fetched = self
                    .with_imap(account, &imap, move |session| {
                        let path = path.clone();
                        Box::pin(async move {
                            imap::fetch_envelopes(session, &path, uid_validity, since_uid, limit)
                                .await
                        })
                    })
                    .await;
                Ok(vec![match fetched {
                    Ok(fetched) => Event::EnvelopesFetched {
                        account,
                        mailbox,
                        uid_validity: fetched.uid_validity,
                        uid_next: fetched.uid_next,
                        total: fetched.total,
                        unread: fetched.unread,
                        envelopes: fetched.envelopes,
                    },
                    Err(error) => Event::SyncFailed {
                        account,
                        mailbox: Some(mailbox),
                        error,
                    },
                }])
            }
            Effect::ResyncFlags {
                account,
                mailbox,
                path,
                imap,
                uid_validity: _,
                from_uid,
                to_uid,
                changed_since,
            } => {
                let fetched = self
                    .with_imap(account, &imap, move |session| {
                        let path = path.clone();
                        Box::pin(async move {
                            imap::fetch_flags(session, &path, from_uid, to_uid, changed_since).await
                        })
                    })
                    .await;
                Ok(vec![match fetched {
                    Ok(resynced) => Event::FlagsChanged {
                        account,
                        mailbox,
                        uid_validity: resynced.uid_validity,
                        from_uid,
                        to_uid,
                        flags: resynced
                            .flags
                            .into_iter()
                            .map(|(uid, flags)| UidFlags {
                                uid,
                                flags: app_io::mime::parse_flags(flags.iter().map(String::as_str)),
                            })
                            .collect(),
                        existing: resynced.existing,
                        highest_modseq: resynced.highest_modseq,
                    },
                    Err(error) => Event::SyncFailed {
                        account,
                        mailbox: Some(mailbox),
                        error,
                    },
                }])
            }
            Effect::SearchMailbox {
                account,
                mailbox,
                path,
                imap,
                search,
                started_ms,
                mailboxes,
                query,
                gmail_raw,
                limit,
            } => {
                let arguments = crate::search::search_arguments(&query, gmail_raw);
                let found = self
                    .with_imap(account, &imap, move |session| {
                        let (path, arguments) = (path.clone(), arguments.clone());
                        Box::pin(async move {
                            imap::search_mailbox(session, &path, &arguments, limit).await
                        })
                    })
                    .await;
                Ok(vec![match found {
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
                }])
            }
            Effect::FetchBody {
                account,
                message,
                path,
                imap,
                uid_validity,
                uid,
            } => {
                let raw = self
                    .with_imap(account, &imap, move |session| {
                        let path = path.clone();
                        Box::pin(async move {
                            imap::fetch_body(session, &path, uid_validity, uid).await
                        })
                    })
                    .await;
                let stored = match raw {
                    Ok(raw) => self.store_blob(message, raw).await,
                    Err(error) => Err(error),
                };
                Ok(vec![match stored {
                    Ok(Stored {
                        blob,
                        has_attachments,
                        snippet,
                    }) => Event::BodyFetched {
                        message,
                        blob,
                        has_attachments,
                        snippet,
                    },
                    Err(error) => Event::BodyFetchFailed { message, error },
                }])
            }
            Effect::RunRemoteOp {
                op,
                account,
                imap,
                steps,
            } => {
                let done = self
                    .with_imap(account, &imap, move |session| {
                        let steps = steps.clone();
                        Box::pin(async move { imap::run_steps(session, &steps).await })
                    })
                    .await;
                Ok(match done {
                    Ok(Ok(refused)) => refused
                        .into_iter()
                        .map(|path| Event::KeywordsRefused { account, path })
                        .chain([Event::RemoteOpSucceeded { op }])
                        .collect(),
                    Ok(Err(error)) => vec![Event::RemoteOpFailed {
                        op,
                        error,
                        permanent: true,
                    }],
                    Err(error) => vec![Event::RemoteOpFailed {
                        op,
                        error,
                        permanent: false,
                    }],
                })
            }
            Effect::SaveDraft {
                draft,
                account,
                imap,
                path,
                revision,
                message,
            } => {
                let saved = match self.render(&message, Bcc::Keep).await {
                    Ok(raw) => {
                        let id = message.message_id.clone();
                        self.with_imap(account, &imap, move |session| {
                            let (path, id, raw) = (path.clone(), id.clone(), raw.clone());
                            Box::pin(
                                async move { imap::save_draft(session, &path, &id, &raw).await },
                            )
                        })
                        .await
                    }
                    Err(error) => Err(error),
                };
                Ok(vec![match saved {
                    Ok(()) => Event::DraftSaved { draft, revision },
                    Err(error) => Event::DraftSaveFailed { draft, error },
                }])
            }
            Effect::DeleteDraft {
                draft,
                account,
                imap,
                path,
                message_id,
            } => {
                let deleted = self
                    .with_imap(account, &imap, move |session| {
                        let (path, id) = (path.clone(), message_id.clone());
                        Box::pin(async move { imap::delete_draft(session, &path, &id).await })
                    })
                    .await;
                Ok(vec![match deleted {
                    Ok(()) => Event::DraftDeleted { draft },
                    Err(error) => Event::DraftDeleteFailed {
                        account,
                        draft,
                        error,
                    },
                }])
            }
            Effect::WatchMailbox {
                account,
                mailbox,
                path,
                imap,
            } => Ok(vec![self.watch(account, mailbox, &path, &imap).await]),
            Effect::ManageFolder {
                account,
                imap,
                action,
            } => {
                let wanted = action.clone();
                let done = self
                    .with_imap(account, &imap, move |session| {
                        let action = wanted.clone();
                        Box::pin(async move { imap::manage_folder(session, &action).await })
                    })
                    .await;
                Ok(vec![match done {
                    Ok(()) => Event::FolderManaged { account, action },
                    Err(error) => Event::FolderFailed {
                        account,
                        action,
                        error,
                    },
                }])
            }
            Effect::SearchIndex {
                search,
                words,
                limit,
            } => {
                let Some(index) = self.fulltext.clone() else {
                    return Ok(vec![Event::IndexSearched {
                        search,
                        hits: Vec::new(),
                    }]);
                };
                let limit = usize::try_from(limit).unwrap_or(usize::MAX);
                let found = tokio::task::spawn_blocking(move || index.search(&words, limit)).await;
                Ok(vec![match found {
                    Ok(Ok(hits)) => Event::IndexSearched { search, hits },
                    Ok(Err(err)) => Event::IndexSearchFailed {
                        search,
                        error: err.to_string(),
                    },
                    Err(err) => Event::IndexSearchFailed {
                        search,
                        error: err.to_string(),
                    },
                }])
            }
            Effect::CompactStore { keep } => {
                let store = Arc::clone(&self.store);
                let compacted = tokio::task::spawn_blocking(move || {
                    let keep: std::collections::BTreeSet<String> = keep.into_iter().collect();
                    // A body written in the last hour may not have reached the engine yet.
                    let before = std::time::SystemTime::now()
                        .checked_sub(COMPACT_GRACE)
                        .unwrap_or(std::time::UNIX_EPOCH);
                    store.compact(&keep, before)
                })
                .await;
                Ok(vec![match compacted {
                    Ok(Ok(done)) => Event::StoreCompacted {
                        removed_blobs: done.removed_blobs,
                        freed_bytes: done.freed_bytes,
                    },
                    Ok(Err(err)) => Event::CompactFailed {
                        error: err.to_string(),
                    },
                    Err(err) => Event::CompactFailed {
                        error: err.to_string(),
                    },
                }])
            }
            Effect::FetchOlder {
                account,
                mailbox,
                path,
                imap,
                uid_validity,
                before_uid,
                limit,
            } => {
                let older = self
                    .with_imap(account, &imap, move |session| {
                        let path = path.clone();
                        Box::pin(async move {
                            imap::fetch_older(session, &path, uid_validity, before_uid, limit).await
                        })
                    })
                    .await;
                Ok(vec![match older {
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
                }])
            }
            Effect::SaveAttachments {
                message,
                blob,
                parts,
                dir,
            } => {
                let raw = self.get_blob(&blob).await?;
                Ok(vec![
                    crate::saves::save_attachments(message, raw, &parts, &dir).await,
                ])
            }
            Effect::DiscoverServers { email, domain } => Ok(vec![match tls::client_config() {
                Ok(config) => crate::autoconfig::discover(&config, email, &domain).await,
                Err(err) => Event::DiscoveryFailed {
                    email,
                    error: err.to_string(),
                },
            }]),
            Effect::InspectAttachment { draft, path } => {
                Ok(vec![match inspect_file(&path).await {
                    Ok(size) => Event::AttachmentInspected {
                        draft,
                        content_type: app_io::mime::content_type_for(app_core::draft::file_name(
                            &path,
                        ))
                        .to_owned(),
                        path,
                        size,
                    },
                    Err(error) => Event::AttachmentFailed { draft, path, error },
                }])
            }
            Effect::SmtpSend {
                draft,
                account,
                smtp,
                message,
                sent_copy,
            } => Ok(vec![
                match self.send_message(account, &smtp, &message, sent_copy).await {
                    Ok(sent_copy_error) => Event::MessageSent {
                        draft,
                        sent_copy_error,
                    },
                    Err(error) => Event::SendFailed { draft, error },
                },
            ]),
            Effect::CheckForUpdates { running } => Ok(vec![match tls::client_config() {
                Ok(config) => {
                    let https = crate::https::Https::new(config, env!("CARGO_PKG_VERSION"));
                    crate::https::check(&https, &running).await
                }
                Err(err) => Event::UpdateFailed {
                    reason: err.to_string(),
                },
            }]),
            Effect::DownloadUpdate { version, artifact } => {
                let Some(dir) = self.updates_dir.clone() else {
                    return Ok(vec![Event::UpdateFailed {
                        reason: "this build does not download updates".to_owned(),
                    }]);
                };
                Ok(vec![match tls::client_config() {
                    Ok(config) => {
                        let https = crate::https::Https::new(config, env!("CARGO_PKG_VERSION"));
                        crate::https::download(&https, &dir, version, &artifact).await
                    }
                    Err(err) => Event::UpdateFailed {
                        reason: err.to_string(),
                    },
                }])
            }
            Effect::ScheduleTick { .. } | Effect::InstallUpdate { .. } => {
                Err(SyncError::Unsupported {
                    backend: self.name(),
                    effect: effect.name(),
                })
            }
        }
    }
}

/// The size of a readable regular file.
async fn inspect_file(path: &str) -> Result<u64, String> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|err| format!("cannot read {path}: {err}"))?;
    if !meta.is_file() {
        return Err(format!("{path} is not a regular file"));
    }
    // Opening checks that the file is readable now; it is read again when the draft is rendered.
    tokio::fs::File::open(path)
        .await
        .map_err(|err| format!("cannot read {path}: {err}"))?;
    Ok(meta.len())
}

impl Backend for LiveBackend {
    fn name(&self) -> &'static str {
        "live"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        Box::pin(async move {
            let state = self.store.load_snapshot()?;
            if let (Some(index), Some(state)) = (&self.fulltext, &state) {
                crate::fulltext::index_missing(Arc::clone(index), Arc::clone(&self.store), state);
            }
            Ok(state)
        })
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(self.run(effect))
    }

    fn get_blob<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Option<Vec<u8>>, SyncError>> {
        let store = Arc::clone(&self.store);
        let id = id.to_owned();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || store.get_blob(&id))
                .await
                .map_err(|err| SyncError::Effect {
                    effect: "get_blob",
                    reason: err.to_string(),
                })?
                .map_err(SyncError::from)
        })
    }
}

/// A body put in the blob store, with what the engine learns from parsing it.
struct Stored {
    blob: String,
    has_attachments: bool,
    snippet: Option<String>,
}
