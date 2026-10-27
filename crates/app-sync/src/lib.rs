//! dail runtime: `Session`, effect executor, IMAP/SMTP, keyring and IPC attach.

pub mod audit;
pub mod autoconfig;
pub mod backend;
pub mod bodies;
pub mod bridge;
pub mod clock;
pub mod connection;
pub mod error;
pub mod executor;
pub mod fulltext;
pub mod https;
pub mod imap;
pub mod ipc;
pub mod live;
pub mod original;
pub mod outgoing;
pub mod saves;
pub mod search;
pub mod secrets;
pub mod session;
pub mod smtp;
pub mod tls;
pub mod verify;

/// Image decoding with the shared limits (`app_io::images`), for the front-ends that reach
/// `app-io` only through the runtime.
pub use app_io::images;
pub use audit::AuditLog;
pub use backend::{Backend, BoxFuture};
pub use bridge::{Bridge, Reply, Wake};
pub use clock::{Clock, CounterSeeds, RandomSeeds, SeedSource, SystemClock, VirtualClock};
pub use connection::Connection;
pub use error::SyncError;
pub use imap::{ImapConnector, ImapSession, ImapStream, NetImap};
pub use ipc::{Claim, IpcClient, IpcEndpoint, IpcServer, ProfileLock};
pub use live::LiveBackend;
pub use secrets::{KeyringSecrets, SecretStore};
pub use session::{PreviewResult, Session, SessionClient, SubmitResult};
pub use smtp::{NetSmtp, SmtpSender, Submission};
pub use verify::{AccountVerifier, NetVerifier};
