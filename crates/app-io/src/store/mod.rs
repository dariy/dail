//! The local store: durable snapshot of `AppState` (`ARCHITECTURE.md §6`).
//!
//! - [`RedbStore`]: the mail cache in `state.redb` (mailboxes, envelopes, threads).
//! - [`BlobDir`]: content-addressed raw messages under `blobs/`.
//! - [`ProfileStore`]: a whole profile: `config.toml` + `state.redb` + `blobs/`.

pub mod backup;
mod blobs;
mod cache;
pub mod migrations;
mod profile;

use std::collections::BTreeSet;
use std::path::Path;
use std::time::SystemTime;

use app_core::{AppState, PersistDelta};

pub use blobs::{BlobDir, blob_id};
pub use cache::RedbStore;
pub use profile::ProfileStore;

/// Errors of the local store, the config file and the profile directories.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    /// The store could not be read or written.
    #[error("store unavailable: {0}")]
    Unavailable(String),
    /// A file or directory could not be read or written.
    #[error("{path}: {source}")]
    Fs {
        /// The path involved.
        path: String,
        /// Underlying error.
        source: std::io::Error,
    },
    /// `config.toml` is not valid TOML or holds invalid values.
    #[error("invalid config {path}: {reason}")]
    Config {
        /// The config file.
        path: String,
        /// What is wrong.
        reason: String,
    },
    /// The platform has no home/data directory to put profiles in.
    #[error("no data directory found for profiles (is $HOME set?)")]
    NoDataDir,
    /// A profile name contains characters that are not allowed.
    #[error("invalid profile name `{0}`: use letters, digits, `-` and `_` only")]
    InvalidProfile(String),
    /// `state.redb` was written by a newer dail.
    #[error("{path}: schema version {found} is newer than this dail supports ({supported})")]
    SchemaTooNew {
        /// The database file.
        path: String,
        /// Version found in the file.
        found: u64,
        /// Newest version this build knows.
        supported: u64,
    },
    /// A blob ID is not a lower-case hex SHA-256.
    #[error("invalid blob id `{0}`")]
    InvalidBlobId(String),
}

impl IoError {
    /// Wraps an `std::io::Error` with the path it happened on.
    pub fn fs(path: &Path, source: std::io::Error) -> Self {
        Self::Fs {
            path: path.display().to_string(),
            source,
        }
    }
}

/// Durable storage of state and raw messages. Implementations: `ConfigStore` (settings and
/// accounts in `config.toml`), [`RedbStore`] (mail cache), [`ProfileStore`] (both, plus blobs)
/// and `app_testkit::MemoryStore`.
///
/// Methods are synchronous; async runtimes call them from a blocking-friendly context.
pub trait Store: Send + Sync {
    /// The last persisted state, or `None` for a fresh profile.
    fn load_snapshot(&self) -> Result<Option<AppState>, IoError>;
    /// Persists one change. Deltas a store does not hold are ignored.
    fn apply(&self, delta: &PersistDelta) -> Result<(), IoError>;
    /// Stores a raw message and returns its content address ([`blob_id`]). Storing the same
    /// bytes twice is a no-op.
    fn put_blob(&self, bytes: &[u8]) -> Result<String, IoError>;
    /// Reads a blob by content address; `None` if it is not stored.
    fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>, IoError>;
    /// Compacts the store: gives free space back and deletes the blobs not in `keep` that were
    /// last written before `written_before` (a blob written meanwhile may belong to a body whose
    /// `BodyFetched` the engine has not seen yet). Stores without anything to compact do
    /// nothing.
    fn compact(
        &self,
        keep: &BTreeSet<String>,
        written_before: SystemTime,
    ) -> Result<Compacted, IoError> {
        let _ = (keep, written_before);
        Ok(Compacted::default())
    }
}

/// What [`Store::compact`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Compacted {
    /// Blobs deleted.
    pub removed_blobs: u64,
    /// Bytes they took.
    pub freed_bytes: u64,
}

impl IoError {
    /// Wraps a redb (or other) error on `path`.
    pub fn db(path: &Path, err: impl std::fmt::Display) -> Self {
        Self::Unavailable(format!("{}: {err}", path.display()))
    }
}
