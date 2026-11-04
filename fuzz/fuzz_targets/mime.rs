//! Everything sync and the reader do with a raw message: headers, snippet, attachments, the
//! parsed body with its document, inline parts and attachment files.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|raw: &[u8]| {
    let _ = app_io::mime::summarize_headers(raw);
    let _ = app_io::mime::snippet(raw);
    let _ = app_io::mime::has_attachments(raw);
    let _ = app_io::mime::parse_message_with_document(raw);
    let _ = app_io::mime::inline_part(raw, "image001");
    for part in 0..4 {
        let _ = app_io::mime::attachment_part(raw, part);
    }
    let _ = app_io::mime::attachment_files(raw, &[0, 1, 2]);
});
