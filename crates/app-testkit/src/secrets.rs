//! `MemorySecrets`: an `app_sync::SecretStore` in memory, for device tokens of remote profiles
//! (ADR 0017) in tests and `--backend memory` runs.

use std::collections::BTreeMap;

use app_core::{Secret, SecretRef};
use app_sync::{SecretStore, SyncError};
use parking_lot::Mutex;

/// Keyring entries in a map; values never appear in `Debug` output.
#[derive(Debug, Default)]
pub struct MemorySecrets {
    entries: Mutex<BTreeMap<SecretRef, Secret>>,
}

impl MemorySecrets {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether there is an entry for `secret`.
    pub fn contains(&self, secret: &SecretRef) -> bool {
        self.entries.lock().contains_key(secret)
    }
}

impl SecretStore for MemorySecrets {
    fn set(&self, secret: &SecretRef, value: &Secret) -> Result<(), SyncError> {
        self.entries.lock().insert(secret.clone(), value.clone());
        Ok(())
    }

    fn get(&self, secret: &SecretRef) -> Result<Option<Secret>, SyncError> {
        Ok(self.entries.lock().get(secret).cloned())
    }

    fn delete(&self, secret: &SecretRef) -> Result<(), SyncError> {
        self.entries.lock().remove(secret);
        Ok(())
    }
}
