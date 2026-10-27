//! The signed update manifest (Wave 7 S2): only a manifest signed by a trusted key, for its own
//! version, newer than the running one, is accepted; artifacts must match size and SHA-256.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use app_io::update::{UPDATE_PUBLIC_KEYS, UpdateError, check_artifact, verify_manifest};

fn testdata(name: &str) -> Vec<u8> {
    let dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
    std::fs::read(dir.join("testdata/updates").join(name)).unwrap()
}

fn text(name: &str) -> String {
    String::from_utf8(testdata(name)).unwrap()
}

/// The test key: the last line of `testdata/updates/test.pub`. Never trusted by a build.
fn test_key() -> String {
    text("test.pub").lines().last().unwrap().trim().to_owned()
}

#[test]
fn a_signed_newer_manifest_is_accepted() {
    let key = test_key();
    let manifest = verify_manifest(
        &testdata("manifest.json"),
        &text("manifest.json.minisig"),
        &[&key],
        "0.1.0",
    )
    .unwrap();
    assert_eq!(manifest.version, "0.99.0");
    assert!(manifest.artifacts.iter().any(|a| a.kind == "appimage"));
}

#[test]
fn every_tampering_is_refused() {
    let key = test_key();
    let keys = [key.as_str()];
    let manifest = testdata("manifest.json");
    let signature = text("manifest.json.minisig");

    let mut tampered = manifest.clone();
    let at = tampered.windows(6).position(|w| w == b"0.99.0").unwrap();
    tampered[at + 2] = b'8';
    assert_eq!(
        verify_manifest(&tampered, &signature, &keys, "0.1.0"),
        Err(UpdateError::BadSignature)
    );

    let other = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    assert_eq!(
        verify_manifest(&manifest, &signature, &[other], "0.1.0"),
        Err(UpdateError::BadSignature)
    );
    assert_eq!(
        verify_manifest(&manifest, &signature, &[], "0.1.0"),
        Err(UpdateError::NoKey)
    );
    assert!(matches!(
        verify_manifest(&manifest, "not a signature", &keys, "0.1.0"),
        Err(UpdateError::BadSignatureFile(_))
    ));
    assert_eq!(
        verify_manifest(
            &manifest,
            &text("manifest.json.wrong-comment.minisig"),
            &keys,
            "0.1.0"
        ),
        Err(UpdateError::VersionMismatch {
            signed: "0.98.0".to_owned(),
            manifest: "0.99.0".to_owned()
        })
    );
    // No downgrades, no reinstalls.
    for running in ["0.99.0", "1.0.0"] {
        assert!(matches!(
            verify_manifest(&manifest, &signature, &keys, running),
            Err(UpdateError::NotNewer { .. })
        ));
    }
    assert!(matches!(
        verify_manifest(&vec![b' '; 70_000], &signature, &keys, "0.1.0"),
        Err(UpdateError::BadManifest(_))
    ));
}

#[test]
fn artifacts_must_match_size_and_hash() {
    let key = test_key();
    let manifest = verify_manifest(
        &testdata("manifest.json"),
        &text("manifest.json.minisig"),
        &[&key],
        "0.1.0",
    )
    .unwrap();
    let artifact = &manifest.artifacts[0];
    let data = testdata("artifact.bin");
    check_artifact(&data, artifact).unwrap();

    let mut flipped = data.clone();
    flipped[0] ^= 1;
    assert_eq!(
        check_artifact(&flipped, artifact),
        Err(UpdateError::HashMismatch)
    );
    assert!(matches!(
        check_artifact(&data[1..], artifact),
        Err(UpdateError::SizeMismatch { .. })
    ));
    let mut longer = data.clone();
    longer.push(b'x');
    assert!(matches!(
        check_artifact(&longer, artifact),
        Err(UpdateError::SizeMismatch { .. })
    ));
}

#[test]
fn the_built_in_keys_parse() {
    for key in UPDATE_PUBLIC_KEYS {
        assert!(minisign_key_parses(key), "{key}");
    }
}

fn minisign_key_parses(key: &str) -> bool {
    // A minisign public key is 42 bytes in base64 (56 characters) starting with `RW`.
    key.len() == 56 && key.starts_with("RW")
}
