//! Keeping the full-text index (ADR 0005, `app_io::fulltext`) complete: bodies stored before
//! the index existed (or before a crash lost uncommitted ones) are indexed in the background
//! when a backend starts.

use std::sync::Arc;

use app_core::{AppState, BodyState, MessageId};
use app_io::Store;
use app_io::fulltext::FullText;

/// Commit after this many bodies, so that a long re-index shows progress to searches.
const BATCH: usize = 500;

/// Indexes, on a blocking thread, the downloaded bodies of `state` that `index` lacks.
pub fn index_missing(index: Arc<FullText>, store: Arc<dyn Store>, state: &AppState) {
    let bodies: Vec<(MessageId, String)> = state
        .messages
        .values()
        .filter_map(|m| match &m.body {
            BodyState::Fetched { blob } => Some((m.id, blob.clone())),
            _ => None,
        })
        .collect();
    if bodies.is_empty() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("dail-index".to_owned())
        .spawn(move || {
            let mut added = 0_usize;
            for (message, blob) in bodies {
                if index.contains(message) {
                    continue;
                }
                let Ok(Some(raw)) = store.get_blob(&blob) else {
                    continue;
                };
                if let Err(err) = index.add(message, &raw) {
                    tracing::warn!(error = %err, "could not index a body");
                    return;
                }
                added += 1;
                if added.is_multiple_of(BATCH)
                    && let Err(err) = index.commit()
                {
                    tracing::warn!(error = %err, "could not commit the full-text index");
                    return;
                }
            }
            if let Err(err) = index.commit() {
                tracing::warn!(error = %err, "could not commit the full-text index");
            }
            tracing::debug!(added, "full-text index completed");
        });
    if let Err(err) = spawned {
        tracing::warn!(error = %err, "could not start indexing");
    }
}
