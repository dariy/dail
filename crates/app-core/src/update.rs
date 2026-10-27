//! Updates of dail itself (Wave 7, ADR 0011): the policy the user chose, what the runtime reported
//! about the running build, and where an update stands.
//!
//! The runtime fetches and verifies the signed manifest; core compares versions, picks the
//! artifact for the running build's install kind, and decides when a check is due.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Whether dail checks for updates on its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChecks {
    /// The user has not been asked yet: no checks (the first-run question asks).
    #[default]
    Unset,
    /// Periodic checks every `interval_hours`.
    On,
    /// No periodic checks. `check_for_updates` still works on request.
    Off,
}

/// The user's update preferences (`[updates]` in `config.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct UpdatePolicy {
    /// Periodic checks on, off, or not asked yet.
    pub checks: UpdateChecks,
    /// Hours between periodic checks (1 to 720).
    pub interval_hours: u64,
    /// A version the user chose to skip: it is not offered again (a newer one is).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_version: Option<String>,
}

impl Default for UpdatePolicy {
    fn default() -> Self {
        Self {
            checks: UpdateChecks::Unset,
            interval_hours: 24,
            skipped_version: None,
        }
    }
}

impl UpdatePolicy {
    /// The shortest interval accepted, in hours.
    pub const MIN_INTERVAL_HOURS: u64 = 1;
    /// The longest interval accepted, in hours (30 days).
    pub const MAX_INTERVAL_HOURS: u64 = 720;

    /// The interval in milliseconds.
    pub fn interval_ms(&self) -> i64 {
        i64::try_from(self.interval_hours)
            .unwrap_or(i64::MAX)
            .saturating_mul(3_600_000)
    }
}

/// How the running dail was installed, which decides the artifact and the install strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    /// An AppImage (`$APPIMAGE` is set): replaced in place.
    Appimage,
    /// A plain binary (`dail`, `dail.exe`): replaced in place.
    Binary,
    /// The Debian package: `pkexec apt-get install ./<file>.deb`.
    Deb,
    /// The Windows installer: `msiexec /i <file>`.
    Msi,
    /// The Android app: notify only, the user opens the release page.
    Android,
    /// The iOS app: notify only, the user opens the release page.
    Ios,
}

impl InstallKind {
    /// Whether dail can download and install the update itself (desktop) or only tells the user
    /// (mobile).
    pub const fn self_updates(self) -> bool {
        !matches!(self, Self::Android | Self::Ios)
    }

    /// Whether `kind` (a manifest artifact kind) is the one this install updates from.
    pub fn takes(self, kind: &str) -> bool {
        match self {
            Self::Appimage => kind == "appimage",
            Self::Binary => kind == "exe",
            Self::Deb => kind == "deb",
            Self::Msi => kind == "msi",
            Self::Android => kind.starts_with("apk-"),
            Self::Ios => kind == "ipa",
        }
    }
}

/// What the runtime knows about the running build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateHost {
    /// The running version, `x.y.z`.
    pub version: String,
    /// The build's target (`x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `android`, `ios`).
    pub target: String,
    /// How it was installed.
    pub install: InstallKind,
}

/// One installable file of a release, as the signed manifest lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateArtifact {
    /// The target it runs on.
    pub target: String,
    /// `deb`, `appimage`, `msi`, `exe`, `apk-<abi>` or `ipa`.
    pub kind: String,
    /// Where to download it (a GitHub release asset).
    pub url: String,
    /// Its SHA-256, lower-case hex.
    pub sha256: String,
    /// Its size in bytes.
    pub size: u64,
}

/// The release manifest, after the runtime verified its signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateManifest {
    /// The release version, `x.y.z`.
    pub version: String,
    /// When it was published, in Unix milliseconds.
    pub published_ms: i64,
    /// The release page (notes, every asset).
    pub notes_url: String,
    /// The installable files.
    pub artifacts: Vec<UpdateArtifact>,
}

/// A newer release, offered to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateRelease {
    /// Its version.
    pub version: String,
    /// When it was published, in Unix milliseconds.
    pub published_ms: i64,
    /// The release page.
    pub notes_url: String,
    /// The file for this install, if the release has one. Mobile installs only open
    /// `notes_url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<UpdateArtifact>,
}

/// Where an update stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateStatus {
    /// Nothing going on: no check yet, or the last one found nothing newer.
    #[default]
    Idle,
    /// A check is running.
    Checking,
    /// A newer release is available.
    Available {
        /// The release.
        release: UpdateRelease,
    },
    /// Its artifact is being downloaded.
    Downloading {
        /// The release.
        release: UpdateRelease,
    },
    /// The artifact is downloaded and verified, ready to install.
    Ready {
        /// The release.
        release: UpdateRelease,
        /// The downloaded file.
        path: String,
    },
    /// The installer is running.
    Installing {
        /// The release.
        release: UpdateRelease,
    },
    /// The update is installed; it takes effect when dail restarts.
    Installed {
        /// The installed version.
        version: String,
    },
    /// A check, download or install failed.
    Failed {
        /// What went wrong.
        reason: String,
    },
}

impl UpdateStatus {
    /// Whether a check, download or install is running.
    pub const fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Downloading { .. } | Self::Installing { .. }
        )
    }
}

/// Updates in [`crate::AppState`]. Not saved: the policy lives in [`crate::Settings::updates`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct UpdateState {
    /// The running build, once the runtime reported it (`update_host_detected`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<UpdateHost>,
    /// Where an update stands.
    pub status: UpdateStatus,
    /// When the last check finished (found something, nothing or failed), Unix milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_ms: Option<i64>,
    /// When the next periodic check is due, once a tick is scheduled for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_check_ms: Option<i64>,
}

/// `x.y.z` as numbers; `None` if `version` is not three numeric parts.
pub fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().split('.');
    let mut next = || -> Option<u64> {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let version = (next()?, next()?, next()?);
    parts.next().is_none().then_some(version)
}

/// Whether `candidate` is a newer version than `running`. Unparseable versions are never newer.
pub fn is_newer(candidate: &str, running: &str) -> bool {
    match (parse_version(candidate), parse_version(running)) {
        (Some(candidate), Some(running)) => candidate > running,
        _ => false,
    }
}

/// The artifact of `manifest` for `host`: same target (or the mobile platform) and the kind its
/// install takes.
pub fn artifact_for<'a>(
    manifest: &'a UpdateManifest,
    host: &UpdateHost,
) -> Option<&'a UpdateArtifact> {
    manifest
        .artifacts
        .iter()
        .find(|a| a.target == host.target && host.install.takes(&a.kind))
}
