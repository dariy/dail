//! Pending remote operations (`ARCHITECTURE.md §4.2, §5.2`): local changes that still have to
//! reach the server.
//!
//! A triage command changes the local cache first (optimistic) and queues a [`PendingOp`] in
//! `AppState::outbox`. The engine runs one op per account at a time, oldest first: when an op
//! starts it is turned into an `Effect::RunRemoteOp` whose [`RemoteStep`]s hold the server
//! locations (path, UIDVALIDITY, UIDs) the messages have *at that moment*. The runtime reports
//! `RemoteOpSucceeded` or `RemoteOpFailed`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, MailboxId, MessageId, OpId};
use crate::message::Flags;

/// Failed attempts after which an op the server rejects is given up (`failed`).
pub const MAX_ATTEMPTS: u32 = 8;
/// Delay before the first retry; it doubles with every further failure.
pub const RETRY_BASE_MS: i64 = 2_000;
/// Longest delay between two attempts.
pub const RETRY_MAX_MS: i64 = 300_000;

/// The delay before attempt `attempts + 1`, after `attempts` failures (at least 1).
pub fn retry_delay_ms(attempts: u32) -> i64 {
    let doublings = attempts.saturating_sub(1).min(20);
    RETRY_BASE_MS
        .saturating_mul(1_i64 << doublings)
        .min(RETRY_MAX_MS)
}

/// A message flag that triage commands change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FlagKind {
    /// `\Seen`: read.
    Seen,
    /// `\Flagged`: starred.
    Flagged,
}

impl FlagKind {
    /// The IMAP system flag, e.g. `\Seen`.
    pub const fn imap(self) -> &'static str {
        match self {
            Self::Seen => "\\Seen",
            Self::Flagged => "\\Flagged",
        }
    }

    /// Reads the flag.
    pub const fn get(self, flags: &Flags) -> bool {
        match self {
            Self::Seen => flags.seen,
            Self::Flagged => flags.flagged,
        }
    }

    /// Writes the flag.
    pub const fn set(self, flags: &mut Flags, value: bool) {
        match self {
            Self::Seen => flags.seen = value,
            Self::Flagged => flags.flagged = value,
        }
    }
}

/// What a pending op does on the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RemoteOp {
    /// Set or clear a flag (`UID STORE +FLAGS` / `-FLAGS`).
    SetFlag {
        /// The flag.
        flag: FlagKind,
        /// `true` sets it, `false` clears it.
        value: bool,
        /// The messages.
        messages: Vec<MessageId>,
    },
    /// Move messages to another mailbox (`UID MOVE`). Afterwards each message is a placeholder
    /// in `to` until a fetch of `to` finds its new UID.
    Move {
        /// Target mailbox.
        to: MailboxId,
        /// The messages, wherever they are on the server when the op starts.
        messages: Vec<MessageId>,
    },
    /// Copy messages to another mailbox (`UID COPY`): used when the source is a virtual
    /// mailbox that keeps every message (role `all`, e.g. Gmail's All Mail).
    Copy {
        /// Target mailbox.
        to: MailboxId,
        /// Source messages and their local placeholders in `to`.
        copies: Vec<CopyRef>,
    },
    /// Set or clear a flag on every message of a mailbox on the server, cached or not
    /// (`UID STORE 1:* +FLAGS`; `mark_mailbox_read`, UX-7).
    FlagMailbox {
        /// The mailbox.
        mailbox: MailboxId,
        /// The flag.
        flag: FlagKind,
        /// `true` sets it, `false` clears it.
        value: bool,
    },
}

/// One copied message: the source and its placeholder in the target mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CopyRef {
    /// The message that is copied.
    pub source: MessageId,
    /// The local placeholder of the copy (UID 0 until a fetch finds it).
    pub copy: MessageId,
}

impl RemoteOp {
    /// Every local message the op is about (sources and placeholders).
    pub fn messages(&self) -> Vec<MessageId> {
        match self {
            Self::SetFlag { messages, .. } | Self::Move { messages, .. } => messages.clone(),
            Self::Copy { copies, .. } => copies.iter().flat_map(|c| [c.source, c.copy]).collect(),
            Self::FlagMailbox { .. } => Vec::new(),
        }
    }

    /// The target mailbox of a move or copy.
    pub const fn target(&self) -> Option<MailboxId> {
        match self {
            Self::SetFlag { .. } | Self::FlagMailbox { .. } => None,
            Self::Move { to, .. } | Self::Copy { to, .. } => Some(*to),
        }
    }
}

/// Where a pending op is in its life cycle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OpState {
    /// Waiting for its turn, or for its next attempt after a failure (`next_attempt_ms`).
    #[default]
    Queued,
    /// Sent to the runtime; the result has not arrived yet.
    Running,
    /// The server rejected it repeatedly. The local change stays; `retry_pending_ops` tries
    /// again and `discard_pending_op` drops it. A failed op no longer blocks the queue.
    Failed,
}

/// A local change that still has to be applied on the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PendingOp {
    /// Op ID.
    pub id: OpId,
    /// The account whose server the op runs on.
    pub account: AccountId,
    /// What to do.
    pub op: RemoteOp,
    /// Life-cycle state.
    #[serde(default)]
    pub state: OpState,
    /// Failed attempts so far.
    #[serde(default)]
    pub attempts: u32,
    /// The last failure, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// When the op was queued, in Unix milliseconds.
    pub created_ms: i64,
    /// After a failure: not before this time (Unix milliseconds). The op still holds its place
    /// at the head of the account's queue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_attempt_ms: Option<i64>,
    /// The op waits for a fetch that locates messages whose UID is not known yet (placeholders
    /// of an earlier move). If they are still unknown afterwards, they are dropped from the op.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub awaiting_sync: bool,
}

/// One server-side action of an `Effect::RunRemoteOp`, in a mailbox selected read-write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum RemoteStep {
    /// `UID STORE uids ±FLAGS.SILENT (flag)`.
    Store {
        /// IMAP path of the mailbox.
        path: String,
        /// UIDVALIDITY the UIDs belong to; the step fails if the server's differs.
        uid_validity: u32,
        /// The messages' UIDs, ascending.
        uids: Vec<u32>,
        /// The flag.
        flag: FlagKind,
        /// `true` adds the flag, `false` removes it.
        value: bool,
    },
    /// `UID MOVE uids to` (or `UID COPY` + `UID STORE +FLAGS (\Deleted)` + `UID EXPUNGE` if the
    /// server lacks MOVE).
    Move {
        /// IMAP path of the source mailbox.
        path: String,
        /// UIDVALIDITY the UIDs belong to; the step fails if the server's differs.
        uid_validity: u32,
        /// UIDs to move, ascending.
        uids: Vec<u32>,
        /// IMAP path of the target mailbox.
        to: String,
        /// `CREATE` the target first (it may not exist yet); "already exists" is not an error.
        create: bool,
    },
    /// `UID COPY uids to`.
    Copy {
        /// IMAP path of the source mailbox.
        path: String,
        /// UIDVALIDITY the UIDs belong to; the step fails if the server's differs.
        uid_validity: u32,
        /// UIDs to copy, ascending.
        uids: Vec<u32>,
        /// IMAP path of the target mailbox.
        to: String,
        /// `CREATE` the target first; "already exists" is not an error.
        create: bool,
    },
    /// `UID STORE 1:* ±FLAGS.SILENT (flag)`: every message of the mailbox.
    StoreAll {
        /// IMAP path of the mailbox.
        path: String,
        /// UIDVALIDITY the cache knows; the step fails if the server's differs.
        uid_validity: u32,
        /// The flag.
        flag: FlagKind,
        /// `true` adds the flag, `false` removes it.
        value: bool,
    },
}
