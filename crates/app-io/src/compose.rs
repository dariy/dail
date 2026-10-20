//! Rendering drafts as RFC 5322 messages with `mail-builder` (`ARCHITECTURE.md §6`).
//!
//! The engine resolves a draft into an [`OutgoingMessage`]; the runtime reads the quoted
//! original and the attached files and calls [`render`]. Output is plain text (UTF-8, encoded
//! as needed by `mail-builder`), `multipart/mixed` when there are attachments.

use app_core::{Addr, OutgoingMessage, QuoteStyle};
use mail_builder::MessageBuilder;
use mail_builder::headers::address::Address;
use mail_builder::headers::message_id::MessageId;

use crate::store::IoError;

/// The content of one attached file, read by the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileData {
    /// File name shown to recipients.
    pub name: String,
    /// MIME type.
    pub content_type: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

/// Whether the rendered message carries a `Bcc` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bcc {
    /// Keep it: drafts and the copy in Sent show who was in Bcc.
    Keep,
    /// Leave it out: the copy handed to the SMTP server (Bcc recipients are only in the
    /// envelope).
    Hide,
}

/// The message text: the draft's own text, then the quote. A reply quotes every line of the
/// original with `> ` under its attribution line; a forward puts the original below its header
/// block. Without the original's text a reply has no quote and a forward only its header block.
pub fn body_text(message: &OutgoingMessage, original: Option<&str>) -> String {
    let mut text = message.body_text.trim_end().to_owned();
    let Some(quote) = &message.quote else {
        return text;
    };
    let section = match (quote.style, original) {
        (QuoteStyle::Reply, None) => return text,
        (QuoteStyle::Reply, Some(original)) => {
            let mut lines = vec![quote.header.clone()];
            lines.extend(original.trim_end().lines().map(|line| {
                if line.is_empty() {
                    ">".to_owned()
                } else {
                    format!("> {line}")
                }
            }));
            lines.join("\n")
        }
        (QuoteStyle::Forward, None) => quote.header.clone(),
        (QuoteStyle::Forward, Some(original)) => {
            format!("{}\n\n{}", quote.header, original.trim_end())
        }
    };
    if !text.is_empty() {
        text.push_str("\n\n");
    }
    text.push_str(&section);
    text
}

/// Renders `message` as an RFC 5322 message. `original` is the quoted message's text (see
/// [`body_text`]); `files` are the attachments' contents, in order.
pub fn render(
    message: &OutgoingMessage,
    original: Option<&str>,
    files: &[FileData],
    bcc: Bcc,
) -> Result<Vec<u8>, IoError> {
    let mut builder = MessageBuilder::new()
        .from(address(&message.from))
        .subject(message.subject.as_str())
        .message_id(message.message_id.as_str())
        .date(message.date_ms.div_euclid(1000))
        .header("User-Agent", mail_builder::headers::raw::Raw::new("dail"))
        .text_body(body_text(message, original));
    if !message.to.is_empty() {
        builder = builder.to(list(&message.to));
    }
    if !message.cc.is_empty() {
        builder = builder.cc(list(&message.cc));
    }
    if bcc == Bcc::Keep && !message.bcc.is_empty() {
        builder = builder.bcc(list(&message.bcc));
    }
    if let Some(parent) = &message.in_reply_to {
        builder = builder.in_reply_to(parent.as_str());
    }
    if !message.references.is_empty() {
        builder = builder.references(MessageId::new_list(
            message.references.iter().map(String::as_str),
        ));
    }
    for file in files {
        builder = builder.attachment(
            file.content_type.as_str(),
            file.name.as_str(),
            file.bytes.as_slice(),
        );
    }
    builder
        .write_to_vec()
        .map_err(|err| IoError::Unavailable(format!("cannot render message: {err}")))
}

fn address(addr: &Addr) -> Address<'_> {
    Address::new_address(addr.name.as_deref(), addr.email.as_str())
}

fn list(addrs: &[Addr]) -> Address<'_> {
    Address::new_list(addrs.iter().map(address).collect())
}
