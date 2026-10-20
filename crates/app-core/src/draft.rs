//! Drafts (`ARCHITECTURE.md §4.2`): messages being written. A draft lives in `AppState::drafts`
//! (local-first) and is persisted with the mail cache.
//!
//! Replies and forwards keep a [`Quote`] of the original message instead of its text: the core
//! cannot read bodies, so the runtime adds the quoted text when it renders the draft.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::date::civil_from_days;
use crate::ids::{AccountId, DraftId, MessageId};
use crate::message::{Addr, Envelope};

/// A message being written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Draft {
    /// Draft ID.
    pub id: DraftId,
    /// The account that sends it.
    pub account: AccountId,
    /// The address it is sent from (`set_draft_from`); the account's own if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Addr>,
    /// The message this draft answers or forwards, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<MessageId>,
    /// `Message-ID` the draft is saved and sent with, without angle brackets. Stays the same
    /// for the draft's whole life.
    pub message_id_hdr: String,
    /// `In-Reply-To` header (a reply's parent), without angle brackets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    /// `References` header IDs, oldest first, without angle brackets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    /// `To` recipients.
    #[serde(default)]
    pub to: Vec<Addr>,
    /// `Cc` recipients.
    #[serde(default)]
    pub cc: Vec<Addr>,
    /// `Bcc` recipients (never shown to the other recipients).
    #[serde(default)]
    pub bcc: Vec<Addr>,
    /// Subject.
    #[serde(default)]
    pub subject: String,
    /// Plain-text body written by the user or agent (without the quoted original).
    #[serde(default)]
    pub body_text: String,
    /// The original message quoted below the body (replies) or forwarded (forwards).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<Quote>,
    /// Files to attach, in order. The runtime reads them when it renders the draft.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AttachmentRef>,
    /// When the draft was created, in Unix milliseconds.
    pub created_ms: i64,
    /// When the draft was last changed, in Unix milliseconds.
    pub updated_ms: i64,
    /// `updated_ms` of the version stored in the account's Drafts mailbox (`None`: not saved
    /// on the server yet). The draft is saved again when it differs from `updated_ms`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_ms: Option<i64>,
    /// Why the last save to the server failed, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_error: Option<String>,
    /// When a scheduled send leaves (Unix milliseconds). Until then `cancel_send` (or undo)
    /// stops it; the draft can't be edited meanwhile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_at: Option<i64>,
    /// The message has been handed to the runtime for SMTP; it can't be stopped any more.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sending: bool,
    /// Why the last send failed, if it did (the draft is back to editable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_error: Option<String>,
}

/// How long a draft must be left unchanged before it is saved to the server (milliseconds).
pub const DRAFT_SAVE_IDLE_MS: i64 = 5_000;
/// Delay before a failed save is tried again (milliseconds).
pub const DRAFT_SAVE_RETRY_MS: i64 = 60_000;

/// Largest total size of a draft's attachments, in bytes (25 MiB: what Gmail accepts, and more
/// than most servers want after Base64).
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;

/// A local file attached to a draft. The core only records the path; the runtime checks the file
/// (`InspectAttachment`) and reads it when the draft is rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentRef {
    /// Absolute path of the file.
    pub path: String,
    /// File name shown to recipients (the last path component).
    pub name: String,
    /// Whether the file was found and fits the size limit.
    pub state: AttachmentState,
}

/// The check of an attached file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AttachmentState {
    /// The runtime is checking the file.
    Checking,
    /// The file exists and fits the limit.
    Ready {
        /// Size in bytes.
        size: u64,
        /// MIME type, guessed from the file name.
        content_type: String,
    },
    /// The file is missing, unreadable or too large; the draft can't be sent with it.
    Failed {
        /// Why.
        error: String,
    },
}

impl AttachmentRef {
    /// The size of a checked file.
    pub const fn size(&self) -> Option<u64> {
        match self.state {
            AttachmentState::Ready { size, .. } => Some(size),
            AttachmentState::Checking | AttachmentState::Failed { .. } => None,
        }
    }
}

/// The file name of `path`: its last component (`/` or `\` separated).
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Whether `path` is absolute on Unix (`/…`) or Windows (`C:\…`, `C:/…`, `\\server\…`).
pub fn is_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with("\\\\")
        || matches!(bytes, [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic())
}

/// The original message of a reply or forward. The runtime renders `header`, then the
/// original's body text (quoted with `> ` for replies) below the draft's own text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Quote {
    /// The quoted message.
    pub message: MessageId,
    /// Reply (quoted with `> `) or forward (inline, with a header block).
    pub style: QuoteStyle,
    /// The line(s) above the quoted text, e.g. `On 2023-11-14 22:13 UTC, Bob <bob@x> wrote:`.
    /// Built from the original's headers, so it is email content.
    pub header: String,
}

/// How a [`Quote`] is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStyle {
    /// Every line prefixed with `> `.
    Reply,
    /// Inline, below a "Forwarded message" header block.
    Forward,
}

impl Draft {
    /// Whether a send is scheduled or running.
    pub const fn is_outgoing(&self) -> bool {
        self.send_at.is_some() || self.sending
    }

    /// Whether the server copy is missing or older than the draft.
    pub fn needs_save(&self) -> bool {
        self.saved_ms != Some(self.updated_ms)
    }

    /// The draft as it is rendered at `date_ms`, sent by `from`. `quote_blob` is the cached raw
    /// original (if any). Only attachments that were checked successfully are included.
    pub fn outgoing(
        &self,
        from: Addr,
        quote_blob: Option<String>,
        date_ms: i64,
    ) -> OutgoingMessage {
        OutgoingMessage {
            from,
            to: self.to.clone(),
            cc: self.cc.clone(),
            bcc: self.bcc.clone(),
            subject: self.subject.clone(),
            body_text: self.body_text.clone(),
            message_id: self.message_id_hdr.clone(),
            in_reply_to: self.in_reply_to.clone(),
            references: self.references.clone(),
            date_ms,
            quote: self.quote.as_ref().map(|quote| QuoteSource {
                style: quote.style,
                header: quote.header.clone(),
                blob: quote_blob,
            }),
            attachments: self
                .attachments
                .iter()
                .filter(|a| matches!(a.state, AttachmentState::Ready { .. }))
                .cloned()
                .collect(),
        }
    }
}

/// A draft resolved for rendering (`app_io::compose::render`): everything the runtime needs,
/// taken from the draft, its account and the quoted message at the moment of saving or sending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutgoingMessage {
    /// The sender (the account's display name and address).
    pub from: Addr,
    /// `To` recipients.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub to: Vec<Addr>,
    /// `Cc` recipients.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<Addr>,
    /// `Bcc` recipients.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<Addr>,
    /// Subject.
    pub subject: String,
    /// The text written by the user or agent.
    pub body_text: String,
    /// `Message-ID`, without angle brackets.
    pub message_id: String,
    /// `In-Reply-To`, without angle brackets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    /// `References`, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    /// `Date`, in Unix milliseconds.
    pub date_ms: i64,
    /// The quoted or forwarded original, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<QuoteSource>,
    /// Files to attach (only `ready` ones), in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AttachmentRef>,
}

/// Where the runtime finds the text of a [`Quote`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuoteSource {
    /// Reply or forward.
    pub style: QuoteStyle,
    /// The attribution line or forward header block.
    pub header: String,
    /// Blob of the original's raw message, if its body is cached. Without it a reply carries no
    /// quote and a forward only its header block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// Which recipients a reply goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyTo {
    /// The sender only.
    Sender,
    /// The sender and every other recipient.
    All,
}

/// `To` and `Cc` of a reply to `original` sent from `me`. A reply to one's own message goes to
/// its recipients. `me` is never a recipient; duplicates (case-insensitive) are dropped.
pub fn reply_recipients(original: &Envelope, me: &str, mode: ReplyTo) -> (Vec<Addr>, Vec<Addr>) {
    let from_me = original
        .from
        .iter()
        .any(|addr| addr.email.eq_ignore_ascii_case(me));
    let primary: Vec<&Addr> = if from_me {
        original.to.iter().collect()
    } else {
        original.from.iter().collect()
    };
    let mut to: Vec<Addr> = Vec::new();
    let mut cc: Vec<Addr> = Vec::new();
    let known = |list: &[Addr], addr: &Addr| {
        list.iter()
            .any(|known| known.email.eq_ignore_ascii_case(&addr.email))
    };
    let add = |list: &mut Vec<Addr>, other: &[Addr], addr: &Addr| {
        if !addr.email.eq_ignore_ascii_case(me) && !known(list, addr) && !known(other, addr) {
            list.push(addr.clone());
        }
    };
    for addr in primary {
        add(&mut to, &cc, addr);
    }
    if mode == ReplyTo::All {
        if !from_me {
            for addr in &original.to {
                add(&mut to, &cc, addr);
            }
        }
        for addr in &original.cc {
            add(&mut cc, &to, addr);
        }
    }
    (to, cc)
}

/// `subject` with `prefix` (`Re:` or `Fwd:`) in front, unless it already starts with it (or,
/// for `Fwd:`, with `Fw:`), case-insensitively.
pub fn prefixed_subject(prefix: &str, subject: &str) -> String {
    let subject = subject.trim();
    let lower = subject.to_ascii_lowercase();
    let already = match prefix {
        "Fwd:" => lower.starts_with("fwd:") || lower.starts_with("fw:"),
        _ => lower.starts_with(&prefix.to_ascii_lowercase()),
    };
    if already {
        subject.to_owned()
    } else if subject.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix} {subject}")
    }
}

/// Whether `id` can be a `Message-ID`, `In-Reply-To` or `References` entry of a draft: not empty,
/// no whitespace or control characters. A line break would end the header, or the IMAP command
/// line that looks for the saved copy. IDs parsed from mail are cleaned like this already
/// (`app_io::mime`).
pub fn is_message_id(id: &str) -> bool {
    !id.is_empty() && !id.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// `References` of a reply to (or forward of) `original`: its references plus its own ID, leaving
/// out any that is not [`is_message_id`].
pub fn reply_references(original: &Envelope) -> Vec<String> {
    let mut references: Vec<String> = original
        .references
        .iter()
        .filter(|id| is_message_id(id))
        .cloned()
        .collect();
    if let Some(id) = &original.message_id_hdr
        && is_message_id(id)
        && !references.contains(id)
    {
        references.push(id.clone());
    }
    references
}

/// The attribution line of a reply: `On <date>, <sender> wrote:`.
pub fn reply_header(original: &Envelope) -> String {
    let sender = original
        .from
        .first()
        .map_or_else(|| "unknown sender".to_owned(), format_addr);
    format!("On {}, {sender} wrote:", format_date(original.date_ms))
}

/// The header block of a forward.
pub fn forward_header(original: &Envelope) -> String {
    let mut lines = vec![
        "---------- Forwarded message ----------".to_owned(),
        format!("From: {}", format_addrs(&original.from)),
        format!("Date: {}", format_date(original.date_ms)),
        format!("Subject: {}", original.subject),
        format!("To: {}", format_addrs(&original.to)),
    ];
    if !original.cc.is_empty() {
        lines.push(format!("Cc: {}", format_addrs(&original.cc)));
    }
    lines.join("\n")
}

/// `Name <email>`, or just `email`.
pub fn format_addr(addr: &Addr) -> String {
    match &addr.name {
        Some(name) if !name.is_empty() => format!("{name} <{}>", addr.email),
        _ => addr.email.clone(),
    }
}

fn format_addrs(addrs: &[Addr]) -> String {
    addrs.iter().map(format_addr).collect::<Vec<_>>().join(", ")
}

/// `YYYY-MM-DD HH:MM UTC` for Unix milliseconds (`unknown date` for 0).
pub fn format_date(ms: i64) -> String {
    if ms == 0 {
        return "unknown date".to_owned();
    }
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_formatted_in_utc() {
        assert_eq!(format_date(1_700_000_000_000), "2023-11-14 22:13 UTC");
        assert_eq!(format_date(951_782_400_000), "2000-02-29 00:00 UTC");
        assert_eq!(format_date(0), "unknown date");
    }

    #[test]
    fn paths_are_checked_for_both_platforms() {
        assert!(is_absolute_path("/home/me/a.pdf"));
        assert!(is_absolute_path("C:\\Users\\me\\a.pdf"));
        assert!(is_absolute_path("d:/a.pdf"));
        assert!(is_absolute_path("\\\\server\\share\\a.pdf"));
        assert!(!is_absolute_path("a.pdf"));
        assert!(!is_absolute_path("./a.pdf"));
        assert_eq!(file_name("/home/me/a.pdf"), "a.pdf");
        assert_eq!(file_name("C:\\Users\\me\\b.txt"), "b.txt");
    }

    #[test]
    fn outgoing_takes_only_checked_attachments() {
        let attachment = |path: &str, state| AttachmentRef {
            path: path.to_owned(),
            name: file_name(path).to_owned(),
            state,
        };
        let draft = Draft {
            id: DraftId::from_uuid(uuid::Uuid::from_u128(1)),
            account: AccountId::from_uuid(uuid::Uuid::from_u128(2)),
            reply_to: None,
            message_id_hdr: "x@example.com".to_owned(),
            in_reply_to: Some("p@example.org".to_owned()),
            references: vec!["p@example.org".to_owned()],
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Re: Hi".to_owned(),
            body_text: "Yes".to_owned(),
            quote: Some(Quote {
                message: MessageId::from_uuid(uuid::Uuid::from_u128(3)),
                style: QuoteStyle::Reply,
                header: "On …, Bob wrote:".to_owned(),
            }),
            attachments: vec![
                attachment(
                    "/a",
                    AttachmentState::Ready {
                        size: 1,
                        content_type: "text/plain".to_owned(),
                    },
                ),
                attachment("/b", AttachmentState::Checking),
                attachment(
                    "/c",
                    AttachmentState::Failed {
                        error: "gone".to_owned(),
                    },
                ),
            ],
            created_ms: 0,
            updated_ms: 0,
            saved_ms: None,
            save_error: None,
            send_at: None,
            sending: false,
            send_error: None,
            from: None,
        };
        let me = Addr {
            name: None,
            email: "me@example.com".to_owned(),
        };
        let out = draft.outgoing(me.clone(), Some("blob".to_owned()), 42);
        assert_eq!(out.from, me);
        assert_eq!(out.message_id, "x@example.com");
        assert_eq!(out.date_ms, 42);
        assert_eq!(out.quote.and_then(|q| q.blob).as_deref(), Some("blob"));
        let paths: Vec<_> = out.attachments.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(paths, ["/a"]);
    }

    #[test]
    fn subjects_get_one_prefix() {
        assert_eq!(prefixed_subject("Re:", "Lunch"), "Re: Lunch");
        assert_eq!(prefixed_subject("Re:", "RE: Lunch"), "RE: Lunch");
        assert_eq!(prefixed_subject("Fwd:", "Fw: Lunch"), "Fw: Lunch");
        assert_eq!(prefixed_subject("Fwd:", "Re: Lunch"), "Fwd: Re: Lunch");
        assert_eq!(prefixed_subject("Re:", ""), "Re:");
    }
}
