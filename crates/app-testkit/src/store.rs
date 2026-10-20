//! `MemoryStore`: an `app_io::Store` that keeps the snapshot in memory.

use std::collections::BTreeMap;

use app_core::{AppState, PersistDelta};
use app_io::{IoError, Store, blob_id};
use parking_lot::Mutex;

/// In-memory store. Holds a whole `AppState` snapshot and applies deltas to it; blobs are kept
/// in a map by content address.
#[derive(Debug, Default)]
pub struct MemoryStore {
    snapshot: Mutex<Option<AppState>>,
    blobs: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl MemoryStore {
    /// A store that starts with `state` already persisted.
    pub fn with_state(state: AppState) -> Self {
        Self {
            snapshot: Mutex::new(Some(state)),
            ..Self::default()
        }
    }

    /// The persisted snapshot (for assertions).
    pub fn snapshot(&self) -> Option<AppState> {
        self.snapshot.lock().clone()
    }

    /// Number of stored blobs (for assertions).
    pub fn blob_count(&self) -> usize {
        self.blobs.lock().len()
    }
}

impl Store for MemoryStore {
    fn load_snapshot(&self) -> Result<Option<AppState>, IoError> {
        Ok(self.snapshot.lock().clone())
    }

    fn apply(&self, delta: &PersistDelta) -> Result<(), IoError> {
        let mut snapshot = self.snapshot.lock();
        delta.apply_to(snapshot.get_or_insert_with(AppState::default));
        Ok(())
    }

    fn put_blob(&self, bytes: &[u8]) -> Result<String, IoError> {
        let id = blob_id(bytes);
        self.blobs
            .lock()
            .entry(id.clone())
            .or_insert_with(|| bytes.to_vec());
        Ok(id)
    }

    fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>, IoError> {
        Ok(self.blobs.lock().get(id).cloned())
    }

    /// Deletes the blobs not in `keep` (the in-memory store has no write times: all count as
    /// old).
    fn compact(
        &self,
        keep: &std::collections::BTreeSet<String>,
        _written_before: std::time::SystemTime,
    ) -> Result<app_io::Compacted, IoError> {
        let mut done = app_io::Compacted::default();
        self.blobs.lock().retain(|id, bytes| {
            let kept = keep.contains(id);
            if !kept {
                done.removed_blobs += 1;
                done.freed_bytes += u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            }
            kept
        });
        Ok(done)
    }
}
