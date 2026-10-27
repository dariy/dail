//! dail I/O: MIME parse/build, HTML→text, config (TOML), profile directories and the local `Store`.

pub mod autoconfig;
pub mod compose;
pub mod config;
pub mod document;
pub mod fulltext;
pub mod images;
pub mod mime;
pub mod original;
pub mod profile;
pub mod store;
pub mod update;

pub use config::{AccountConfig, Config, ConfigStore};
pub use profile::ProfileDirs;
pub use store::{BlobDir, Compacted, IoError, ProfileStore, RedbStore, Store, blob_id};
