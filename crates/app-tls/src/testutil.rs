//! Helpers for the unit tests: hex and the Wycheproof vector files (`testdata/wycheproof/`).

use std::path::PathBuf;

use serde::Deserialize;

/// Decodes a hex string; panics on bad input (test data only).
pub(crate) fn hex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "odd hex length: {s}");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Decodes a hex string of exactly `N` bytes.
pub(crate) fn hex_array<const N: usize>(s: &str) -> [u8; N] {
    hex(s).try_into().unwrap()
}

/// One Wycheproof file: groups of test cases that share parameters (`G`), each case with fields `T`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Wycheproof<G> {
    pub(crate) number_of_tests: usize,
    pub(crate) test_groups: Vec<G>,
}

/// Wycheproof's verdict for a case. `Acceptable` cases are legal but weak or unusual; dail rejects them.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Expect {
    Valid,
    Invalid,
    Acceptable,
}

/// The crate's directory. Cargo sets `CARGO_MANIFEST_DIR` when it runs a test, and `cargo xtask
/// android test` sets it to where it pushed the test data on the emulator; the build-time value is
/// the fallback.
pub(crate) fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

/// Reads `testdata/wycheproof/<name>`.
pub(crate) fn wycheproof<G: for<'de> Deserialize<'de>>(name: &str) -> Wycheproof<G> {
    let path = manifest_dir().join("testdata/wycheproof").join(name);
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("{}: {err}", path.display()))
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The signature hash named in a Wycheproof group (`"SHA-256"`, …).
pub(crate) fn sig_hash(name: &str) -> crate::hash::SigHash {
    use crate::hash::SigHash;
    let hash = match name {
        "SHA-256" => Some(SigHash::Sha256),
        "SHA-384" => Some(SigHash::Sha384),
        "SHA-512" => Some(SigHash::Sha512),
        _ => None,
    };
    hash.unwrap()
}
