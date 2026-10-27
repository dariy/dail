//! Profile directories (`ARCHITECTURE.md §6`): `<data_dir>/dail/profiles/<name>/`, via
//! `directories` (XDG on Linux, `%APPDATA%` on Windows).

use std::path::{Path, PathBuf};

use crate::store::IoError;

/// The files and directories of one profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDirs {
    root: PathBuf,
}

impl ProfileDirs {
    /// The name of the profile used when none is given.
    pub const DEFAULT: &'static str = "default";

    /// The profile `name` under the platform data directory.
    pub fn named(name: &str) -> Result<Self, IoError> {
        let valid = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            return Err(IoError::InvalidProfile(name.to_owned()));
        }
        let dirs = directories::ProjectDirs::from("", "", "dail").ok_or(IoError::NoDataDir)?;
        Ok(Self::at(dirs.data_dir().join("profiles").join(name)))
    }

    /// A profile rooted at an explicit directory (tests, `--profile-dir`).
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Creates the profile directory if needed (user-only permissions on Unix).
    pub fn ensure(&self) -> Result<(), IoError> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder
            .create(&self.root)
            .map_err(|err| IoError::fs(&self.root, err))
    }

    /// The profile directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `config.toml`: settings and accounts, never secrets.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// `state.redb`: the local mail cache (M2).
    pub fn state_db(&self) -> PathBuf {
        self.root.join("state.redb")
    }

    /// `blobs/`: content-addressed raw messages (M2).
    pub fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    /// `index/`: the full-text index of downloaded bodies (ADR 0005).
    pub fn index_dir(&self) -> PathBuf {
        self.root.join("index")
    }

    /// `backups/`: daily copies of `config.toml` and `state.redb` (`store::backup`).
    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// `logs/`: replay logs (M7).
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// `updates/`: the newest downloaded update of dail (ADR 0011).
    pub fn updates_dir(&self) -> PathBuf {
        self.root.join("updates")
    }

    /// `lock`: single-instance lock (M7).
    pub fn lock_file(&self) -> PathBuf {
        self.root.join("lock")
    }
}
