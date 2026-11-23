//! `config.toml`: settings and accounts of a profile (`ARCHITECTURE.md §6`). Never holds a secret:
//! accounts carry only the [`SecretRef`] of their keyring entry.

use std::path::{Path, PathBuf};

use app_core::link::{LinkState, LinkStatus, ServerEndpoint};
use app_core::{
    Account, AccountId, AccountStatus, AppState, Device, DeviceId, DeviceRole, Endpoint,
    PersistDelta, ProviderPreset, SecretRef, SettingKey, Settings, UpdatePolicy, ViewPrefs,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::store::{IoError, Store};

/// Written at the top of every saved config file.
const HEADER: &str = "# dail configuration, written by dail. Passwords are stored in the OS keyring, \
                      never in this file.\n\n";

/// The content of `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// User preferences. Missing keys take their defaults.
    #[serde(default)]
    pub settings: Settings,
    /// View preferences: collapsed sidebar, folders, focus, list filter (`[view]`, UX-1).
    #[serde(default, skip_serializing_if = "ViewPrefs::is_default")]
    pub view: ViewPrefs,
    /// Accounts, in display order.
    #[serde(default, rename = "account")]
    pub accounts: Vec<AccountConfig>,
    /// What `dail serve` keeps (`[serve]`, ADR 0017): the paired devices. Absent unless the
    /// profile serves.
    #[serde(default, skip_serializing_if = "ServeConfig::is_empty")]
    pub serve: ServeConfig,
    /// The dail server this profile is a device of (`[remote]`, ADR 0017). A remote profile
    /// has no accounts: a config with both is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteConfig>,
}

/// The `[remote]` table of a remote profile (ADR 0017): the dail server a device connects to.
/// It is the source of truth for `state.link.server` when the profile opens; the engine writes
/// it when pairing completes and removes it with `disconnect_server`
/// (`PersistDelta::Remote`). Holds the keyring entry of the device token, never the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteConfig {
    /// The server's host name or IP address.
    pub host: String,
    /// Its port.
    pub port: u16,
    /// The pinned SHA-256 fingerprint of its certificate, 64 lower-case hex digits.
    pub fingerprint: String,
    /// The keyring entry of the device token.
    pub token: SecretRef,
}

impl RemoteConfig {
    /// The `[remote]` table of a pinned server; `None` while it is not pinned (pairing).
    pub fn from_endpoint(server: &ServerEndpoint) -> Option<Self> {
        Some(Self {
            host: server.host.clone(),
            port: server.port,
            fingerprint: server.fingerprint.clone()?,
            token: server.token.clone()?,
        })
    }

    /// The pinned server as the engine keeps it in `state.link.server`.
    pub fn to_endpoint(&self) -> ServerEndpoint {
        ServerEndpoint {
            host: self.host.clone(),
            port: self.port,
            offered: None,
            pairing: None,
            fingerprint: Some(self.fingerprint.clone()),
            token: Some(self.token.clone()),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.host.trim().is_empty() || self.port == 0 {
            return Err("remote: host must not be empty and port must not be 0".to_owned());
        }
        let hex = &self.fingerprint;
        if hex.len() != 64 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err("remote: fingerprint must be 64 lower-case hex digits".to_owned());
        }
        Ok(())
    }
}

/// The `[serve]` table of a profile that runs `dail serve` (ADR 0017).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeConfig {
    /// The paired devices (`[[serve.devices]]`), in pairing order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<ServeDevice>,
}

impl ServeConfig {
    /// Whether there is nothing to write.
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

/// One `[[serve.devices]]` row: a paired device and the SHA-256 of its token. The token itself
/// is never stored on the server; the device keeps it in its OS keyring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeDevice {
    /// The device.
    pub id: DeviceId,
    /// Its name (`start_pairing`).
    pub name: String,
    /// What it may do.
    pub role: DeviceRole,
    /// SHA-256 of the device token's 32 bytes, lower-case hex.
    pub token_sha256: String,
    /// When it was paired, in Unix milliseconds.
    pub paired_at_ms: i64,
}

impl ServeDevice {
    /// The device as the engine lists it.
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id,
            name: self.name.clone(),
            role: self.role,
            paired_at_ms: self.paired_at_ms,
            last_seen_ms: None,
        }
    }
}

/// One `[[account]]` table: an [`Account`] without its runtime status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountConfig {
    /// Account ID.
    pub id: AccountId,
    /// Name shown in the UI.
    pub display_name: String,
    /// Email address and login.
    pub email: String,
    /// Provider preset.
    pub preset: ProviderPreset,
    /// IMAP server.
    pub imap: Endpoint,
    /// SMTP submission server.
    pub smtp: Endpoint,
    /// Keyring entry of the password.
    pub secret: SecretRef,
    /// Addresses it sends from, with names and signatures (`[[account.identity]]`).
    #[serde(default, rename = "identity", skip_serializing_if = "Vec::is_empty")]
    pub identities: Vec<app_core::Identity>,
    /// ManageSieve server (`[account.managesieve]`: host, port, security), e.g. a self-hosted Dovecot on
    /// port 4190. Absent: the preset's default, if any (SET-8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managesieve: Option<Endpoint>,
    /// Certificates the user trusts for its servers (`[[account.trusted_cert]]`, ADR 0016).
    #[serde(
        default,
        rename = "trusted_cert",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub trusted_certs: Vec<app_core::TrustedCert>,
}

impl From<&Account> for AccountConfig {
    fn from(account: &Account) -> Self {
        Self {
            id: account.id,
            display_name: account.display_name.clone(),
            email: account.email.clone(),
            preset: account.preset,
            imap: account.imap.clone(),
            smtp: account.smtp.clone(),
            secret: account.secret.clone(),
            identities: account.identities.clone(),
            managesieve: account.managesieve.clone(),
            trusted_certs: account.trusted_certs.clone(),
        }
    }
}

impl AccountConfig {
    /// The account, with status `unverified` (status is not persisted).
    pub fn to_account(&self) -> Account {
        Account {
            id: self.id,
            display_name: self.display_name.clone(),
            email: self.email.clone(),
            preset: self.preset,
            imap: self.imap.clone(),
            smtp: self.smtp.clone(),
            secret: self.secret.clone(),
            status: AccountStatus::Unverified,
            identities: self.identities.clone(),
            managesieve: self.managesieve.clone(),
            trusted_certs: self.trusted_certs.clone(),
        }
    }
}

impl Config {
    /// The persisted part of `state`.
    pub fn from_state(state: &AppState) -> Self {
        Self {
            settings: state.settings.clone(),
            view: state.view.prefs(),
            accounts: state.accounts.values().map(AccountConfig::from).collect(),
            // The token hashes are not in the state; `dail serve` writes `[serve]` itself.
            serve: ServeConfig::default(),
            remote: state
                .link
                .as_ref()
                .and_then(|link| link.server.as_ref())
                .and_then(RemoteConfig::from_endpoint),
        }
    }

    /// Replaces settings, accounts, view preferences, paired devices and the link of `state`
    /// with this config. A `[remote]` table puts the profile in mirror mode (`state.link`,
    /// `connecting` until the runtime reports otherwise).
    pub fn apply_to(&self, state: &mut AppState) {
        state.settings = self.settings.clone();
        state.view.apply_prefs(&self.view);
        state.accounts = self
            .accounts
            .iter()
            .map(|account| (account.id, account.to_account()))
            .collect();
        state.devices = self
            .serve
            .devices
            .iter()
            .map(|device| (device.id, device.to_device()))
            .collect();
        state.link = self.remote.as_ref().map(|remote| LinkState {
            status: LinkStatus::Connecting,
            server: Some(remote.to_endpoint()),
            ..LinkState::default()
        });
    }

    /// Applies one persisted change.
    pub fn apply_delta(&mut self, delta: &PersistDelta) {
        match delta {
            PersistDelta::Settings(settings) => self.settings = settings.clone(),
            // A remote profile has no accounts of its own: the mirror's are the server's.
            PersistDelta::Account(_) | PersistDelta::AccountRemoved(_) if self.remote.is_some() => {
            }
            PersistDelta::Account(account) => {
                let entry = AccountConfig::from(account);
                match self.accounts.iter_mut().find(|a| a.id == account.id) {
                    Some(existing) => *existing = entry,
                    None => self.accounts.push(entry),
                }
            }
            PersistDelta::AccountRemoved(id) => self.accounts.retain(|a| a.id != *id),
            PersistDelta::View(prefs) => self.view = prefs.clone(),
            PersistDelta::Remote(server) => {
                self.remote = server.as_ref().and_then(RemoteConfig::from_endpoint);
            }
            // The mail cache, the synced settings and a mirror's offline queue live in
            // `state.redb`, not in the config file.
            PersistDelta::Mail(_) | PersistDelta::Synced(_) | PersistDelta::OfflineQueue(_) => {}
        }
    }

    /// Parses and validates a config. `path` is only used in error messages.
    pub fn parse(text: &str, path: &Path) -> Result<Self, IoError> {
        let mut config: Self = toml::from_str(text).map_err(|err| config_error(path, err))?;
        // `theme = 3` / `4` name the high-contrast themes now (ADR 0008).
        config.settings.migrate_legacy_theme();
        config.validate().map_err(|reason| IoError::Config {
            path: path.display().to_string(),
            reason,
        })?;
        if config.remote.is_some() && !config.accounts.is_empty() {
            return Err(IoError::RemoteWithAccounts {
                path: path.display().to_string(),
                accounts: config.accounts.len(),
            });
        }
        Ok(config)
    }

    /// Serializes to TOML, with a header comment.
    pub fn to_toml(&self) -> Result<String, IoError> {
        let body = toml::to_string_pretty(self).map_err(|err| IoError::Config {
            path: "config.toml".to_owned(),
            reason: err.to_string(),
        })?;
        Ok(format!("{HEADER}{body}"))
    }

    /// Reads `path`. A missing file is `Ok(None)` (fresh profile).
    pub fn load(path: &Path) -> Result<Option<Self>, IoError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path).map(Some),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(IoError::fs(path, err)),
        }
    }

    /// Writes `path` durably and atomically (`store::backup::write_atomic`).
    pub fn save(&self, path: &Path) -> Result<(), IoError> {
        let text = self.to_toml()?;
        crate::store::backup::write_atomic(path, text.as_bytes())
    }

    /// Checks setting ranges (with the update interval) and that account IDs and emails are unique.
    pub fn validate(&self) -> Result<(), String> {
        for &key in SettingKey::ALL {
            let value = key.get(&self.settings);
            if !key.accepts(value) {
                return Err(format!(
                    "settings.{} = {value} is not {}",
                    key.as_str(),
                    key.allowed()
                ));
            }
        }
        let hours = self.settings.updates.interval_hours;
        let range = UpdatePolicy::MIN_INTERVAL_HOURS..=UpdatePolicy::MAX_INTERVAL_HOURS;
        if !range.contains(&hours) {
            return Err(format!(
                "settings.updates.interval_hours = {hours} is not {} to {}",
                range.start(),
                range.end()
            ));
        }
        for (index, account) in self.accounts.iter().enumerate() {
            let earlier = &self.accounts[..index];
            if earlier.iter().any(|a| a.id == account.id) {
                return Err(format!("duplicate account id {}", account.id));
            }
            if earlier
                .iter()
                .any(|a| a.email.eq_ignore_ascii_case(&account.email))
            {
                return Err(format!("duplicate account email {}", account.email));
            }
            if account.imap.port == 0 || account.smtp.port == 0 {
                return Err(format!("account {}: port must not be 0", account.email));
            }
        }
        if let Some(remote) = &self.remote {
            remote.validate()?;
        }
        for (index, device) in self.serve.devices.iter().enumerate() {
            if self.serve.devices[..index]
                .iter()
                .any(|d| d.id == device.id)
            {
                return Err(format!("duplicate serve device id {}", device.id));
            }
            let hash = &device.token_sha256;
            if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                return Err(format!(
                    "serve device {}: token_sha256 must be 64 lower-case hex digits",
                    device.id
                ));
            }
        }
        Ok(())
    }
}

fn config_error(path: &Path, err: impl std::fmt::Display) -> IoError {
    IoError::Config {
        path: path.display().to_string(),
        reason: err.to_string(),
    }
}

/// A [`Store`] that keeps settings and accounts in `config.toml`. Other deltas arrive with the
/// mail cache (`RedbStore`, M2).
#[derive(Debug)]
pub struct ConfigStore {
    path: PathBuf,
    config: Mutex<Option<Config>>,
}

impl ConfigStore {
    /// Opens (and validates) the config at `path`. A missing file is a fresh profile; it is
    /// created on the first change.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, IoError> {
        let path = path.into();
        let config = Config::load(&path)?;
        Ok(Self {
            path,
            config: Mutex::new(config),
        })
    }

    /// The config file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the profile is a device of a dail server (`[remote]`, ADR 0017).
    pub fn is_remote(&self) -> bool {
        self.config
            .lock()
            .as_ref()
            .is_some_and(|config| config.remote.is_some())
    }

    /// The `[serve]` table (empty for a fresh profile).
    pub fn serve(&self) -> ServeConfig {
        self.config
            .lock()
            .as_ref()
            .map(|config| config.serve.clone())
            .unwrap_or_default()
    }

    /// Changes the `[serve]` table and writes the file, under the same lock as every other
    /// change, so that neither overwrites the other.
    pub fn update_serve(&self, change: impl FnOnce(&mut ServeConfig)) -> Result<(), IoError> {
        let mut guard = self.config.lock();
        let mut next = guard.clone().unwrap_or_default();
        change(&mut next.serve);
        next.validate().map_err(|reason| IoError::Config {
            path: self.path.display().to_string(),
            reason,
        })?;
        next.save(&self.path)?;
        *guard = Some(next);
        Ok(())
    }
}

impl Store for ConfigStore {
    fn load_snapshot(&self) -> Result<Option<AppState>, IoError> {
        Ok(self.config.lock().as_ref().map(|config| {
            let mut state = AppState::default();
            config.apply_to(&mut state);
            state
        }))
    }

    fn apply(&self, delta: &PersistDelta) -> Result<(), IoError> {
        if matches!(
            delta,
            PersistDelta::Mail(_) | PersistDelta::Synced(_) | PersistDelta::OfflineQueue(_)
        ) {
            return Ok(());
        }
        let mut guard = self.config.lock();
        let mut next = guard.clone().unwrap_or_default();
        next.apply_delta(delta);
        // Never write a file that would not open again.
        if next.remote.is_some() && !next.accounts.is_empty() {
            return Err(IoError::RemoteWithAccounts {
                path: self.path.display().to_string(),
                accounts: next.accounts.len(),
            });
        }
        next.save(&self.path)?;
        *guard = Some(next);
        Ok(())
    }

    fn put_blob(&self, _bytes: &[u8]) -> Result<String, IoError> {
        Err(IoError::Unavailable(
            "config.toml holds no blobs; use ProfileStore".to_owned(),
        ))
    }

    fn get_blob(&self, _id: &str) -> Result<Option<Vec<u8>>, IoError> {
        Ok(None)
    }
}
