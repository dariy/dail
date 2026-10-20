//! Forward-only schema migrations of `state.redb`.
//!
//! `meta.schema_version` records the version a database was written with. Opening a database
//! runs every migration between its version and [`CURRENT`] in one write transaction; a version
//! newer than [`CURRENT`] is refused, never downgraded.

use redb::WriteTransaction;

/// The schema version this build writes.
pub const CURRENT: u64 = 1;

/// One migration: brings a database from version `index + 1` to `index + 2`.
pub type Migration = fn(&WriteTransaction) -> Result<(), redb::Error>;

/// Migrations in order; `MIGRATIONS[n]` upgrades version `n + 1` to `n + 2`. Version 1 is the
/// initial schema, so there are none yet.
pub const MIGRATIONS: &[Migration] = &[];

/// The migrations needed to bring version `from` to [`CURRENT`].
pub fn pending(from: u64) -> &'static [Migration] {
    let start = usize::try_from(from.saturating_sub(1)).unwrap_or(usize::MAX);
    MIGRATIONS.get(start..).unwrap_or_default()
}
