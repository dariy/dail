//! dail test kit: fake IMAP/SMTP backend, in-memory store, fixtures and replay helpers.
//!
//! Used as a `[dev-dependency]` by other crates, and by `app-cli` for `--backend memory` (ADR 0001).

pub mod backend;
pub mod fixture;
pub mod remote;
pub mod secrets;
pub mod store;
pub mod synced;
pub mod updates;

pub use backend::{FAKE_SERVER_FINGERPRINT, MemoryBackend, SentMail};
pub use fixture::{
    FakeAccount, FakeMailbox, FakeMessage, Fixture, LARGE_MESSAGES, TestkitError, media,
};
pub use remote::{TestDevice, TestServer};
pub use secrets::MemorySecrets;
pub use store::MemoryStore;
pub use synced::{Synced, synced_session, synced_state};
pub use updates::FakeUpdates;
