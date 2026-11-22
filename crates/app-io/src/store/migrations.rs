//! Forward-only schema migrations of `state.redb`.
//!
//! `meta.schema_version` records the version a database was written with. Opening a database
//! runs every migration between its version and [`CURRENT`] in one write transaction; a version
//! newer than [`CURRENT`] is refused, never downgraded.

use base64ct::{Base64, Encoding as _};
use redb::{ReadableTable as _, TableDefinition, WriteTransaction};
use serde::{Deserialize, Serialize};

/// The schema version this build writes.
pub const CURRENT: u64 = 2;

/// One migration: brings a database from version `index + 1` to `index + 2`.
pub type Migration = fn(&WriteTransaction) -> Result<(), redb::Error>;

/// Migrations in order; `MIGRATIONS[n]` upgrades version `n + 1` to `n + 2`. Version 1 is the
/// initial schema.
pub const MIGRATIONS: &[Migration] = &[v1_to_v2];

/// The migrations needed to bring version `from` to [`CURRENT`].
pub fn pending(from: u64) -> &'static [Migration] {
    let start = usize::try_from(from.saturating_sub(1)).unwrap_or(usize::MAX);
    MIGRATIONS.get(start..).unwrap_or_default()
}

/// A `quarantine` row as version 1 wrote it: the bytes as UTF-8 text, made lossy (U+FFFD for every
/// byte that is not UTF-8), so a binary row could not be restored.
#[derive(Deserialize)]
struct HeldV1 {
    table: String,
    error: String,
    raw: String,
}

/// A `quarantine` row since version 2: the bytes in standard base64, exact. `lossy` marks the rows
/// migrated from version 1, whose bytes went through the lossy text.
#[derive(Serialize)]
struct HeldV2 {
    table: String,
    error: String,
    bytes: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    lossy: bool,
}

/// 1 → 2: `quarantine` rows hold their bytes as base64 (`bytes`) instead of lossy text (`raw`).
/// The text of a version 1 row is kept as its UTF-8 bytes, with `lossy`. A row that is neither
/// shape stays as it is (the store does not list it, as before).
fn v1_to_v2(txn: &WriteTransaction) -> Result<(), redb::Error> {
    const QUARANTINE: TableDefinition<u128, &[u8]> = TableDefinition::new("quarantine");
    let mut table = txn.open_table(QUARANTINE)?;
    let mut rewritten = Vec::new();
    for row in table.iter()? {
        let (key, value) = row?;
        let Ok(old) = serde_json::from_slice::<HeldV1>(value.value()) else {
            continue;
        };
        let new = HeldV2 {
            table: old.table,
            error: old.error,
            bytes: Base64::encode_string(old.raw.as_bytes()),
            lossy: true,
        };
        // A struct of strings and a bool always serializes.
        if let Ok(bytes) = serde_json::to_vec(&new) {
            rewritten.push((key.value(), bytes));
        }
    }
    for (key, bytes) in &rewritten {
        table.insert(*key, bytes.as_slice())?;
    }
    Ok(())
}
