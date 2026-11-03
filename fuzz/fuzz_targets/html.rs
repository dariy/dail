//! HTML bodies: the reader's document and the plain-text conversion.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|html: &str| {
    let _ = app_io::document::html_document(html);
    let _ = app_io::mime::html_to_text(html);
});
