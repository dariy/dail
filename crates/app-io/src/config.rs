//! `config.toml`: settings and accounts of a profile (`ARCHITECTURE.md §6`). Never holds a secret:
//! accounts carry only the [`SecretRef`] of their keyring entry.

use std::path::{Path, PathBuf};

use app_core::{
    Account, AccountId, AccountStatus, AppState, Endpoint, PersistDelta, ProviderPreset, SecretRef,
    SettingKey, Settings, ViewPrefs,
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
        }
    }

    /// Replaces settings, accounts and view preferences of `state` with this config.
    pub fn apply_to(&self, state: &mut AppState) {
        state.settings = self.settings.clone();
        state.view.apply_prefs(&self.view);
        state.accounts = self
            .accounts
            .iter()
            .map(|account| (account.id, account.to_account()))
            .collect();
    }

    /// Applies one persisted change.
    pub fn apply_delta(&mut self, delta: &PersistDelta) {
        match delta {
            PersistDelta::Settings(settings) => self.settings = settings.clone(),
            PersistDelta::Account(account) => {
                let entry = AccountConfig::from(account);
                match self.accounts.iter_mut().find(|a| a.id == account.id) {
                    Some(existing) => *existing = entry,
                    None => self.accounts.push(entry),
                }
            }
            PersistDelta::AccountRemoved(id) => self.accounts.retain(|a| a.id != *id),
            PersistDelta::View(prefs) => self.view = prefs.clone(),
            // The mail cache lives in `state.redb`, not in the config file.
            PersistDelta::Mail(_) => {}
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

    /// Checks setting ranges and that account IDs and emails are unique.
    pub fn validate(&self) -> Result<(), String> {
        for &key in SettingKey::ALL {
            let (min, max) = key.range();
            let value = key.get(&self.settings);
            if value < min || value > max {
                return Err(format!(
                    "settings.{} = {value} is outside {min}..={max}",
                    key.as_str()
                ));
            }
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
        if matches!(delta, PersistDelta::Mail(_)) {
            return Ok(());
        }
        let mut guard = self.config.lock();
        let mut next = guard.clone().unwrap_or_default();
        next.apply_delta(delta);
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
