//! The message that carries the settings document in the `dail/Settings` folder (ADR 0015): a
//! single `application/json` part. Reading takes the first `application/json` part of any
//! message, whoever wrote it; the bytes stay untrusted until `SyncedSettings::from_json` checks
//! them.

use app_core::SETTINGS_MAX_BYTES;
use mail_builder::MessageBuilder;
use mail_builder::headers::raw::Raw;
use mail_builder::mime::MimePart;
use mail_parser::{MessageParser, MimeHeaders};

use crate::store::IoError;

/// The largest raw message read from the settings folder: the document's limit in base64, plus
/// room for the headers. A larger message is refused before it is downloaded.
pub const MAX_RAW_BYTES: usize = SETTINGS_MAX_BYTES / 3 * 4 + 16 * 1024;

/// The subject of the messages dail writes.
pub const SUBJECT: &str = "dail settings";

/// Renders `document` (the JSON) as a message with one `application/json` part. `date_ms` is
/// the `Date` header, in Unix milliseconds.
pub fn render(document: &str, date_ms: i64) -> Result<Vec<u8>, IoError> {
    MessageBuilder::new()
        .from(("dail", "dail@localhost"))
        .subject(SUBJECT)
        .date(date_ms.div_euclid(1000))
        .header("User-Agent", Raw::new("dail"))
        .body(MimePart::new(
            "application/json; charset=utf-8",
            document.as_bytes(),
        ))
        .write_to_vec()
        .map_err(|err| IoError::Unavailable(format!("cannot render the settings message: {err}")))
}

/// The decoded bytes of the first `application/json` part of `raw`, if any.
pub fn document(raw: &[u8]) -> Option<Vec<u8>> {
    let message = MessageParser::default().parse(raw)?;
    message
        .parts
        .iter()
        .find(|part| {
            part.content_type().is_some_and(|ct| {
                ct.ctype().eq_ignore_ascii_case("application")
                    && ct
                        .subtype()
                        .is_some_and(|sub| sub.eq_ignore_ascii_case("json"))
            })
        })
        .map(|part| part.contents().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rendered_document_reads_back() {
        let json = "{\"schema\":1,\"prefs\":{\"x\":{\"updated_ms\":1,\"value\":\"é\"}}}";
        let raw = render(json, 1_700_000_000_000).unwrap();
        assert!(String::from_utf8_lossy(&raw).contains("application/json"));
        assert_eq!(document(&raw).unwrap(), json.as_bytes());
    }

    #[test]
    fn a_message_without_json_has_no_document() {
        assert_eq!(document(b"Subject: hi\r\n\r\nhello\r\n"), None);
    }
}
