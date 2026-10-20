//! A session over a fixture with its first account added and synced: the starting point of the
//! front-ends' tests and demos.

use std::sync::Arc;

use app_core::{AccountId, AppState, Command, Origin, ProviderPreset, Secret};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};

use crate::{Fixture, MemoryBackend, TestkitError};

/// The virtual clock's start: 2023-11-14 22:13 UTC, after the built-in fixtures' mail.
pub const START_MS: i64 = 1_700_000_000_000;

/// A synced session and what it was built from.
pub struct Synced {
    /// The fake server.
    pub backend: Arc<MemoryBackend>,
    /// The session (on a virtual clock starting at [`START_MS`], counter seeds from 1).
    pub session: Session,
    /// The fixture's first account, as added.
    pub account: AccountId,
}

fn session_error(err: impl std::fmt::Display) -> TestkitError {
    TestkitError::Session(err.to_string())
}

/// Opens a session over `fixture`, adds its first account (Hostinger preset) and syncs it.
/// Must be called inside a tokio runtime.
pub async fn synced_session(fixture: Fixture) -> Result<Synced, TestkitError> {
    let email = fixture
        .accounts
        .first()
        .map(|a| a.email.clone())
        .ok_or_else(|| TestkitError::Invalid("the fixture has no account".to_owned()))?;
    let backend = Arc::new(MemoryBackend::from_fixture(fixture));
    let session = Session::open(
        Arc::clone(&backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(START_MS)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .map_err(session_error)?;
    let add = Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email,
        display_name: None,
        password: Some(Secret::new("pw")),
        imap: None,
        smtp: None,
    };
    let receipt = session
        .submit(add, Origin::Cli)
        .await
        .map_err(session_error)?
        .map_err(session_error)?;
    let account = serde_json::from_value(receipt.data["id"].clone()).map_err(session_error)?;
    session
        .submit(Command::SyncNow { account: None }, Origin::Cli)
        .await
        .map_err(session_error)?
        .map_err(session_error)?;
    session.flush().await.map_err(session_error)?;
    Ok(Synced {
        backend,
        session,
        account,
    })
}

/// The state of [`synced_session`] over `fixture`, on a runtime of its own.
pub fn synced_state(fixture: Fixture) -> Result<AppState, TestkitError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(session_error)?;
    runtime.block_on(async {
        let synced = synced_session(fixture).await?;
        let state = (*synced.session.state()).clone();
        synced.session.shutdown().await.map_err(session_error)?;
        Ok(state)
    })
}
