//! MIME parsing (`ARCHITECTURE.md §6`): header summaries for envelopes and readable bodies
//! (text/plain, or text/html rendered to text), attachment lists and charset decoding. Header
//! dates are parsed here and handed to the engine as values.
//!
//! Message content is untrusted: HTML is converted to plain text (scripts, styles and remote
//! images are dropped; link targets are listed as footnotes), nothing is fetched.

use app_core::document::Document;
use app_core::{Addr, Flags, RemoteEnvelope};
use mail_parser::{Address, HeaderValue, Message, MessageParser, MessagePart, MimeHeaders};
use serde::Serialize;

use crate::document::html_document;

/// Line width of HTML rendered as text.
pub const TEXT_WIDTH: usize = 100;

/// What sync needs from a message's header block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HeaderSummary {
    /// `Message-ID`, without angle brackets.
    pub message_id: Option<String>,
    /// First `In-Reply-To` ID.
    pub in_reply_to: Option<String>,
    /// `References`, oldest first.
    pub references: Vec<String>,
    /// `From`.
    pub from: Vec<Addr>,
    /// `To`.
    pub to: Vec<Addr>,
    /// `Cc`.
    pub cc: Vec<Addr>,
    /// Decoded subject (RFC 2047), empty if missing.
    pub subject: String,
    /// `Date` in Unix milliseconds, 0 if missing or unparsable.
    pub date_ms: i64,
    /// `multipart/mixed` or a single non-text part: probably has attachments.
    pub has_attachments: bool,
}

/// Parses a header block (or a whole message; the body is ignored).
pub fn summarize_headers(raw: &[u8]) -> HeaderSummary {
    let Some(message) = MessageParser::default().parse_headers(raw) else {
        return HeaderSummary::default();
    };
    let has_attachments = message.content_type().is_some_and(|ct| {
        let ctype = ct.ctype().to_ascii_lowercase();
        let subtype = ct.subtype().unwrap_or_default().to_ascii_lowercase();
        match ctype.as_str() {
            "multipart" => subtype == "mixed",
            "text" | "message" => false,
            _ => true,
        }
    });
    HeaderSummary {
        message_id: message.message_id().and_then(clean_id),
        in_reply_to: ids(message.in_reply_to()).into_iter().next(),
        references: ids(message.references()),
        from: addresses(message.from()),
        to: addresses(message.to()),
        cc: addresses(message.cc()),
        subject: message.subject().unwrap_or_default().trim().to_owned(),
        date_ms: message
            .date()
            .map_or(0, |date| date.to_timestamp().saturating_mul(1000)),
        has_attachments,
    }
}

/// The envelope of a fetched message from its UID, IMAP flags, size and header block.
pub fn remote_envelope<'a>(
    uid: u32,
    flags: impl IntoIterator<Item = &'a str>,
    size: u32,
    header: &[u8],
) -> RemoteEnvelope {
    let summary = summarize_headers(header);
    RemoteEnvelope {
        uid,
        message_id_hdr: summary.message_id,
        in_reply_to: summary.in_reply_to,
        references: summary.references,
        from: summary.from,
        to: summary.to,
        cc: summary.cc,
        subject: summary.subject,
        date_ms: summary.date_ms,
        flags: parse_flags(flags),
        size,
        has_attachments: summary.has_attachments,
    }
}

/// A readable rendering of a whole message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedMessage {
    /// Header fields.
    pub headers: HeaderSummary,
    /// Body text: the text/plain parts, or the text/html parts rendered as text.
    pub text: String,
    /// Where `text` came from.
    pub text_source: TextSource,
    /// Attachments and inline files, in MIME order.
    pub attachments: Vec<AttachmentInfo>,
}

/// The origin of [`ParsedMessage::text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSource {
    /// text/plain parts.
    Plain,
    /// text/html parts, converted with `html2text`.
    Html,
    /// The message has no text part.
    None,
}

/// One attached file (or nested message).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttachmentInfo {
    /// MIME part number within the message (stable handle for saving it later).
    pub part: usize,
    /// File name from `Content-Disposition` / `Content-Type`, if any. Untrusted.
    pub filename: Option<String>,
    /// `type/subtype`, lower case.
    pub content_type: String,
    /// Decoded size in bytes.
    pub size: usize,
    /// `Content-Disposition: inline` (e.g. an image referenced from the HTML).
    pub inline: bool,
    /// `Content-ID` without angle brackets, if any.
    pub content_id: Option<String>,
}

/// The list-row preview of a raw message ([`app_core::snippet`] of its readable text).
pub fn snippet(raw: &[u8]) -> Option<String> {
    app_core::snippet(&parse_message(raw).text)
}

/// Parses a whole raw message into headers, readable text and attachments. Never fails: an
/// unparsable message yields empty fields.
pub fn parse_message(raw: &[u8]) -> ParsedMessage {
    parse(raw, false).0
}

/// [`parse_message`], plus the HTML body as a [`Document`] for display when the text comes
/// from HTML (`text_source: html`).
pub fn parse_message_with_document(raw: &[u8]) -> (ParsedMessage, Option<Document>) {
    parse(raw, true)
}

fn parse(raw: &[u8], with_document: bool) -> (ParsedMessage, Option<Document>) {
    let Some(message) = MessageParser::default().parse(raw) else {
        let empty = ParsedMessage {
            headers: HeaderSummary::default(),
            text: String::new(),
            text_source: TextSource::None,
            attachments: Vec::new(),
        };
        return (empty, None);
    };
    let headers = summarize_headers(raw);
    let (text, text_source, document) = match body_part(&message) {
        BodyPart::Plain(text) => (text, TextSource::Plain, None),
        BodyPart::Html(html) => (
            html_to_text(html).trim_end().to_owned(),
            TextSource::Html,
            with_document.then(|| html_document(html)),
        ),
        BodyPart::None => (String::new(), TextSource::None, None),
    };
    let attachments = message
        .attachments
        .iter()
        .filter_map(|&part| {
            let id = usize::try_from(part).ok()?;
            message.parts.get(id).map(|p| attachment_info(id, p))
        })
        .collect();
    let parsed = ParsedMessage {
        headers,
        text,
        text_source,
        attachments,
    };
    (parsed, document)
}

/// The part a message's readable text comes from.
enum BodyPart<'a> {
    /// The text/plain parts, joined.
    Plain(String),
    /// The HTML of the first text/html part.
    Html(&'a str),
    None,
}

/// The text/plain parts joined, or else the first text/html part.
fn body_part<'a>(message: &'a Message<'a>) -> BodyPart<'a> {
    let parts = |ids: &[u32]| -> Vec<&'a MessagePart<'a>> {
        ids.iter()
            .filter_map(|&id| message.parts.get(usize::try_from(id).ok()?))
            .collect()
    };
    let plain: Vec<String> = parts(&message.text_body)
        .into_iter()
        .filter(|part| part.is_text() && !part.is_text_html())
        .filter_map(|part| part.text_contents().map(normalize_newlines))
        .collect();
    if !plain.is_empty() {
        return BodyPart::Plain(plain.join("\n").trim_end().to_owned());
    }
    let html = parts(&message.html_body)
        .into_iter()
        .chain(parts(&message.text_body))
        .filter(|part| part.is_text_html())
        .find_map(|part| part.text_contents());
    if let Some(html) = html {
        return BodyPart::Html(html);
    }
    // Broken structure (e.g. an unterminated multipart) can leave the only text part filed as
    // an attachment; show it unless it is explicitly an attachment.
    let stray = parts(&message.attachments).into_iter().find(|part| {
        part.is_text()
            && !part
                .content_disposition()
                .is_some_and(|d| d.is_attachment())
    });
    match stray {
        Some(part) if part.is_text_html() => {
            BodyPart::Html(part.text_contents().unwrap_or_default())
        }
        Some(part) => BodyPart::Plain(
            normalize_newlines(part.text_contents().unwrap_or_default())
                .trim_end()
                .to_owned(),
        ),
        None => BodyPart::None,
    }
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Renders HTML as plain text (links become numbered footnotes with their real targets).
pub fn html_to_text(html: &str) -> String {
    html2text::config::plain()
        .link_footnotes(true)
        .string_from_read(html.as_bytes(), TEXT_WIDTH)
        .unwrap_or_default()
}

fn attachment_info(part: usize, mime: &MessagePart<'_>) -> AttachmentInfo {
    let content_type = mime.content_type().map_or_else(
        || "application/octet-stream".to_owned(),
        |ct| match ct.subtype() {
            Some(sub) => format!("{}/{}", ct.ctype(), sub).to_ascii_lowercase(),
            None => ct.ctype().to_ascii_lowercase(),
        },
    );
    AttachmentInfo {
        part,
        filename: mime
            .attachment_name()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        content_type,
        size: mime.len(),
        inline: mime.content_disposition().is_some_and(|d| d.is_inline()),
        content_id: mime.content_id().and_then(clean_id),
    }
}

/// A part of a message found by its `Content-ID`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlinePart {
    /// `type/subtype`, lower case.
    pub content_type: String,
    /// The decoded bytes. Untrusted.
    pub bytes: Vec<u8>,
}

/// The part with `Content-ID: <cid>` (an inline image the HTML refers to as `cid:…`), if any.
pub fn inline_part(raw: &[u8], cid: &str) -> Option<InlinePart> {
    let message = MessageParser::default().parse(raw)?;
    let part = message
        .parts
        .iter()
        .find(|part| part.content_id().and_then(clean_id).as_deref() == Some(cid))?;
    let content_type = attachment_info(0, part).content_type;
    Some(InlinePart {
        content_type,
        bytes: part.contents().to_vec(),
    })
}

/// The attachments of a message to write to disk: `(safe file name, decoded bytes)` for each of
/// `parts` (MIME part numbers, as in [`AttachmentInfo::part`]), or for every attachment if
/// `parts` is empty. Names are made safe with [`safe_file_name`] and made unique within the
/// list. An error names a part that is not an attachment of the message.
pub fn attachment_files(raw: &[u8], parts: &[usize]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let message = MessageParser::default()
        .parse(raw)
        .ok_or_else(|| "the message cannot be parsed".to_owned())?;
    let attachments: Vec<usize> = message
        .attachments
        .iter()
        .filter_map(|&part| usize::try_from(part).ok())
        .collect();
    let wanted: Vec<usize> = if parts.is_empty() {
        attachments.clone()
    } else {
        parts.to_vec()
    };
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for part in wanted {
        let mime = attachments
            .contains(&part)
            .then(|| message.parts.get(part))
            .flatten()
            .ok_or_else(|| format!("part {part} is not an attachment of the message"))?;
        let name = safe_file_name(mime.attachment_name(), part);
        let mut unique = name.clone();
        let mut n = 1;
        while out
            .iter()
            .any(|(taken, _)| taken.eq_ignore_ascii_case(&unique))
        {
            unique = numbered_file_name(&name, n);
            n += 1;
        }
        out.push((unique, mime.contents().to_vec()));
    }
    Ok(out)
}

/// A file name that is safe to create in a folder: the last path component of `name`, without
/// control characters, bidirectional controls (a right-to-left override would make
/// `invoice\u{202e}fdp.exe` read as `invoiceexe.pdf`) and the characters Windows forbids, without leading dots (no hidden or
/// dot files) and trailing dots or spaces, not a reserved Windows device name, at most 200
/// bytes; `attachment-<part>` if nothing is left.
pub fn safe_file_name(name: Option<&str>, part: usize) -> String {
    let name = name.unwrap_or_default();
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .filter(|c| {
            !c.is_control()
                && !is_bidi_control(*c)
                && !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
        })
        .collect();
    let mut cleaned = cleaned
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .to_owned();
    if cleaned.len() > 200 {
        let (stem, extension) = split_extension(&cleaned);
        let keep = 200usize.saturating_sub(extension.len());
        let mut cut = keep.min(stem.len());
        while !stem.is_char_boundary(cut) {
            cut -= 1;
        }
        cleaned = format!("{}{extension}", &stem[..cut]);
    }
    let device = split_extension(&cleaned).0.to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((device.starts_with("COM") || device.starts_with("LPT"))
            && device.len() == 4
            && device.ends_with(|c: char| c.is_ascii_digit()));
    if reserved {
        cleaned.insert(0, '_');
    }
    if cleaned.is_empty() {
        format!("attachment-{part}")
    } else {
        cleaned
    }
}

/// The characters that reorder text (embeddings, overrides, isolates and marks).
const fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{061c}'
    )
}

/// `name (n).ext`: the `n`-th alternative to an existing `name.ext`.
pub fn numbered_file_name(name: &str, n: u32) -> String {
    let (stem, extension) = split_extension(name);
    format!("{stem} ({n}){extension}")
}

/// `("report", ".pdf")`; a name without a dot (or starting with one) has no extension.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 => (&name[..at], &name[at..]),
        _ => (name, ""),
    }
}

/// Attachment `part` of a message (a MIME part number from [`AttachmentInfo::part`]), with its
/// safe file name; `None` if the message has no such attachment.
pub fn attachment_part(raw: &[u8], part: usize) -> Option<(String, InlinePart)> {
    let message = MessageParser::default().parse(raw)?;
    let is_attachment = message
        .attachments
        .iter()
        .any(|&p| usize::try_from(p).ok() == Some(part));
    let mime = message.parts.get(part).filter(|_| is_attachment)?;
    let name = safe_file_name(mime.attachment_name(), part);
    let content_type = attachment_info(part, mime).content_type;
    Some((
        name,
        InlinePart {
            content_type,
            bytes: mime.contents().to_vec(),
        },
    ))
}

/// Whether a full raw message has attachments (parts that are not the message text).
pub fn has_attachments(raw: &[u8]) -> bool {
    MessageParser::default()
        .parse(raw)
        .is_some_and(|message| message.attachment_count() > 0)
}

/// IMAP system flags (`\Seen`, `\Flagged`, …), case-insensitive; others are ignored.
pub fn parse_flags<'a>(flags: impl IntoIterator<Item = &'a str>) -> Flags {
    let mut parsed = Flags::default();
    for flag in flags {
        match flag.to_ascii_lowercase().as_str() {
            "\\seen" => parsed.seen = true,
            "\\flagged" => parsed.flagged = true,
            "\\answered" => parsed.answered = true,
            "\\draft" => parsed.draft = true,
            _ => {}
        }
    }
    parsed
}

/// A message ID without angle brackets, whitespace and control characters. mail-parser keeps
/// everything between `<` and `>`, folds and stray CRs included, and IDs end up in the
/// `In-Reply-To`/`References` headers of replies; RFC 5322 IDs contain neither.
fn clean_id(id: &str) -> Option<String> {
    let id: String = id
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    (!id.is_empty()).then_some(id)
}

fn ids(value: &HeaderValue<'_>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in value.as_text_list().unwrap_or_default() {
        if let Some(id) = clean_id(id)
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

fn addresses(address: Option<&Address<'_>>) -> Vec<Addr> {
    address
        .map(|address| {
            address
                .iter()
                .filter_map(|addr| {
                    let email = addr.address()?.trim();
                    (!email.is_empty()).then(|| Addr {
                        name: addr
                            .name()
                            .map(str::trim)
                            .filter(|name| !name.is_empty())
                            .map(str::to_owned),
                        email: email.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The MIME type of a file, guessed from its name's extension (`application/octet-stream` if
/// unknown).
pub fn content_type_for(file_name: &str) -> &'static str {
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "ics" => "text/calendar",
        "json" => "application/json",
        "xml" => "application/xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" | "tgz" => "application/gzip",
        "tar" => "application/x-tar",
        "rtf" => "application/rtf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "eml" => "message/rfc822",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &[u8] = b"Message-ID: <b@example.org>\r\n\
In-Reply-To: <a@example.org>\r\n\
References: <root@example.org>\r\n <a@example.org>\r\n\
From: =?UTF-8?Q?Ren=C3=A9e?= <renee@example.org>\r\n\
To: alice@example.com, \"Bob B.\" <bob@example.com>\r\n\
Cc: undisclosed-recipients:;\r\n\
Subject: =?UTF-8?B?UmU6IFEzIG7Dum1lcm9z?=\r\n\
Date: Tue, 14 Nov 2023 09:41:00 +0100\r\n\
Content-Type: multipart/mixed; boundary=x\r\n\r\n";

    #[test]
    fn headers_are_decoded() {
        let summary = summarize_headers(HEADER);
        assert_eq!(summary.message_id.as_deref(), Some("b@example.org"));
        assert_eq!(summary.in_reply_to.as_deref(), Some("a@example.org"));
        assert_eq!(summary.references, ["root@example.org", "a@example.org"]);
        assert_eq!(summary.from[0].name.as_deref(), Some("Renée"));
        assert_eq!(summary.from[0].email, "renee@example.org");
        assert_eq!(summary.to.len(), 2);
        assert_eq!(summary.to[1].name.as_deref(), Some("Bob B."));
        assert!(summary.cc.is_empty());
        assert_eq!(summary.subject, "Re: Q3 números");
        assert_eq!(summary.date_ms, 1_699_951_260_000);
        assert!(summary.has_attachments);
    }

    #[test]
    fn message_ids_lose_folds_and_control_characters() {
        let summary = summarize_headers(
            b"Message-ID: <a@b\r\n Z1 DELETE INBOX>\r\n\
In-Reply-To: <p@q\rBcc: x@y>\r\n\
References: <r@s>\r\n\t<t\x01@u>\r\n\r\n",
        );
        assert_eq!(summary.message_id.as_deref(), Some("a@bZ1DELETEINBOX"));
        assert_eq!(summary.in_reply_to.as_deref(), Some("p@qBcc:x@y"));
        assert_eq!(summary.references, ["r@s", "t@u"]);
    }

    #[test]
    fn missing_headers_have_defaults() {
        let summary = summarize_headers(b"X-Nothing: here\r\n\r\n");
        assert_eq!(summary, HeaderSummary::default());
        let plain = summarize_headers(b"Content-Type: text/plain\r\n\r\n");
        assert!(!plain.has_attachments);
        let pdf = summarize_headers(b"Content-Type: application/pdf\r\n\r\n");
        assert!(pdf.has_attachments);
        let alt = summarize_headers(b"Content-Type: multipart/alternative; boundary=b\r\n\r\n");
        assert!(!alt.has_attachments);
    }

    #[test]
    fn inline_parts_are_found_by_content_id() {
        let raw = b"Content-Type: multipart/related; boundary=b\r\n\r\n--b\r\n\
Content-Type: text/html\r\n\r\n<img src=\"cid:logo@x\">\r\n--b\r\n\
Content-Type: image/PNG\r\nContent-ID: <logo@x>\r\nContent-Transfer-Encoding: base64\r\n\r\n\
iVBORw0K\r\n--b--\r\n";
        let part = inline_part(raw, "logo@x").unwrap();
        assert_eq!(part.content_type, "image/png");
        assert_eq!(part.bytes, b"\x89PNG\r\n");
        assert!(inline_part(raw, "other@x").is_none());
        assert!(inline_part(b"", "logo@x").is_none());
    }

    #[test]
    fn file_names_are_made_safe() {
        assert_eq!(safe_file_name(Some("report.pdf"), 2), "report.pdf");
        assert_eq!(safe_file_name(Some("../../.bashrc"), 2), "bashrc");
        assert_eq!(safe_file_name(Some("C:\\Windows\\evil.exe"), 2), "evil.exe");
        assert_eq!(safe_file_name(Some("a\u{0}b<c>:d|?.txt. "), 2), "abcd.txt");
        assert_eq!(safe_file_name(Some("CON.txt"), 2), "_CON.txt");
        assert_eq!(safe_file_name(Some("com1"), 2), "_com1");
        assert_eq!(safe_file_name(Some("..."), 7), "attachment-7");
        assert_eq!(
            safe_file_name(Some("invoice\u{202e}fdp.exe\u{2066}"), 2),
            "invoicefdp.exe"
        );
        assert_eq!(safe_file_name(None, 3), "attachment-3");
        let long = format!("{}.pdf", "é".repeat(150));
        let safe = safe_file_name(Some(&long), 1);
        assert!(safe.len() <= 200 && safe.ends_with(".pdf"), "{safe}");
        assert_eq!(numbered_file_name("report.pdf", 2), "report (2).pdf");
        assert_eq!(numbered_file_name("README", 1), "README (1)");
    }

    #[test]
    fn attachment_files_by_part() {
        let raw = b"Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\n\
Content-Type: text/plain\r\n\r\nHi\r\n--b\r\n\
Content-Type: text/plain; name=a.txt\r\nContent-Disposition: attachment; filename=a.txt\r\n\r\n\
one\r\n--b\r\n\
Content-Type: text/plain\r\nContent-Disposition: attachment; filename=A.TXT\r\n\r\ntwo\r\n--b--\r\n";
        let all = attachment_files(raw, &[]).unwrap();
        let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["a.txt", "A (1).TXT"]);
        assert_eq!(all[0].1, b"one");
        let parts: Vec<usize> = parse_message(raw)
            .attachments
            .iter()
            .map(|a| a.part)
            .collect();
        let one = attachment_files(raw, &parts[1..]).unwrap();
        assert_eq!(one[0].1, b"two");
        assert!(
            attachment_files(raw, &[0])
                .unwrap_err()
                .contains("not an attachment")
        );
        let (name, part) = attachment_part(raw, parts[0]).unwrap();
        assert_eq!(
            (name.as_str(), part.bytes.as_slice()),
            ("a.txt", b"one".as_slice())
        );
        assert_eq!(part.content_type, "text/plain");
        assert!(
            attachment_part(raw, 0).is_none(),
            "the body is not an attachment"
        );
        assert!(attachment_part(raw, 99).is_none());
    }

    #[test]
    fn flags_and_envelopes() {
        let envelope = remote_envelope(7, ["\\Seen", "\\FLAGGED", "$Junk"], 1234, HEADER);
        assert_eq!(envelope.uid, 7);
        assert_eq!(envelope.size, 1234);
        assert!(envelope.flags.seen && envelope.flags.flagged);
        assert!(!envelope.flags.answered && !envelope.flags.draft);
        assert_eq!(envelope.message_id_hdr.as_deref(), Some("b@example.org"));
    }
}
