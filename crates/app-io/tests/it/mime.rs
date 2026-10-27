//! MIME corpus: every `tests/corpus/*.eml` is parsed and snapshot-tested (headers, body text,
//! attachments). Covers plain/HTML/alternative bodies, nested multiparts, attachments,
//! quoted-printable Windows-1252, ISO-2022-JP, Shift_JIS and malformed input.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use app_io::mime::{TextSource, parse_message, parse_message_with_document};

/// The crate's directory. Cargo sets `CARGO_MANIFEST_DIR` when it runs a test, and `cargo xtask
/// android test` sets it to where it pushed the test data on the emulator; the build-time value is
/// the fallback.
fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

#[test]
fn eml_corpus_matches_snapshots() {
    let dir = manifest_dir().join("tests/corpus");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "eml"))
        .collect();
    paths.sort();
    assert!(paths.len() >= 8, "corpus missing in {}", dir.display());
    for path in paths {
        let raw = std::fs::read(&path).unwrap();
        let parsed = parse_message(&raw);
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        insta::assert_json_snapshot!(name.clone(), parsed);
        // The reader's document of HTML bodies; the text is the same either way.
        let (with_document, document) = parse_message_with_document(&raw);
        assert_eq!(with_document, parsed);
        assert_eq!(
            document.is_some(),
            parsed.text_source == TextSource::Html,
            "{name}"
        );
        if let Some(document) = document {
            insta::assert_json_snapshot!(format!("{name}_document"), document);
        }
    }
}

#[test]
fn html_is_rendered_without_active_content() {
    let raw = std::fs::read(manifest_dir().join("tests/corpus/03_html_only.eml")).unwrap();
    let parsed = parse_message(&raw);
    assert_eq!(parsed.text_source, TextSource::Html);
    assert!(!parsed.text.contains("alert"), "{}", parsed.text);
    assert!(!parsed.text.contains("color: red"), "{}", parsed.text);
    assert!(
        parsed.text.contains("https://evil.example/track?id=1"),
        "link target is shown"
    );
    assert!(!parsed.text.contains("<p>"));
}

#[test]
fn charsets_are_decoded() {
    let read = |name: &str| {
        parse_message(&std::fs::read(manifest_dir().join("tests/corpus").join(name)).unwrap())
    };
    let latin = read("05_latin1_qp.eml");
    assert_eq!(latin.headers.subject, "Grüße – Preis 50 €");
    assert_eq!(latin.headers.from[0].name.as_deref(), Some("Jürgen Müller"));
    assert!(latin.text.contains("Grüße aus Köln – ça va? 50 €"));
    let jp = read("06_iso2022jp.eml");
    assert_eq!(jp.headers.subject, "会議の件");
    assert!(jp.text.contains("明日の会議"));
    let sjis = read("07_shift_jis_html.eml");
    assert_eq!(sjis.headers.subject, "お知らせ");
    assert!(sjis.text.contains("千円"), "{}", sjis.text);
}

#[test]
fn garbage_never_panics() {
    for raw in [
        &b""[..],
        b"\xff\xfe\x00",
        b"Content-Type: multipart/mixed\r\n\r\n--",
    ] {
        let _ = parse_message(raw);
    }
}

#[test]
fn the_pinned_keyword_is_a_flag() {
    let flags = app_io::mime::parse_flags(["\\Seen", "$pinned", "$Junk"]);
    assert!(flags.seen && flags.pinned && !flags.flagged);
}
