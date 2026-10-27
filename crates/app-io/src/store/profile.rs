//! `ProfileStore`: the whole durable state of a profile.

use app_core::{AppState, PersistDelta};

use super::backup::{self, Recovery};
use super::{BlobDir, IoError, RedbStore, Store};
use crate::config::ConfigStore;
use crate::profile::ProfileDirs;

/// Settings and accounts in `config.toml`, the mail cache in `state.redb`, raw messages in
/// `blobs/`. Deltas are routed to the file that holds them.
#[derive(Debug)]
pub struct ProfileStore {
    config: ConfigStore,
    cache: RedbStore,
    blobs: BlobDir,
    recovery: Option<Recovery>,
}

impl ProfileStore {
    /// Opens the stores of `dirs` (the profile directory must exist; the caller holds its
    /// lock). A `state.redb` that does not open is set aside and replaced by the newest
    /// backup's; once both files open, the daily backup is made (`store::backup`), so a broken
    /// file never becomes a backup.
    pub fn open(dirs: &ProfileDirs) -> Result<Self, IoError> {
        let now = std::time::SystemTime::now();
        let config = ConfigStore::open(dirs.config_file())?;
        let (cache, recovery) = match RedbStore::open(dirs.state_db()) {
            Ok(cache) => (cache, None),
            Err(err @ IoError::Unavailable(_)) if dirs.state_db().is_file() => {
                tracing::error!(error = %err, "state.redb does not open; recovering");
                let recovery = backup::recover(dirs, "state.redb", now)?;
                tracing::warn!(
                    broken = %recovery.broken.display(),
                    backup = ?recovery.restored_from.as_ref().map(|b| &b.name),
                    "state.redb set aside"
                );
                (RedbStore::open(dirs.state_db())?, Some(recovery))
            }
            Err(err) => return Err(err),
        };
        // Closed for the copy (Windows locks an open database file).
        drop(cache);
        match backup::backup_if_due(dirs, now) {
            Ok(Some(made)) => tracing::info!(backup = %made.path.display(), "profile backed up"),
            Ok(None) => {}
            Err(err) => tracing::warn!(error = %err, "profile backup failed"),
        }
        Ok(Self {
            config,
            cache: RedbStore::open(dirs.state_db())?,
            blobs: BlobDir::new(dirs.blobs_dir()),
            recovery,
        })
    }

    /// What opening had to recover, if anything.
    pub fn recovery(&self) -> Option<&Recovery> {
        self.recovery.as_ref()
    }

    /// The mail cache.
    pub fn cache(&self) -> &RedbStore {
        &self.cache
    }
}

impl Store for ProfileStore {
    fn load_snapshot(&self) -> Result<Option<AppState>, IoError> {
        let config = self.config.load_snapshot()?;
        let mut state = config.clone().unwrap_or_default();
        self.cache.load_into(&mut state)?;
        // Drop cache entries of accounts that are gone from the config (e.g. hand-edited).
        let accounts = state.accounts.clone();
        state
            .mailboxes
            .retain(|_, m| accounts.contains_key(&m.account));
        state
            .messages
            .retain(|_, m| accounts.contains_key(&m.account));
        state
            .threads
            .retain(|_, t| accounts.contains_key(&t.account));
        state
            .outbox
            .retain(|_, op| accounts.contains_key(&op.account));
        state
            .drafts
            .retain(|_, d| accounts.contains_key(&d.account));
        state
            .approvals
            .retain(|_, a| accounts.contains_key(&a.account));
        if config.is_none() && state.mailboxes.is_empty() && state.messages.is_empty() {
            return Ok(None);
        }
        Ok(Some(state))
    }

    fn apply(&self, delta: &PersistDelta) -> Result<(), IoError> {
        match delta {
            PersistDelta::Mail(mail) => self.cache.apply_mail(mail),
            PersistDelta::Settings(_)
            | PersistDelta::Account(_)
            | PersistDelta::AccountRemoved(_)
            | PersistDelta::View(_) => self.config.apply(delta),
        }
    }

    fn put_blob(&self, bytes: &[u8]) -> Result<String, IoError> {
        self.blobs.put(bytes)
    }

    fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>, IoError> {
        self.blobs.get(id)
    }

    fn compact(
        &self,
        keep: &std::collections::BTreeSet<String>,
        written_before: std::time::SystemTime,
    ) -> Result<super::Compacted, IoError> {
        let collected = self.blobs.collect(keep, written_before)?;
        self.cache.compact()?;
        Ok(collected)
    }
}
