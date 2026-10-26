//! `Event` (facts from the outside world) and `Effect` (requests to the outside world).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::account::{Account, Endpoint, ImapLogin, Secret, SecretRef, SmtpLogin};
use crate::approval::ApprovalRequest;
use crate::draft::{Draft, OutgoingMessage};
use crate::ids::{AccountId, ApprovalId, DraftId, MailboxId, MessageId, OpId, ThreadId};
use crate::mailbox::{Mailbox, RemoteMailbox};
use crate::message::{Envelope, RemoteEnvelope, Thread, UidFlags};
use crate::ops::{PendingOp, RemoteStep};
use crate::search::Query;
use crate::state::ViewPrefs;
use crate::state::{AppState, Settings};

/// A fact reported by the runtime, usually the result of an [`Effect`]. Events are never recorded
/// in undo history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", content = "args", rename_all = "snake_case")]
pub enum Event {
    /// A timer fired (see [`Effect::ScheduleTick`]).
    Tick {
        /// Time the timer fired, in Unix milliseconds.
        now_ms: i64,
    },
    /// A [`Effect::PersistState`] completed.
    Persisted {},
    /// A [`Effect::PersistState`] failed.
    PersistFailed {
        /// Diagnostic from the store.
        error: String,
    },
    /// An [`Effect::VerifyAccount`] succeeded: IMAP LOGIN and SMTP AUTH both worked.
    AccountVerified {
        /// The tested account.
        account: AccountId,
    },
    /// An [`Effect::VerifyAccount`] failed.
    AccountVerifyFailed {
        /// The tested account.
        account: AccountId,
        /// Diagnostic, e.g. `IMAP LOGIN failed: …`.
        error: String,
    },
    /// An [`Effect::StoreSecret`] completed.
    SecretStored {
        /// The account the secret belongs to.
        account: AccountId,
    },
    /// An [`Effect::DeleteSecret`] completed (also when there was nothing to delete).
    SecretDeleted {
        /// The account the secret belonged to.
        account: AccountId,
    },
    /// An [`Effect::StoreSecret`] or [`Effect::DeleteSecret`] failed.
    SecretFailed {
        /// The account the secret belongs to.
        account: AccountId,
        /// Diagnostic from the keyring.
        error: String,
    },
    /// An [`Effect::ListMailboxes`] completed: the account's mailboxes as the server lists them.
    MailboxesListed {
        /// The listed account.
        account: AccountId,
        /// Every mailbox, in server order.
        mailboxes: Vec<RemoteMailbox>,
    },
    /// An [`Effect::FetchEnvelopes`] completed.
    EnvelopesFetched {
        /// The account.
        account: AccountId,
        /// The fetched mailbox.
        mailbox: MailboxId,
        /// The mailbox's UIDVALIDITY on the server. If it differs from the cached one, the cached
        /// messages of the mailbox are dropped before `envelopes` are added.
        uid_validity: u32,
        /// The server's UIDNEXT, if reported.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uid_next: Option<u32>,
        /// Messages in the mailbox on the server.
        total: u32,
        /// Unread messages in the mailbox on the server.
        unread: u32,
        /// New messages (and flag updates of known UIDs), ascending by UID.
        envelopes: Vec<RemoteEnvelope>,
    },
    /// An [`Effect::ResyncFlags`] completed: the current flags of every UID in the range that
    /// still exists. Cached UIDs in the range that are missing were expunged.
    FlagsChanged {
        /// The account.
        account: AccountId,
        /// The resynced mailbox.
        mailbox: MailboxId,
        /// UIDVALIDITY on the server; the event is ignored if it no longer matches the cache.
        uid_validity: u32,
        /// First UID of the range.
        from_uid: u32,
        /// Last UID of the range.
        to_uid: u32,
        /// Flags of the UIDs that exist (only of those that changed, when `existing` is given).
        flags: Vec<UidFlags>,
        /// With CONDSTORE: every UID of the range that still exists (`flags` then lists only
        /// the changed ones). Without, `flags` lists every existing UID.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        existing: Option<Vec<u32>>,
        /// The server's HIGHESTMODSEQ (CONDSTORE) at the resync.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        highest_modseq: Option<u64>,
    },
    /// An [`Effect::FetchBody`] completed: the raw message is in the store.
    BodyFetched {
        /// The message.
        message: MessageId,
        /// Content address of the stored raw message.
        blob: String,
        /// Whether the parsed message has attachments.
        has_attachments: bool,
        /// The start of the parsed text ([`crate::message::snippet`]). Untrusted; the engine
        /// cleans and cuts it again.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snippet: Option<String>,
    },
    /// An [`Effect::FetchBody`] failed.
    BodyFetchFailed {
        /// The message.
        message: MessageId,
        /// Diagnostic.
        error: String,
    },
    /// An [`Effect::RunRemoteOp`] completed: every step was applied on the server.
    RemoteOpSucceeded {
        /// The op.
        op: OpId,
    },
    /// An [`Effect::RunRemoteOp`] failed. Steps before the failing one may have been applied;
    /// every step is safe to repeat.
    RemoteOpFailed {
        /// The op.
        op: OpId,
        /// Diagnostic from the server or the connection.
        error: String,
        /// The server rejected the op (a `NO`/`BAD` answer, changed UIDVALIDITY). `false`: the
        /// server could not be reached or the connection broke, so retrying later may work.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        permanent: bool,
    },
    /// The front-end is shutting down: drafts with unsaved changes are saved now instead of
    /// after the idle delay.
    Closing {},
    /// An [`Effect::SaveDraft`] stored the draft in the Drafts mailbox (and removed older
    /// copies).
    DraftSaved {
        /// The draft.
        draft: DraftId,
        /// The `revision` of the effect.
        revision: i64,
    },
    /// An [`Effect::SaveDraft`] failed. The draft stays local and is saved again later.
    DraftSaveFailed {
        /// The draft.
        draft: DraftId,
        /// Diagnostic.
        error: String,
    },
    /// An [`Effect::DeleteDraft`] removed the server copies of a discarded or sent draft.
    DraftDeleted {
        /// The draft.
        draft: DraftId,
    },
    /// An [`Effect::DeleteDraft`] failed; the server copy stays in the Drafts mailbox.
    DraftDeleteFailed {
        /// The account.
        account: AccountId,
        /// The draft.
        draft: DraftId,
        /// Diagnostic.
        error: String,
    },
    /// An [`Effect::SmtpSend`] delivered the message to the SMTP server.
    MessageSent {
        /// The draft that was sent.
        draft: DraftId,
        /// Storing the copy in Sent failed (the message was sent nevertheless).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sent_copy_error: Option<String>,
    },
    /// An [`Effect::SmtpSend`] failed: nothing was sent and the draft is editable again.
    SendFailed {
        /// The draft.
        draft: DraftId,
        /// Diagnostic from the SMTP server or the connection.
        error: String,
    },
    /// An [`Effect::InspectAttachment`] found the file.
    AttachmentInspected {
        /// The draft.
        draft: DraftId,
        /// The file's path, as recorded in the draft.
        path: String,
        /// Size in bytes.
        size: u64,
        /// MIME type guessed from the file name.
        content_type: String,
    },
    /// An [`Effect::InspectAttachment`] failed: the file is missing, not a regular file or not
    /// readable.
    AttachmentFailed {
        /// The draft.
        draft: DraftId,
        /// The file's path, as recorded in the draft.
        path: String,
        /// Diagnostic.
        error: String,
    },
    /// An [`Effect::DiscoverServers`] found the servers of a domain.
    ServersDiscovered {
        /// The address looked up.
        email: String,
        /// Its IMAP server.
        imap: Endpoint,
        /// Its SMTP submission server.
        smtp: Endpoint,
        /// Where they were found (the URL).
        source: String,
    },
    /// An [`Effect::DiscoverServers`] found nothing usable.
    DiscoveryFailed {
        /// The address looked up.
        email: String,
        /// Why (the last place asked and its answer).
        error: String,
    },
    /// An [`Effect::SearchIndex`] completed.
    IndexSearched {
        /// The search key (the canonical query, lower-cased).
        search: String,
        /// The messages found, best first.
        hits: Vec<MessageId>,
    },
    /// An [`Effect::SearchIndex`] failed.
    IndexSearchFailed {
        /// The search key.
        search: String,
        /// Diagnostic.
        error: String,
    },
    /// An [`Effect::CompactStore`] completed.
    StoreCompacted {
        /// Bodies deleted.
        removed_blobs: u64,
        /// Bytes freed by them.
        freed_bytes: u64,
    },
    /// An [`Effect::CompactStore`] failed.
    CompactFailed {
        /// Diagnostic.
        error: String,
    },
    /// An [`Effect::FetchOlder`] completed: envelopes of messages older than the cached ones.
    OlderFetched {
        /// The account.
        account: AccountId,
        /// The mailbox.
        mailbox: MailboxId,
        /// UIDVALIDITY on the server; the event is ignored if it no longer matches the cache.
        uid_validity: u32,
        /// The envelopes, ascending by UID.
        envelopes: Vec<RemoteEnvelope>,
        /// The oldest message of the mailbox is among them (or there was nothing older).
        complete: bool,
    },
    /// An [`Effect::FetchOlder`] failed; the next fetch of the mailbox tries again.
    BackfillFailed {
        /// The account.
        account: AccountId,
        /// The mailbox.
        mailbox: MailboxId,
        /// Diagnostic.
        error: String,
    },
    /// An [`Effect::WatchMailbox`] ended normally: the server reported a change (new mail,
    /// flags, an expunge) or the wait timed out quietly.
    MailboxWatched {
        /// The account.
        account: AccountId,
        /// The watched mailbox.
        mailbox: MailboxId,
        /// Whether the server reported a change.
        changed: bool,
    },
    /// An [`Effect::WatchMailbox`] failed (or the server has no IDLE).
    WatchFailed {
        /// The account.
        account: AccountId,
        /// The watched mailbox.
        mailbox: MailboxId,
        /// Diagnostic.
        error: String,
        /// The server does not support IDLE: don't try again.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        unsupported: bool,
    },
    /// An [`Effect::SaveAttachments`] wrote every requested attachment.
    AttachmentsSaved {
        /// The message.
        message: MessageId,
        /// The files written, as absolute paths.
        files: Vec<String>,
    },
    /// An [`Effect::SaveAttachments`] failed (the files written before the failure are listed).
    AttachmentsSaveFailed {
        /// The message.
        message: MessageId,
        /// The files written before the failure.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<String>,
        /// Diagnostic.
        error: String,
    },
    /// A sync effect (listing, fetching) failed.
    SyncFailed {
        /// The account being synced.
        account: AccountId,
        /// The mailbox being synced, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mailbox: Option<MailboxId>,
        /// Diagnostic from the server or the connection.
        error: String,
    },
    /// An [`Effect::SearchMailbox`] completed: the newest messages of the mailbox that match.
    SearchResults {
        /// The account.
        account: AccountId,
        /// The searched mailbox.
        mailbox: MailboxId,
        /// The mailbox's UIDVALIDITY on the server; results are not merged if it no longer
        /// matches the cache.
        uid_validity: u32,
        /// The search key (the canonical query, lower-cased) the results belong to.
        search: String,
        /// When the search round started, in Unix milliseconds (from the effect).
        started_ms: i64,
        /// Mailboxes searched in that round (from the effect).
        mailboxes: u32,
        /// The matching messages, ascending by UID.
        envelopes: Vec<RemoteEnvelope>,
    },
    /// An [`Effect::SearchMailbox`] failed.
    SearchFailed {
        /// The account.
        account: AccountId,
        /// The mailbox that could not be searched.
        mailbox: MailboxId,
        /// The search key (the canonical query, lower-cased).
        search: String,
        /// When the search round started, in Unix milliseconds (from the effect).
        started_ms: i64,
        /// Mailboxes searched in that round (from the effect).
        mailboxes: u32,
        /// Diagnostic from the server or the connection.
        error: String,
    },
}

impl Event {
    /// The event's `snake_case` name, equal to its serde tag.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Tick { .. } => "tick",
            Self::Persisted {} => "persisted",
            Self::PersistFailed { .. } => "persist_failed",
            Self::AccountVerified { .. } => "account_verified",
            Self::AccountVerifyFailed { .. } => "account_verify_failed",
            Self::SecretStored { .. } => "secret_stored",
            Self::SecretDeleted { .. } => "secret_deleted",
            Self::SecretFailed { .. } => "secret_failed",
            Self::MailboxesListed { .. } => "mailboxes_listed",
            Self::EnvelopesFetched { .. } => "envelopes_fetched",
            Self::FlagsChanged { .. } => "flags_changed",
            Self::BodyFetched { .. } => "body_fetched",
            Self::BodyFetchFailed { .. } => "body_fetch_failed",
            Self::RemoteOpSucceeded { .. } => "remote_op_succeeded",
            Self::RemoteOpFailed { .. } => "remote_op_failed",
            Self::SyncFailed { .. } => "sync_failed",
            Self::Closing {} => "closing",
            Self::DraftSaved { .. } => "draft_saved",
            Self::DraftSaveFailed { .. } => "draft_save_failed",
            Self::DraftDeleted { .. } => "draft_deleted",
            Self::DraftDeleteFailed { .. } => "draft_delete_failed",
            Self::MessageSent { .. } => "message_sent",
            Self::SendFailed { .. } => "send_failed",
            Self::AttachmentInspected { .. } => "attachment_inspected",
            Self::AttachmentFailed { .. } => "attachment_failed",
            Self::AttachmentsSaved { .. } => "attachments_saved",
            Self::MailboxWatched { .. } => "mailbox_watched",
            Self::OlderFetched { .. } => "older_fetched",
            Self::StoreCompacted { .. } => "store_compacted",
            Self::IndexSearched { .. } => "index_searched",
            Self::ServersDiscovered { .. } => "servers_discovered",
            Self::DiscoveryFailed { .. } => "discovery_failed",
            Self::IndexSearchFailed { .. } => "index_search_failed",
            Self::CompactFailed { .. } => "compact_failed",
            Self::BackfillFailed { .. } => "backfill_failed",
            Self::WatchFailed { .. } => "watch_failed",
            Self::AttachmentsSaveFailed { .. } => "attachments_save_failed",
            Self::SearchResults { .. } => "search_results",
            Self::SearchFailed { .. } => "search_failed",
        }
    }
}

/// A side effect the runtime must perform on the engine's behalf.
///
/// Effects may carry a [`Secret`]; its `Serialize` and `Debug` output is redacted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "effect", content = "args", rename_all = "snake_case")]
pub enum Effect {
    /// Write a change of durable state to the local store.
    PersistState {
        /// What changed.
        delta: PersistDelta,
    },
    /// Deliver an [`Event::Tick`] at the given time.
    ScheduleTick {
        /// When to fire, in Unix milliseconds.
        at_ms: i64,
    },
    /// Store a password in the OS keyring (overwriting any previous one).
    StoreSecret {
        /// The account the secret belongs to.
        account: AccountId,
        /// Keyring entry.
        secret: SecretRef,
        /// The password.
        value: Secret,
    },
    /// Delete a password from the OS keyring. Deleting a missing entry succeeds.
    DeleteSecret {
        /// The account the secret belonged to.
        account: AccountId,
        /// Keyring entry.
        secret: SecretRef,
    },
    /// Log in to the IMAP server and authenticate to the SMTP server, then disconnect. The
    /// password is read from the keyring.
    VerifyAccount {
        /// The account under test.
        account: AccountId,
        /// IMAP/SMTP login (the email address).
        login: String,
        /// IMAP server.
        imap: Endpoint,
        /// SMTP server.
        smtp: Endpoint,
        /// Keyring entry of the password.
        secret: SecretRef,
    },
    /// List the account's mailboxes (IMAP `LIST` with SPECIAL-USE attributes).
    ListMailboxes {
        /// The account to list.
        account: AccountId,
        /// How to reach its IMAP server.
        imap: ImapLogin,
    },
    /// Fetch envelopes of a mailbox: `SELECT`, then the newest `limit` messages if nothing is
    /// cached (or UIDVALIDITY changed), else every message with a UID of at least `since_uid`
    /// (at most the newest `limit`).
    FetchEnvelopes {
        /// The account.
        account: AccountId,
        /// The mailbox to fetch.
        mailbox: MailboxId,
        /// Its IMAP path.
        path: String,
        /// How to reach the IMAP server.
        imap: ImapLogin,
        /// UIDVALIDITY of the cached messages (`None`: nothing cached).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uid_validity: Option<u32>,
        /// First UID to fetch (`None`: initial window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since_uid: Option<u32>,
        /// Maximum number of envelopes (`settings.initial_window`).
        limit: u32,
    },
    /// Re-read the flags of the cached UIDs `from_uid..=to_uid` (`UID FETCH from:to (FLAGS)`);
    /// UIDs that no longer exist were expunged.
    ResyncFlags {
        /// The account.
        account: AccountId,
        /// The mailbox.
        mailbox: MailboxId,
        /// Its IMAP path.
        path: String,
        /// How to reach the IMAP server.
        imap: ImapLogin,
        /// UIDVALIDITY of the cached messages.
        uid_validity: u32,
        /// First UID.
        from_uid: u32,
        /// Last UID.
        to_uid: u32,
        /// The mailbox's HIGHESTMODSEQ at the last resync: a CONDSTORE server then sends only
        /// the flags that changed since (`CHANGEDSINCE`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        changed_since: Option<u64>,
    },
    /// Download one raw message (`UID FETCH uid (BODY.PEEK[])`) into the blob store.
    FetchBody {
        /// The account.
        account: AccountId,
        /// The message.
        message: MessageId,
        /// IMAP path of its mailbox.
        path: String,
        /// How to reach the IMAP server.
        imap: ImapLogin,
        /// UIDVALIDITY the UID belongs to.
        uid_validity: u32,
        /// The message's UID.
        uid: u32,
    },
    /// Send a draft: render `message` (Bcc only in the envelope), submit it over SMTP to every
    /// `To`/`Cc`/`Bcc` recipient, then `APPEND` the copy (with Bcc) to `sent_copy` if given.
    /// Reports `MessageSent` or `SendFailed`.
    SmtpSend {
        /// The draft.
        draft: DraftId,
        /// The account.
        account: AccountId,
        /// How to reach the SMTP server.
        smtp: SmtpLogin,
        /// The message to render and send.
        message: Box<OutgoingMessage>,
        /// Where to store the sent copy (`None`: the server does it itself, as Gmail does, or
        /// the account has no Sent mailbox).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sent_copy: Option<SentCopy>,
    },
    /// Store a draft in the account's Drafts mailbox: render `message` (reading the quoted blob
    /// and the attached files), `APPEND` it with `\Seen \Draft`, then delete the older copies
    /// (same `Message-ID`). Reports `DraftSaved` or `DraftSaveFailed`.
    SaveDraft {
        /// The draft.
        draft: DraftId,
        /// The account.
        account: AccountId,
        /// How to reach the IMAP server.
        imap: ImapLogin,
        /// IMAP path of the Drafts mailbox.
        path: String,
        /// `updated_ms` of the draft version being saved.
        revision: i64,
        /// The draft to render.
        message: Box<OutgoingMessage>,
    },
    /// Delete every copy of a draft (by `Message-ID`) from the Drafts mailbox. Reports
    /// `DraftDeleted` or `DraftDeleteFailed`.
    DeleteDraft {
        /// The draft.
        draft: DraftId,
        /// The account.
        account: AccountId,
        /// How to reach the IMAP server.
        imap: ImapLogin,
        /// IMAP path of the Drafts mailbox.
        path: String,
        /// The draft's `Message-ID`, without angle brackets.
        message_id: String,
    },
    /// Check a local file attached to a draft: that it is a readable regular file, its size and
    /// MIME type. Reports `AttachmentInspected` or `AttachmentFailed`.
    InspectAttachment {
        /// The draft.
        draft: DraftId,
        /// Absolute path of the file.
        path: String,
    },
    /// Look up a domain's mail servers over HTTPS (ADR 0006). Reports `ServersDiscovered` or
    /// `DiscoveryFailed`.
    DiscoverServers {
        /// The address (reported back).
        email: String,
        /// Its domain, the only part sent.
        domain: String,
    },
    /// Search the local full-text index of downloaded bodies (ADR 0005) for messages that
    /// contain every word of `words`. Reports `IndexSearched` or `IndexSearchFailed`.
    SearchIndex {
        /// The search key (the canonical query, lower-cased).
        search: String,
        /// The text terms of the query.
        words: Vec<String>,
        /// At most this many hits.
        limit: u32,
    },
    /// Compact the local store and delete the bodies not in `keep` that were written more than
    /// an hour ago. Reports `StoreCompacted` or `CompactFailed`.
    CompactStore {
        /// The bodies cached messages refer to.
        keep: Vec<String>,
    },
    /// Fetch the envelopes of up to `limit` messages older than `before_uid` (the oldest cached
    /// one), the newest of them first in line: the history backfill. Reports `OlderFetched` or
    /// `BackfillFailed`.
    FetchOlder {
        /// The account.
        account: AccountId,
        /// The mailbox.
        mailbox: MailboxId,
        /// Its server path.
        path: String,
        /// How to log in.
        imap: ImapLogin,
        /// UIDVALIDITY of the cached messages.
        uid_validity: u32,
        /// The oldest cached UID.
        before_uid: u32,
        /// At most this many envelopes.
        limit: u32,
    },
    /// Wait on the server (IMAP IDLE, on a connection of its own) until `path` changes or a
    /// quiet timeout passes. Reports `MailboxWatched` or `WatchFailed`. Runs beside the other
    /// effects; it does not hold them up.
    WatchMailbox {
        /// The account.
        account: AccountId,
        /// The mailbox (the INBOX).
        mailbox: MailboxId,
        /// Its server path.
        path: String,
        /// How to log in.
        imap: ImapLogin,
    },
    /// Write attachments of a stored message body into a local folder, under their (made safe)
    /// file names, never overwriting a file. Reports `AttachmentsSaved` or
    /// `AttachmentsSaveFailed`.
    SaveAttachments {
        /// The message.
        message: MessageId,
        /// Its stored body (`BodyState::Fetched`).
        blob: String,
        /// MIME part numbers of the attachments; empty means every attachment.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<usize>,
        /// Absolute path of an existing folder.
        dir: String,
    },
    /// Apply a pending op on the server: run `steps` in order on one IMAP connection.
    RunRemoteOp {
        /// The op (reported back in `RemoteOpSucceeded` / `RemoteOpFailed`).
        op: OpId,
        /// The account.
        account: AccountId,
        /// How to reach the IMAP server.
        imap: ImapLogin,
        /// What to do, mailbox by mailbox.
        steps: Vec<RemoteStep>,
    },
    /// Search one mailbox on the server (`EXAMINE`, then `UID SEARCH` with the query's
    /// criteria, or Gmail's `X-GM-RAW`) and fetch the envelopes of the newest `limit` hits.
    SearchMailbox {
        /// The account.
        account: AccountId,
        /// The mailbox to search.
        mailbox: MailboxId,
        /// Its IMAP path.
        path: String,
        /// How to reach the IMAP server.
        imap: ImapLogin,
        /// The search key (the canonical query, lower-cased) the results are reported for.
        search: String,
        /// When the search round started (`Ctx::now_ms`); identifies the round.
        started_ms: i64,
        /// Mailboxes searched in this round, so the engine knows when the round is complete.
        mailboxes: u32,
        /// What to search for in this mailbox (the search without its `in:` terms).
        query: Query,
        /// Send the query in Gmail's syntax (`X-GM-RAW`) instead of IMAP criteria.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        gmail_raw: bool,
        /// Maximum number of hits returned (the newest).
        limit: u32,
    },
}

impl Effect {
    /// The effect's `snake_case` name, equal to its serde tag.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::PersistState { .. } => "persist_state",
            Self::ScheduleTick { .. } => "schedule_tick",
            Self::StoreSecret { .. } => "store_secret",
            Self::DeleteSecret { .. } => "delete_secret",
            Self::VerifyAccount { .. } => "verify_account",
            Self::ListMailboxes { .. } => "list_mailboxes",
            Self::FetchEnvelopes { .. } => "fetch_envelopes",
            Self::ResyncFlags { .. } => "resync_flags",
            Self::FetchBody { .. } => "fetch_body",
            Self::RunRemoteOp { .. } => "run_remote_op",
            Self::InspectAttachment { .. } => "inspect_attachment",
            Self::SaveAttachments { .. } => "save_attachments",
            Self::WatchMailbox { .. } => "watch_mailbox",
            Self::FetchOlder { .. } => "fetch_older",
            Self::CompactStore { .. } => "compact_store",
            Self::SearchIndex { .. } => "search_index",
            Self::DiscoverServers { .. } => "discover_servers",
            Self::SaveDraft { .. } => "save_draft",
            Self::SmtpSend { .. } => "smtp_send",
            Self::DeleteDraft { .. } => "delete_draft",
            Self::SearchMailbox { .. } => "search_mailbox",
        }
    }

    /// The event that reports this effect's failure, so that failures surface in state.
    pub fn failure_event(&self, error: String) -> Event {
        match self {
            Self::PersistState { .. } => Event::PersistFailed { error },
            // A timer cannot fail meaningfully; deliver the tick so scheduled work still runs.
            Self::ScheduleTick { at_ms } => Event::Tick { now_ms: *at_ms },
            Self::StoreSecret { account, .. } | Self::DeleteSecret { account, .. } => {
                Event::SecretFailed {
                    account: *account,
                    error,
                }
            }
            Self::VerifyAccount { account, .. } => Event::AccountVerifyFailed {
                account: *account,
                error,
            },
            Self::ListMailboxes { account, .. } => Event::SyncFailed {
                account: *account,
                mailbox: None,
                error,
            },
            Self::FetchEnvelopes {
                account, mailbox, ..
            }
            | Self::ResyncFlags {
                account, mailbox, ..
            } => Event::SyncFailed {
                account: *account,
                mailbox: Some(*mailbox),
                error,
            },
            Self::FetchBody { message, .. } => Event::BodyFetchFailed {
                message: *message,
                error,
            },
            Self::RunRemoteOp { op, .. } => Event::RemoteOpFailed {
                op: *op,
                error,
                permanent: false,
            },
            Self::InspectAttachment { draft, path } => Event::AttachmentFailed {
                draft: *draft,
                path: path.clone(),
                error,
            },
            Self::CompactStore { .. } => Event::CompactFailed { error },
            Self::DiscoverServers { email, .. } => Event::DiscoveryFailed {
                email: email.clone(),
                error,
            },
            Self::SearchIndex { search, .. } => Event::IndexSearchFailed {
                search: search.clone(),
                error,
            },
            Self::FetchOlder {
                account, mailbox, ..
            } => Event::BackfillFailed {
                account: *account,
                mailbox: *mailbox,
                error,
            },
            Self::WatchMailbox {
                account, mailbox, ..
            } => Event::WatchFailed {
                account: *account,
                mailbox: *mailbox,
                error,
                unsupported: false,
            },
            Self::SaveAttachments { message, .. } => Event::AttachmentsSaveFailed {
                message: *message,
                files: Vec::new(),
                error,
            },
            Self::SmtpSend { draft, .. } => Event::SendFailed {
                draft: *draft,
                error,
            },
            Self::SaveDraft { draft, .. } => Event::DraftSaveFailed {
                draft: *draft,
                error,
            },
            Self::DeleteDraft { draft, account, .. } => Event::DraftDeleteFailed {
                account: *account,
                draft: *draft,
                error,
            },
            Self::SearchMailbox {
                account,
                mailbox,
                search,
                started_ms,
                mailboxes,
                ..
            } => Event::SearchFailed {
                account: *account,
                mailbox: *mailbox,
                search: search.clone(),
                started_ms: *started_ms,
                mailboxes: *mailboxes,
                error,
            },
        }
    }
}

/// Where `SmtpSend` stores the sent message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SentCopy {
    /// How to reach the IMAP server.
    pub imap: ImapLogin,
    /// IMAP path of the Sent mailbox.
    pub path: String,
}

/// A change to durable state. Variants grow with the entities that the store persists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "table", content = "value", rename_all = "snake_case")]
pub enum PersistDelta {
    /// The full settings record.
    Settings(Settings),
    /// One account, inserted or replaced.
    Account(Account),
    /// An account was removed.
    AccountRemoved(AccountId),
    /// A change to the local mail cache.
    Mail(MailDelta),
    /// The saved view preferences (`[view]` in `config.toml`, UX-1).
    View(ViewPrefs),
}

/// Upserts and removals in the local mail cache. Removals are applied before upserts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MailDelta {
    /// Mailboxes inserted or replaced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mailboxes: Vec<Mailbox>,
    /// Mailboxes removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_mailboxes: Vec<MailboxId>,
    /// Envelopes inserted or replaced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<Envelope>,
    /// Envelopes removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_messages: Vec<MessageId>,
    /// Threads inserted or replaced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threads: Vec<Thread>,
    /// Threads removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_threads: Vec<ThreadId>,
    /// Pending ops inserted or replaced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ops: Vec<PendingOp>,
    /// Pending ops removed (done or discarded).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_ops: Vec<OpId>,
    /// Drafts inserted or replaced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drafts: Vec<Draft>,
    /// Drafts removed (discarded or sent).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_drafts: Vec<DraftId>,
    /// Approval requests inserted or replaced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<ApprovalRequest>,
    /// Approval requests removed (decided, withdrawn or gone with their draft).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_approvals: Vec<ApprovalId>,
}

impl MailDelta {
    /// Whether the delta changes nothing.
    pub fn is_empty(&self) -> bool {
        self.mailboxes.is_empty()
            && self.removed_mailboxes.is_empty()
            && self.messages.is_empty()
            && self.removed_messages.is_empty()
            && self.threads.is_empty()
            && self.removed_threads.is_empty()
            && self.ops.is_empty()
            && self.removed_ops.is_empty()
            && self.drafts.is_empty()
            && self.removed_drafts.is_empty()
            && self.approvals.is_empty()
            && self.removed_approvals.is_empty()
    }

    /// Applies the delta to `state`.
    pub fn apply_to(&self, state: &mut AppState) {
        for id in &self.removed_mailboxes {
            state.mailboxes.shift_remove(id);
        }
        for id in &self.removed_messages {
            state.messages.shift_remove(id);
        }
        for id in &self.removed_threads {
            state.threads.shift_remove(id);
        }
        for id in &self.removed_ops {
            state.outbox.shift_remove(id);
        }
        for id in &self.removed_drafts {
            state.drafts.shift_remove(id);
        }
        for id in &self.removed_approvals {
            state.approvals.shift_remove(id);
        }
        for mailbox in &self.mailboxes {
            state.mailboxes.insert(mailbox.id, mailbox.clone());
        }
        for message in &self.messages {
            state.messages.insert(message.id, message.clone());
        }
        for thread in &self.threads {
            state.threads.insert(thread.id, thread.clone());
        }
        for op in &self.ops {
            state.outbox.insert(op.id, op.clone());
        }
        for draft in &self.drafts {
            state.drafts.insert(draft.id, draft.clone());
        }
        for approval in &self.approvals {
            state.approvals.insert(approval.id, approval.clone());
        }
    }
}

impl PersistDelta {
    /// Applies the delta to a persisted snapshot. Used by stores that keep whole snapshots.
    pub fn apply_to(&self, state: &mut AppState) {
        match self {
            Self::Settings(settings) => state.settings = settings.clone(),
            Self::Account(account) => {
                state.accounts.insert(account.id, account.clone());
            }
            Self::AccountRemoved(id) => {
                state.accounts.shift_remove(id);
            }
            Self::Mail(delta) => delta.apply_to(state),
            Self::View(prefs) => state.view.apply_prefs(prefs),
        }
    }
}
