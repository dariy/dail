//! The fake update server (Wave 7 S5): a release `0.99.0` with a manifest signed by the test key
//! (`crates/app-io/testdata/updates/`), one artifact for every target and kind, and switches that
//! tamper with the manifest, its signature or the artifact. The memory backend answers the update
//! effects from it, through the same `app_io::update` checks as the live backend.

use app_core::{Event, InstallKind, UpdateArtifact, UpdateHost};

/// The manifest of release `0.99.0`, signed by [`TEST_KEY`].
pub const MANIFEST: &[u8] = include_bytes!("../../app-io/testdata/updates/manifest.json");
/// Its signature.
pub const SIGNATURE: &str = include_str!("../../app-io/testdata/updates/manifest.json.minisig");
/// The artifact every manifest entry describes.
pub const ARTIFACT: &[u8] = include_bytes!("../../app-io/testdata/updates/artifact.bin");
/// The test public key (`test.pub`). No build trusts it; its secret key was thrown away.
pub const TEST_KEY: &str = "RWTN7BiQXxnx3cVdk4lZf/rZD4XVNRcDZK+9zyQ0isYvNRduzMJeMpm6";

/// The version of the fake release.
pub const VERSION: &str = "0.99.0";

/// The build a memory session reports: this version, Linux, a plain binary.
pub fn host() -> UpdateHost {
    UpdateHost {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        target: "x86_64-unknown-linux-gnu".to_owned(),
        install: InstallKind::Binary,
    }
}

/// What the fake server serves, and what it saw installed.
#[derive(Debug, Clone)]
pub struct FakeUpdates {
    /// `manifest.json`.
    pub manifest: Vec<u8>,
    /// `manifest.json.minisig`.
    pub signature: String,
    /// The artifact's bytes.
    pub artifact: Vec<u8>,
    /// Downloads, by path.
    pub downloads: Vec<(String, Vec<u8>)>,
    /// Versions installed, in order.
    pub installed: Vec<String>,
}

impl Default for FakeUpdates {
    fn default() -> Self {
        Self {
            manifest: MANIFEST.to_vec(),
            signature: SIGNATURE.to_owned(),
            artifact: ARTIFACT.to_vec(),
            downloads: Vec::new(),
            installed: Vec::new(),
        }
    }
}

impl FakeUpdates {
    /// Changes one byte of the manifest (its version), so the signature no longer verifies.
    pub fn tamper_manifest(&mut self) {
        if let Some(at) = self
            .manifest
            .windows(VERSION.len())
            .position(|w| w == VERSION.as_bytes())
        {
            self.manifest[at] = b'1';
        }
    }

    /// Replaces the signature with one of another key.
    pub fn tamper_signature(&mut self) {
        // The same file with the key ID and signature bytes flipped.
        let mut lines: Vec<String> = self.signature.lines().map(str::to_owned).collect();
        if let Some(line) = lines.get_mut(1) {
            let flipped: String = line
                .chars()
                .enumerate()
                .map(|(i, c)| match (i, c) {
                    (20, 'A') => 'B',
                    (20, _) => 'A',
                    _ => c,
                })
                .collect();
            *line = flipped;
        }
        self.signature = lines.join("\n") + "\n";
    }

    /// Flips a bit of the artifact, so its SHA-256 no longer matches.
    pub fn tamper_artifact(&mut self) {
        if let Some(byte) = self.artifact.first_mut() {
            *byte ^= 1;
        }
    }

    /// `CheckForUpdates`.
    pub fn check(&self, running: &str) -> Event {
        match app_io::update::verify_manifest(&self.manifest, &self.signature, &[TEST_KEY], running)
        {
            Ok(manifest) => Event::UpdateChecked { manifest },
            Err(err) => Event::UpdateFailed {
                reason: err.to_string(),
            },
        }
    }

    /// `DownloadUpdate`: "downloads" the artifact to `memory://updates/<name>`.
    pub fn download(&mut self, version: String, artifact: &UpdateArtifact) -> Event {
        if let Err(err) = app_io::update::check_artifact(&self.artifact, artifact) {
            return Event::UpdateFailed {
                reason: err.to_string(),
            };
        }
        let name = artifact.url.rsplit('/').next().unwrap_or("dail-update");
        let path = format!("memory://updates/{name}");
        self.downloads.clear();
        self.downloads.push((path.clone(), self.artifact.clone()));
        Event::UpdateDownloaded { version, path }
    }

    /// `InstallUpdate`: records the version if `path` was downloaded.
    pub fn install(&mut self, version: String, path: &str) -> Event {
        if !self.downloads.iter().any(|(p, _)| p == path) {
            return Event::UpdateFailed {
                reason: format!("{path} was not downloaded"),
            };
        }
        self.installed.push(version.clone());
        Event::UpdateInstalled { version }
    }
}
