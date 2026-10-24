//! Where passwords live: on iOS keyring 4's Apple "protected data" store (the data-protection
//! Keychain), set as keyring-core's default store; elsewhere the OS keyring (`KeyringSecrets`),
//! so that the host runs on the desktop.
//!
//! keyring's `v1` `Entry` (what `KeyringSecrets` uses) refuses iOS, so the store is reached
//! through keyring-core's `Entry`, as on Android.

use std::sync::Arc;

use app_host::HostError;
use app_sync::SecretStore;

/// The credential store of this platform.
pub fn store() -> Result<Arc<dyn SecretStore>, HostError> {
    #[cfg(target_os = "ios")]
    {
        ios::install()?;
        Ok(Arc::new(ios::IosSecrets))
    }
    #[cfg(not(target_os = "ios"))]
    {
        Ok(Arc::new(app_sync::KeyringSecrets))
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use std::sync::OnceLock;

    use app_core::{Secret, SecretRef};
    use app_host::HostError;
    use app_sync::{SecretStore, SyncError};
    use keyring_core::{Entry, Error};

    static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();

    /// Makes the protected-data store keyring-core's default, once. It does not sync with
    /// iCloud: the password stays on the device.
    pub fn install() -> Result<(), HostError> {
        INSTALLED
            .get_or_init(|| {
                let store = apple_native_keyring_store::protected::Store::new()
                    .map_err(|err| err.to_string())?;
                keyring_core::set_default_store(store);
                Ok(())
            })
            .clone()
            .map_err(HostError::Secrets)
    }

    /// keyring-core's default store (the protected-data store, after [`install`]).
    #[derive(Debug, Clone, Copy)]
    pub struct IosSecrets;

    fn entry(secret: &SecretRef) -> Result<Entry, SyncError> {
        Entry::new(&secret.service, &secret.user).map_err(keyring_error)
    }

    fn keyring_error(err: Error) -> SyncError {
        SyncError::Keyring(err.to_string())
    }

    impl SecretStore for IosSecrets {
        fn set(&self, secret: &SecretRef, value: &Secret) -> Result<(), SyncError> {
            entry(secret)?
                .set_password(value.expose())
                .map_err(keyring_error)
        }

        fn get(&self, secret: &SecretRef) -> Result<Option<Secret>, SyncError> {
            match entry(secret)?.get_password() {
                Ok(value) => Ok(Some(Secret::new(value))),
                Err(Error::NoEntry) => Ok(None),
                Err(err) => Err(keyring_error(err)),
            }
        }

        fn delete(&self, secret: &SecretRef) -> Result<(), SyncError> {
            match entry(secret)?.delete_credential() {
                Ok(()) | Err(Error::NoEntry) => Ok(()),
                Err(err) => Err(keyring_error(err)),
            }
        }
    }
}
