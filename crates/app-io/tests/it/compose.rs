//! `compose::render`: drafts as RFC 5322, checked by parsing the output back.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{Addr, OutgoingMessage, QuoteSource, QuoteStyle};
use app_io::compose::{Bcc, FileData, body_text, render};
use app_io::mime::{TextSource, parse_message};

fn addr(name: Option<&str>, email: &str) -> Addr {
    Addr {
        name: name.map(Into::into),
        email: email.into(),
    }
}

fn message() -> OutgoingMessage {
    OutgoingMessage {
        from: addr(Some("Ålice Example"), "alice@example.com"),
        to: vec![addr(Some("Bob"), "bob@example.org")],
        cc: vec![addr(None, "carol@example.org")],
        bcc: vec![addr(None, "boss@example.org")],
        subject: "Re: Q3 numbers — final".into(),
        body_text: "Thanks, Bob!\nSee the attached file.\n".into(),
        message_id: "0123abcd@example.com".into(),
        in_reply_to: Some("q3@example.org".into()),
        references: vec!["root@example.org".into(), "q3@example.org".into()],
        date_ms: 1_700_000_000_000,
        quote: None,
        attachments: Vec::new(),
    }
}

fn reply_quote() -> Option<QuoteSource> {
    Some(QuoteSource {
        style: QuoteStyle::Reply,
        header: "On 2023-11-13 22:13 UTC, Bob <bob@example.org> wrote:".into(),
        blob: None,
    })
}

#[test]
fn headers_round_trip() {
    let raw = render(&message(), None, &[], Bcc::Keep).unwrap();
    let parsed = parse_message(&raw);
    let headers = &parsed.headers;
    assert_eq!(headers.message_id.as_deref(), Some("0123abcd@example.com"));
    assert_eq!(headers.in_reply_to.as_deref(), Some("q3@example.org"));
    assert_eq!(headers.references, ["root@example.org", "q3@example.org"]);
    assert_eq!(
        headers.from,
        [addr(Some("Ålice Example"), "alice@example.com")]
    );
    assert_eq!(headers.to, [addr(Some("Bob"), "bob@example.org")]);
    assert_eq!(headers.cc, [addr(None, "carol@example.org")]);
    assert_eq!(headers.subject, "Re: Q3 numbers — final");
    assert_eq!(headers.date_ms, 1_700_000_000_000);
    assert_eq!(parsed.text_source, TextSource::Plain);
    assert_eq!(
        parsed.text.trim_end(),
        "Thanks, Bob!\nSee the attached file."
    );
    assert!(parsed.attachments.is_empty());
    let text = String::from_utf8_lossy(&raw);
    assert!(text.contains("\r\nBcc: <boss@example.org>\r\n"), "{text}");
    assert!(text.contains("\r\nUser-Agent: dail\r\n"));
}

#[test]
fn bcc_is_hidden_from_the_smtp_copy() {
    let raw = render(&message(), None, &[], Bcc::Hide).unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(!text.contains("boss@example.org"), "{text}");
}

#[test]
fn attachments_make_a_multipart_message() {
    let files = [
        FileData {
            name: "Q3 report.pdf".into(),
            content_type: "application/pdf".into(),
            bytes: b"%PDF-1.4 numbers".to_vec(),
        },
        FileData {
            name: "notes.txt".into(),
            content_type: "text/plain".into(),
            bytes: b"plain notes".to_vec(),
        },
    ];
    let raw = render(&message(), None, &files, Bcc::Hide).unwrap();
    let parsed = parse_message(&raw);
    assert_eq!(
        parsed.text.trim_end(),
        "Thanks, Bob!\nSee the attached file."
    );
    let names: Vec<_> = parsed
        .attachments
        .iter()
        .map(|a| (a.filename.clone(), a.content_type.clone(), a.size))
        .collect();
    assert_eq!(
        names,
        [
            (
                Some("Q3 report.pdf".to_owned()),
                "application/pdf".to_owned(),
                16
            ),
            (Some("notes.txt".to_owned()), "text/plain".to_owned(), 11),
        ]
    );
    let summary = app_io::mime::summarize_headers(&raw);
    assert!(summary.has_attachments, "multipart/mixed");
}

#[test]
fn a_reply_quotes_the_original_below_its_attribution() {
    let reply = OutgoingMessage {
        quote: reply_quote(),
        ..message()
    };
    let original = "Hi,\n\nthe Q3 numbers are attached.\n-- \nBob\n";
    assert_eq!(
        body_text(&reply, Some(original)),
        "Thanks, Bob!\nSee the attached file.\n\n\
         On 2023-11-13 22:13 UTC, Bob <bob@example.org> wrote:\n\
         > Hi,\n>\n> the Q3 numbers are attached.\n> -- \n> Bob"
    );
    // Without the original's text the reply has no quote at all.
    assert_eq!(
        body_text(&reply, None),
        "Thanks, Bob!\nSee the attached file."
    );
    let raw = render(&reply, Some(original), &[], Bcc::Hide).unwrap();
    assert!(
        parse_message(&raw)
            .text
            .contains("> the Q3 numbers are attached.")
    );
}

#[test]
fn a_forward_inlines_the_original_below_its_header_block() {
    let forward = OutgoingMessage {
        body_text: String::new(),
        quote: Some(QuoteSource {
            style: QuoteStyle::Forward,
            header: "---------- Forwarded message ----------\nFrom: Bob <bob@example.org>".into(),
            blob: None,
        }),
        ..message()
    };
    assert_eq!(
        body_text(&forward, Some("Numbers inside.\n")),
        "---------- Forwarded message ----------\nFrom: Bob <bob@example.org>\n\nNumbers inside."
    );
    assert_eq!(
        body_text(&forward, None),
        "---------- Forwarded message ----------\nFrom: Bob <bob@example.org>"
    );
}

#[test]
fn a_draft_without_recipients_still_renders() {
    let empty = OutgoingMessage {
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        in_reply_to: None,
        references: Vec::new(),
        subject: String::new(),
        body_text: String::new(),
        ..message()
    };
    let raw = render(&empty, None, &[], Bcc::Keep).unwrap();
    let parsed = parse_message(&raw);
    assert!(parsed.headers.to.is_empty());
    assert_eq!(
        parsed.headers.message_id.as_deref(),
        Some("0123abcd@example.com")
    );
}
