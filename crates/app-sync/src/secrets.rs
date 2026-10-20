//! Password storage: the OS keyring in production, anything implementing [`SecretStore`] in tests.

use app_core::{Secret, SecretRef};

use crate::error::SyncError;

/// A credential store. Calls may block (D-Bus, Windows Credential Manager); async code runs them
/// on a blocking thread.
pub trait SecretStore: Send + Sync + 'static {
    /// Stores `value` under `secret`, replacing any previous value.
    fn set(&self, secret: &SecretRef, value: &Secret) -> Result<(), SyncError>;
    /// Reads the value under `secret`; `None` if there is no entry.
    fn get(&self, secret: &SecretRef) -> Result<Option<Secret>, SyncError>;
    /// Deletes the entry. Deleting a missing entry succeeds.
    fn delete(&self, secret: &SecretRef) -> Result<(), SyncError>;
}

/// The OS keyring: Secret Service (Linux), Credential Manager (Windows), Keychain (macOS).
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyringSecrets;

fn entry(secret: &SecretRef) -> Result<keyring::Entry, SyncError> {
    keyring::Entry::new(&secret.service, &secret.user).map_err(keyring_error)
}

fn keyring_error(err: keyring::Error) -> SyncError {
    SyncError::Keyring(err.to_string())
}

impl SecretStore for KeyringSecrets {
    fn set(&self, secret: &SecretRef, value: &Secret) -> Result<(), SyncError> {
        entry(secret)?
            .set_password(value.expose())
            .map_err(keyring_error)
    }

    fn get(&self, secret: &SecretRef) -> Result<Option<Secret>, SyncError> {
        match entry(secret)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(keyring_error(err)),
        }
    }

    fn delete(&self, secret: &SecretRef) -> Result<(), SyncError> {
        match entry(secret)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(keyring_error(err)),
        }
    }
}
