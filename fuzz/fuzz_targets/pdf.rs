//! PDF attachments (ADR 0012): page sizes, then the first page at a small width.
#![no_main]

use std::sync::Arc;

use app_io::pdf::PdfData;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let data = PdfData::from(Arc::new(bytes.to_vec()));
    if app_io::pdf::pages(&data).is_ok_and(|pages| !pages.is_empty()) {
        let _ = app_io::pdf::render(&data, 0, 64, None);
    }
});
