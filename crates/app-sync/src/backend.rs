//! `Backend`: the outside world as seen by the effect executor.

use std::future::Future;
use std::pin::Pin;

use app_core::{AppState, Effect, Event};

use crate::error::SyncError;

/// A boxed, `Send` future (object-safe async trait methods without extra dependencies).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Performs effects against real or fake servers and storage.
///
/// Implementations: `Live` (IMAP/SMTP + redb, M1/M2) and `app_testkit::MemoryBackend`.
/// `Effect::ScheduleTick` is handled by the executor and never reaches a backend.
pub trait Backend: Send + Sync + 'static {
    /// Short name for diagnostics, e.g. `"memory"`.
    fn name(&self) -> &'static str;
    /// The persisted state to start a session from (`None` for a fresh profile).
    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>>;
    /// Performs one effect and reports what happened as events.
    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>>;
    /// Reads a stored raw message by content address (`None`: not stored). Backends without a
    /// blob store return `None`.
    fn get_blob<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Option<Vec<u8>>, SyncError>> {
        let _ = id;
        Box::pin(async { Ok(None) })
    }
}
