//! `RedbStore`: the mail cache in `state.redb` (`ARCHITECTURE.md §6`).
//!
//! Tables map 128-bit IDs to JSON records: `mailboxes`, `messages` (envelopes), `threads`,
//! `outbox` (pending ops), `drafts`, `approvals` (agents' send requests); `meta` holds `schema_version`;
//! `synced` holds one row, `document`: the synced settings with their unpushed-changes flag
//! (SET-4; absent in older profiles, which start with an empty document). Each [`MailDelta`] is applied in one ACID write transaction.
//!
//! An unreadable row of the cache proper (mailboxes, messages, threads) is skipped: sync
//! fetches it again. An unreadable draft, queued operation or approval is the user's own work, so
//! opening the store moves it into `quarantine` with its raw bytes, and the state lists it
//! (`StatusState::quarantined`). Nothing is ever dropped (STORE-6).

use std::path::{Path, PathBuf};

use app_core::{
    AppState, ApprovalRequest, Draft, Envelope, LocalSettings, MailDelta, Mailbox, PendingOp,
    QuarantinedRow, Thread,
};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::IoError;
use super::migrations;

const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const MAILBOXES: TableDefinition<u128, &[u8]> = TableDefinition::new("mailboxes");
const MESSAGES: TableDefinition<u128, &[u8]> = TableDefinition::new("messages");
const THREADS: TableDefinition<u128, &[u8]> = TableDefinition::new("threads");
const OUTBOX: TableDefinition<u128, &[u8]> = TableDefinition::new("outbox");
const DRAFTS: TableDefinition<u128, &[u8]> = TableDefinition::new("drafts");
const APPROVALS: TableDefinition<u128, &[u8]> = TableDefinition::new("approvals");
/// Rows of `drafts`, `outbox` and `approvals` that did not read, moved aside (STORE-6).
const QUARANTINE: TableDefinition<u128, &[u8]> = TableDefinition::new("quarantine");
/// The synced settings document (SET-4): one row under [`SYNCED_ROW`].
const SYNCED: TableDefinition<&str, &[u8]> = TableDefinition::new("synced");
const SYNCED_ROW: &str = "document";
const SCHEMA_VERSION: &str = "schema_version";

/// The mail cache of one profile.
pub struct RedbStore {
    path: PathBuf,
    // Transactions share it; `compact` needs it alone.
    db: parking_lot::RwLock<Database>,
}

impl std::fmt::Debug for RedbStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl RedbStore {
    /// Opens (or creates) the database at `path` and migrates it to the current schema.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, IoError> {
        let path = path.into();
        let db = Database::create(&path).map_err(|err| IoError::db(&path, err))?;
        let store = Self {
            path,
            db: parking_lot::RwLock::new(db),
        };
        store.migrate()?;
        store.quarantine()?;
        Ok(store)
    }

    /// The database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Checks the file's checksums and structure (redb `check_integrity`; repairs what it
    /// can). `false`: it had to repair something.
    pub fn check_integrity(&self) -> Result<bool, IoError> {
        self.db.write().check_integrity().map_err(|e| self.err(e))
    }

    /// Gives the free pages back to the file system (redb `compact`).
    pub fn compact(&self) -> Result<(), IoError> {
        self.db.write().compact().map_err(|e| self.err(e))?;
        Ok(())
    }

    fn err(&self, err: impl std::fmt::Display) -> IoError {
        IoError::db(&self.path, err)
    }

    fn migrate(&self) -> Result<(), IoError> {
        let txn = self.db.read().begin_write().map_err(|e| self.err(e))?;
        let found = {
            let meta = txn.open_table(META).map_err(|e| self.err(e))?;
            meta.get(SCHEMA_VERSION)
                .map_err(|e| self.err(e))?
                .map(|v| v.value())
        };
        match found {
            Some(version) if version > migrations::CURRENT => {
                return Err(IoError::SchemaTooNew {
                    path: self.path.display().to_string(),
                    found: version,
                    supported: migrations::CURRENT,
                });
            }
            Some(version) => {
                for migration in migrations::pending(version) {
                    migration(&txn).map_err(|e| self.err(e))?;
                }
            }
            None => {}
        }
        // Creates missing tables (fresh databases; `drafts` and `approvals` are additive within
        // version 1) and
        // stamps the version.
        for table in [
            MAILBOXES, MESSAGES, THREADS, OUTBOX, DRAFTS, APPROVALS, QUARANTINE,
        ] {
            txn.open_table(table).map_err(|e| self.err(e))?;
        }
        // Additive within version 1, like `drafts`.
        txn.open_table(SYNCED).map_err(|e| self.err(e))?;
        txn.open_table(META)
            .map_err(|e| self.err(e))?
            .insert(SCHEMA_VERSION, migrations::CURRENT)
            .map_err(|e| self.err(e))?;
        txn.commit().map_err(|e| self.err(e))
    }

    /// Moves the rows of `drafts`, `outbox` and `approvals` that do not read into `quarantine`.
    fn quarantine(&self) -> Result<(), IoError> {
        let txn = self.db.read().begin_write().map_err(|e| self.err(e))?;
        let mut moved = self.quarantine_table::<Draft>(&txn, DRAFTS)?;
        moved += self.quarantine_table::<PendingOp>(&txn, OUTBOX)?;
        moved += self.quarantine_table::<ApprovalRequest>(&txn, APPROVALS)?;
        if moved > 0 {
            tracing::warn!(rows = moved, "quarantined unreadable rows");
        }
        txn.commit().map_err(|e| self.err(e))
    }

    fn quarantine_table<T: DeserializeOwned>(
        &self,
        txn: &redb::WriteTransaction,
        table: TableDefinition<u128, &[u8]>,
    ) -> Result<usize, IoError> {
        let name = redb::TableHandle::name(&table).to_owned();
        let mut source = txn.open_table(table).map_err(|e| self.err(e))?;
        let mut bad = Vec::new();
        for row in source.iter().map_err(|e| self.err(e))? {
            let (key, value) = row.map_err(|e| self.err(e))?;
            if let Err(err) = serde_json::from_slice::<T>(value.value()) {
                bad.push((key.value(), value.value().to_vec(), err.to_string()));
            }
        }
        let mut quarantine = txn.open_table(QUARANTINE).map_err(|e| self.err(e))?;
        for (key, raw, error) in &bad {
            let held = Held {
                table: name.clone(),
                error: error.clone(),
                raw: String::from_utf8_lossy(raw).into_owned(),
            };
            let bytes = self.encode(&held)?;
            quarantine
                .insert(*key, bytes.as_slice())
                .map_err(|e| self.err(e))?;
            source.remove(*key).map_err(|e| self.err(e))?;
        }
        Ok(bad.len())
    }

    /// The schema version recorded in the file.
    pub fn schema_version(&self) -> Result<u64, IoError> {
        let txn = self.db.read().begin_read().map_err(|e| self.err(e))?;
        let meta = txn.open_table(META).map_err(|e| self.err(e))?;
        Ok(meta
            .get(SCHEMA_VERSION)
            .map_err(|e| self.err(e))?
            .map_or(0, |v| v.value()))
    }

    /// Fills the mail collections of `state` from the cache, and `status.quarantined` from the
    /// rows kept aside. Unreadable cache records are skipped.
    pub fn load_into(&self, state: &mut AppState) -> Result<(), IoError> {
        let txn = self.db.read().begin_read().map_err(|e| self.err(e))?;
        {
            let quarantine = txn.open_table(QUARANTINE).map_err(|e| self.err(e))?;
            for row in quarantine.iter().map_err(|e| self.err(e))? {
                let (key, value) = row.map_err(|e| self.err(e))?;
                if let Ok(held) = serde_json::from_slice::<Held>(value.value()) {
                    state.status.quarantined.push(QuarantinedRow {
                        table: held.table,
                        key: uuid::Uuid::from_u128(key.value()).to_string(),
                        error: held.error,
                    });
                }
            }
        }
        for mailbox in self.read_all::<Mailbox>(&txn, MAILBOXES)? {
            state.mailboxes.insert(mailbox.id, mailbox);
        }
        for envelope in self.read_all::<Envelope>(&txn, MESSAGES)? {
            state.messages.insert(envelope.id, envelope);
        }
        for thread in self.read_all::<Thread>(&txn, THREADS)? {
            state.threads.insert(thread.id, thread);
        }
        let mut ops = self.read_all::<PendingOp>(&txn, OUTBOX)?;
        // Keys are random-looking IDs; the queue order is the creation order.
        ops.sort_by_key(|op| op.created_ms);
        for op in ops {
            state.outbox.insert(op.id, op);
        }
        let mut drafts = self.read_all::<Draft>(&txn, DRAFTS)?;
        drafts.sort_by_key(|draft| draft.created_ms);
        for draft in drafts {
            state.drafts.insert(draft.id, draft);
        }
        match txn.open_table(SYNCED) {
            Ok(table) => {
                if let Some(row) = table.get(SYNCED_ROW).map_err(|e| self.err(e))? {
                    match serde_json::from_slice::<LocalSettings>(row.value()) {
                        Ok(local) => local.apply_to(state),
                        Err(err) => {
                            tracing::warn!(error = %err, "skipping unreadable synced settings")
                        }
                    }
                }
            }
            Err(redb::TableError::TableDoesNotExist(_)) => {}
            Err(err) => return Err(self.err(err)),
        }
        let mut approvals = self.read_all::<ApprovalRequest>(&txn, APPROVALS)?;
        approvals.sort_by_key(|approval| approval.requested_ms);
        for approval in approvals {
            state.approvals.insert(approval.id, approval);
        }
        Ok(())
    }

    fn read_all<T: DeserializeOwned>(
        &self,
        txn: &redb::ReadTransaction,
        table: TableDefinition<u128, &[u8]>,
    ) -> Result<Vec<T>, IoError> {
        let name = table.to_string();
        let table = txn.open_table(table).map_err(|e| self.err(e))?;
        let mut out = Vec::new();
        for row in table.iter().map_err(|e| self.err(e))? {
            let (key, value) = row.map_err(|e| self.err(e))?;
            match serde_json::from_slice(value.value()) {
                Ok(record) => out.push(record),
                Err(err) => tracing::warn!(
                    table = %name,
                    key = key.value(),
                    error = %err,
                    "skipping unreadable cache record"
                ),
            }
        }
        Ok(out)
    }

    /// Applies one delta in a single transaction: removals first, then upserts.
    pub fn apply_mail(&self, delta: &MailDelta) -> Result<(), IoError> {
        let txn = self.db.read().begin_write().map_err(|e| self.err(e))?;
        {
            let mut mailboxes = txn.open_table(MAILBOXES).map_err(|e| self.err(e))?;
            let mut messages = txn.open_table(MESSAGES).map_err(|e| self.err(e))?;
            let mut threads = txn.open_table(THREADS).map_err(|e| self.err(e))?;
            let mut outbox = txn.open_table(OUTBOX).map_err(|e| self.err(e))?;
            let mut drafts = txn.open_table(DRAFTS).map_err(|e| self.err(e))?;
            let mut approvals = txn.open_table(APPROVALS).map_err(|e| self.err(e))?;
            for id in &delta.removed_approvals {
                approvals
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for approval in &delta.approvals {
                let bytes = self.encode(approval)?;
                approvals
                    .insert(approval.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_drafts {
                drafts
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for draft in &delta.drafts {
                let bytes = self.encode(draft)?;
                drafts
                    .insert(draft.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_ops {
                outbox
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for op in &delta.ops {
                let bytes = self.encode(op)?;
                outbox
                    .insert(op.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_mailboxes {
                mailboxes
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_messages {
                messages
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_threads {
                threads
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for mailbox in &delta.mailboxes {
                let bytes = self.encode(mailbox)?;
                mailboxes
                    .insert(mailbox.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for envelope in &delta.messages {
                let bytes = self.encode(envelope)?;
                messages
                    .insert(envelope.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for thread in &delta.threads {
                let bytes = self.encode(thread)?;
                threads
                    .insert(thread.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
        }
        txn.commit().map_err(|e| self.err(e))
    }

    /// Stores the synced settings document and its flag (SET-4).
    pub fn put_synced(&self, local: &LocalSettings) -> Result<(), IoError> {
        let bytes = self.encode(local)?;
        let txn = self.db.read().begin_write().map_err(|e| self.err(e))?;
        txn.open_table(SYNCED)
            .map_err(|e| self.err(e))?
            .insert(SYNCED_ROW, bytes.as_slice())
            .map_err(|e| self.err(e))?;
        txn.commit().map_err(|e| self.err(e))
    }

    fn encode(&self, record: &impl Serialize) -> Result<Vec<u8>, IoError> {
        serde_json::to_vec(record).map_err(|e| self.err(e))
    }
}

/// A quarantined row: where it came from, why it did not read, and its bytes as they were
/// (rows are JSON, so text).
#[derive(Serialize, serde::Deserialize)]
struct Held {
    table: String,
    error: String,
    raw: String,
}
