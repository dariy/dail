//! Messages (envelopes) and threads (`ARCHITECTURE.md §4.2`). Envelopes hold metadata only;
//! bodies live in the store as blobs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, MailboxId, MessageId, ThreadId};

/// A mail address with an optional display name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Addr {
    /// Display name, e.g. `Alice Example`. Untrusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Address, e.g. `alice@example.com`.
    pub email: String,
}

/// IMAP system flags of a message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Flags {
    /// `\Seen`: the message has been read.
    pub seen: bool,
    /// `\Flagged`: starred.
    pub flagged: bool,
    /// `\Answered`: replied to.
    pub answered: bool,
    /// `\Draft`: an unsent draft.
    pub draft: bool,
}

/// The flags of one UID, as reported by a flag resync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UidFlags {
    /// IMAP UID.
    pub uid: u32,
    /// Its current flags.
    pub flags: Flags,
}

/// Whether the message body is in the local store.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BodyState {
    /// Only the envelope is cached.
    #[default]
    NotFetched,
    /// The raw message is stored as a blob.
    Fetched {
        /// Content address (lower-case hex SHA-256) of the raw RFC 822 message.
        blob: String,
    },
    /// Fetching the body failed.
    Failed {
        /// Diagnostic.
        error: String,
    },
}

/// One message copy: a `(mailbox, uid)` pair with its header metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope {
    /// Message ID (local).
    pub id: MessageId,
    /// The account the message belongs to.
    pub account: AccountId,
    /// The mailbox holding this copy.
    pub mailbox: MailboxId,
    /// IMAP UID within the mailbox.
    pub uid: u32,
    /// `Message-ID` header, without angle brackets.
    pub message_id_hdr: Option<String>,
    /// `In-Reply-To` header (first ID), without angle brackets.
    pub in_reply_to: Option<String>,
    /// `References` header IDs, oldest first, without angle brackets.
    pub references: Vec<String>,
    /// The conversation this message belongs to.
    pub thread: ThreadId,
    /// `From` addresses. Untrusted.
    pub from: Vec<Addr>,
    /// `To` addresses. Untrusted.
    pub to: Vec<Addr>,
    /// `Cc` addresses. Untrusted.
    pub cc: Vec<Addr>,
    /// Decoded subject. Untrusted.
    pub subject: String,
    /// `Date` header in Unix milliseconds (0 if missing or unparsable).
    pub date_ms: i64,
    /// IMAP flags.
    pub flags: Flags,
    /// RFC 822 size in bytes.
    pub size: u32,
    /// Whether the message probably has attachments.
    pub has_attachments: bool,
    /// Whether the body is cached.
    pub body: BodyState,
    /// Where the message is shown while a move has not reached the server yet. `mailbox` and
    /// `uid` always describe the message's server location (`uid` 0: a placeholder whose new
    /// UID a fetch of `mailbox` has not found yet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<MailboxId>,
}

impl Envelope {
    /// The mailbox the message is shown in: the target of a pending move, else `mailbox`.
    pub fn location(&self) -> MailboxId {
        self.moved_to.unwrap_or(self.mailbox)
    }

    /// Whether the message's UID on the server is not known yet.
    pub const fn is_placeholder(&self) -> bool {
        self.uid == 0
    }
}

/// A conversation: messages of one account linked by `Message-ID` / `References`, or by
/// subject as a fallback (see [`crate::threading`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Thread {
    /// Thread ID.
    pub id: ThreadId,
    /// The account the thread belongs to.
    pub account: AccountId,
    /// Messages, oldest first.
    pub messages: Vec<MessageId>,
    /// Subject of the oldest message. Untrusted.
    pub subject: String,
    /// Date of the newest message, in Unix milliseconds.
    pub last_date_ms: i64,
}

/// A message as fetched from the server (`UID FETCH`): header metadata, flags and size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RemoteEnvelope {
    /// IMAP UID.
    pub uid: u32,
    /// `Message-ID`, without angle brackets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id_hdr: Option<String>,
    /// `In-Reply-To` (first ID), without angle brackets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    /// `References`, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    /// `From`.
    #[serde(default)]
    pub from: Vec<Addr>,
    /// `To`.
    #[serde(default)]
    pub to: Vec<Addr>,
    /// `Cc`.
    #[serde(default)]
    pub cc: Vec<Addr>,
    /// Decoded subject.
    #[serde(default)]
    pub subject: String,
    /// `Date` in Unix milliseconds (0 if missing).
    #[serde(default)]
    pub date_ms: i64,
    /// IMAP flags.
    #[serde(default)]
    pub flags: Flags,
    /// RFC 822 size in bytes.
    #[serde(default)]
    pub size: u32,
    /// Whether the message probably has attachments.
    #[serde(default)]
    pub has_attachments: bool,
}

impl RemoteEnvelope {
    /// The cached envelope for this message.
    pub fn into_envelope(
        self,
        id: MessageId,
        account: AccountId,
        mailbox: MailboxId,
        thread: ThreadId,
    ) -> Envelope {
        Envelope {
            id,
            account,
            mailbox,
            uid: self.uid,
            message_id_hdr: self.message_id_hdr,
            in_reply_to: self.in_reply_to,
            references: self.references,
            thread,
            from: self.from,
            to: self.to,
            cc: self.cc,
            subject: self.subject,
            date_ms: self.date_ms,
            flags: self.flags,
            size: self.size,
            has_attachments: self.has_attachments,
            body: BodyState::NotFetched,
            moved_to: None,
        }
    }
}
