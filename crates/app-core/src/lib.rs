//! dail core: the pure, deterministic state machine.
//!
//! `app-core` never touches the network, disk, clocks or RNG. It *describes* side effects
//! ([`Effect`]) and *ingests* their results ([`Event`]). See `ARCHITECTURE.md §4`.

pub mod account;
pub mod approval;
pub mod command;
pub mod ctx;
pub mod date;
pub mod document;
pub mod draft;
pub mod engine;
pub mod error;
pub mod event;
pub mod history;
pub mod ids;
pub mod listing;
pub mod mailbox;
pub mod message;
pub mod ops;
pub mod receipt;
pub mod registry;
pub mod replay;
pub mod search;
pub mod state;
pub mod threading;

pub use account::{
    Account, AccountStatus, Endpoint, Identity, ImapLogin, PresetProfile, ProviderPreset, Secret,
    SecretRef, Security, ServerDefault, SmtpLogin,
};
pub use approval::ApprovalRequest;
pub use command::{Command, CommandKind, SettingKey, ThemeMode};
pub use ctx::{Ctx, Origin};
pub use draft::{
    AttachmentRef, AttachmentState, Draft, OutgoingMessage, Quote, QuoteSource, QuoteStyle,
};
pub use engine::{Change, Diff, Engine, Preview};
pub use error::CoreError;
pub use event::{Effect, Event, MailDelta, PersistDelta, SentCopy};
pub use history::{History, HistoryEntry};
pub use ids::{AccountId, ApprovalId, DraftId, IdGen, MailboxId, MessageId, OpId, ThreadId};
pub use listing::ThreadSummary;
pub use mailbox::{FolderKey, Mailbox, MailboxRole, RemoteMailbox};
pub use message::{
    Addr, BodyState, Envelope, Flags, RemoteEnvelope, SNIPPET_CHARS, Thread, UidFlags, snippet,
};
pub use ops::{CopyRef, FlagKind, OpState, PendingOp, RemoteOp, RemoteStep};
pub use receipt::{CommandReceipt, EntityRef, Outcome};
pub use registry::{AgentAccess, Category, CommandName, CommandSpec};
pub use replay::{ReplayInput, ReplayRecord, ReplayStep, StepResult};
pub use search::{Query, SearchOrder};
pub use state::{
    AppState, AttachmentSave, Compaction, DiscoveredServers, Discovery, IndexSearch, KeyPreset,
    Pane, Revision, SaveState, ServerSearch, Settings, SettingsSection, StatusState, SyncIssue,
    SyncStatus, ViewPrefs, ViewState, WatchState,
};
