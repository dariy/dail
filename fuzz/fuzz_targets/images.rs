//! Inline and attached images (PNG, JPEG, GIF, WebP).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = app_io::images::decode(bytes);
});
