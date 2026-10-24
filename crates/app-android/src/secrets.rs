//! Where passwords live: on Android keyring 4's Android store (an AES key in the Keystore, the
//! ciphertext in `SharedPreferences`), set as keyring-core's default store; elsewhere the OS
//! keyring (`KeyringSecrets`), so that the host runs on the desktop.
//!
//! keyring's `v1` `Entry` (what `KeyringSecrets` uses) refuses Android, so the Android store is
//! reached through keyring-core's `Entry`. The store needs ndk-context, which `jni.rs` sets up
//! in `open` before the host opens.

use std::sync::Arc;

use app_sync::SecretStore;

use app_host::HostError;

/// The credential store of this platform.
pub fn store() -> Result<Arc<dyn SecretStore>, HostError> {
    #[cfg(target_os = "android")]
    {
        android::install()?;
        Ok(Arc::new(android::AndroidSecrets))
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(Arc::new(app_sync::KeyringSecrets))
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::sync::OnceLock;

    use app_core::{Secret, SecretRef};
    use app_sync::{SecretStore, SyncError};
    use keyring_core::{Entry, Error};

    use app_host::HostError;

    static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();

    /// Makes the Android store keyring-core's default, once.
    pub fn install() -> Result<(), HostError> {
        INSTALLED
            .get_or_init(|| {
                let store =
                    android_native_keyring_store::Store::new().map_err(|err| err.to_string())?;
                keyring_core::set_default_store(store);
                Ok(())
            })
            .clone()
            .map_err(HostError::Secrets)
    }

    /// keyring-core's default store (the Android store, after [`install`]).
    #[derive(Debug, Clone, Copy)]
    pub struct AndroidSecrets;

    fn entry(secret: &SecretRef) -> Result<Entry, SyncError> {
        Entry::new(&secret.service, &secret.user).map_err(keyring_error)
    }

    fn keyring_error(err: Error) -> SyncError {
        SyncError::Keyring(err.to_string())
    }

    impl SecretStore for AndroidSecrets {
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
