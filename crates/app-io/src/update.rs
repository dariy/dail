//! The signed update manifest (Wave 7 S2, ADR 0011): parse `manifest.json`, verify
//! `manifest.json.minisig` against the keys built into the binary, and check a downloaded
//! artifact's size and SHA-256. Pure: the caller fetches the bytes.

use app_core::{UpdateArtifact, UpdateManifest};
use sha2::{Digest, Sha256};

/// The minisign public keys (base64, the last line of a `.pub` file) whose signatures dail
/// accepts. More than one only while a key is rotated (`packaging/README.md`). Empty until the
/// release key is generated with `cargo xtask release keygen`: until then every manifest is
/// refused, which fails closed.
pub const UPDATE_PUBLIC_KEYS: &[&str] = &[];

/// The largest manifest accepted, in bytes.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;

/// Why a manifest or an artifact was refused. Shown to the user in `update.status`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    /// No key is built in.
    #[error("this build trusts no update signing key")]
    NoKey,
    /// The signature file does not parse.
    #[error("the manifest signature is malformed: {0}")]
    BadSignatureFile(String),
    /// No trusted key verifies the signature.
    #[error("the manifest signature does not verify")]
    BadSignature,
    /// The manifest is too large or not the expected JSON.
    #[error("the manifest is malformed: {0}")]
    BadManifest(String),
    /// The trusted comment does not name the manifest's version.
    #[error("the signature is for {signed}, the manifest says {manifest}")]
    VersionMismatch {
        /// The version in the trusted comment.
        signed: String,
        /// The version in the manifest.
        manifest: String,
    },
    /// The manifest's version is not newer than the running one.
    #[error("the manifest's version {manifest} is not newer than {running}")]
    NotNewer {
        /// The manifest's version.
        manifest: String,
        /// The running version.
        running: String,
    },
    /// The downloaded file has another size.
    #[error("the download has {got} bytes, the manifest says {want}")]
    SizeMismatch {
        /// Bytes received.
        got: u64,
        /// Bytes listed.
        want: u64,
    },
    /// The downloaded file has another SHA-256.
    #[error("the download's sha256 does not match the manifest")]
    HashMismatch,
}

/// Verifies `manifest` (the bytes of `manifest.json`) with `signature` (the text of
/// `manifest.json.minisig`) against `keys`, and returns it if it is signed for its own version and
/// that version is newer than `running`.
pub fn verify_manifest(
    manifest: &[u8],
    signature: &str,
    keys: &[&str],
    running: &str,
) -> Result<UpdateManifest, UpdateError> {
    if keys.is_empty() {
        return Err(UpdateError::NoKey);
    }
    if manifest.len() > MAX_MANIFEST_BYTES {
        return Err(UpdateError::BadManifest(format!(
            "{} bytes, at most {MAX_MANIFEST_BYTES}",
            manifest.len()
        )));
    }
    let signature = minisign_verify::Signature::decode(signature)
        .map_err(|e| UpdateError::BadSignatureFile(e.to_string()))?;
    let verified = keys.iter().any(|key| {
        minisign_verify::PublicKey::from_base64(key)
            .is_ok_and(|key| key.verify(manifest, &signature, false).is_ok())
    });
    if !verified {
        return Err(UpdateError::BadSignature);
    }
    // Only now is the JSON read: until the signature verifies it is attacker-controlled.
    let parsed: UpdateManifest =
        serde_json::from_slice(manifest).map_err(|e| UpdateError::BadManifest(e.to_string()))?;
    let signed = signature
        .trusted_comment()
        .strip_prefix("version:")
        .unwrap_or_default()
        .trim();
    if signed != parsed.version {
        return Err(UpdateError::VersionMismatch {
            signed: signed.to_owned(),
            manifest: parsed.version,
        });
    }
    if !app_core::update::is_newer(&parsed.version, running) {
        return Err(UpdateError::NotNewer {
            manifest: parsed.version,
            running: running.to_owned(),
        });
    }
    Ok(parsed)
}

/// Checks a whole downloaded file against its manifest entry.
pub fn check_artifact(data: &[u8], artifact: &UpdateArtifact) -> Result<(), UpdateError> {
    let mut check = ArtifactCheck::new(artifact);
    check.update(data)?;
    check.finish()
}

/// [`check_artifact`] over a stream of chunks, for downloads written to disk as they arrive.
pub struct ArtifactCheck {
    hasher: Sha256,
    size: u64,
    want_size: u64,
    want_sha256: String,
}

impl ArtifactCheck {
    /// A check for `artifact`.
    pub fn new(artifact: &UpdateArtifact) -> Self {
        Self {
            hasher: Sha256::new(),
            size: 0,
            want_size: artifact.size,
            want_sha256: artifact.sha256.to_ascii_lowercase(),
        }
    }

    /// Adds a chunk. Fails as soon as the file is larger than listed.
    pub fn update(&mut self, chunk: &[u8]) -> Result<(), UpdateError> {
        self.size = self.size.saturating_add(chunk.len() as u64);
        if self.size > self.want_size {
            return Err(UpdateError::SizeMismatch {
                got: self.size,
                want: self.want_size,
            });
        }
        self.hasher.update(chunk);
        Ok(())
    }

    /// The verdict once every chunk was added.
    pub fn finish(self) -> Result<(), UpdateError> {
        if self.size != self.want_size {
            return Err(UpdateError::SizeMismatch {
                got: self.size,
                want: self.want_size,
            });
        }
        let hex: String = self
            .hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if hex != self.want_sha256 {
            return Err(UpdateError::HashMismatch);
        }
        Ok(())
    }
}
