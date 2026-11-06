//! The settings stored on the mail server (ADR 0015, SET-3), as the settings screens show
//! them: which account holds them, and where the last pull or push stands, with its error.

use app_core::{AccountId, AppState};

use crate::i18n::{Key, f, s};

/// The rows of the "Settings on the server" group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// The settings account.
    pub account: AccountId,
    /// Its address; `(first account)` follows it when none was chosen (or the chosen one is gone).
    pub account_label: String,
    /// `reading from the server…`, `up to date`, `not read yet`, or the error with its code.
    pub status: String,
    /// Whether `status` is an error.
    pub failed: bool,
}

/// The panel for `state`; `None` without accounts.
pub fn panel(state: &AppState) -> Option<Panel> {
    let chosen = state
        .settings
        .settings_account
        .filter(|id| state.accounts.contains_key(id));
    let account = chosen.or_else(|| state.accounts.keys().next().copied())?;
    let email = state
        .accounts
        .get(&account)
        .map(|a| crate::text::clean(&a.email))
        .unwrap_or_default();
    let account_label = if chosen.is_some() {
        email
    } else {
        f(Key::SettingsAccountFirst, &[("email", &email)])
    };
    let synced = &state.synced;
    let (status, failed) = if synced.pulling {
        (s(Key::SettingsReading).to_owned(), false)
    } else if synced.pushing {
        (s(Key::SettingsWriting).to_owned(), false)
    } else if let Some(issue) = &synced.error {
        (crate::text::clean(&issue.error), true)
    } else if synced.pulled_ms.is_some() {
        (s(Key::SettingsUpToDate).to_owned(), false)
    } else {
        (s(Key::SettingsNotRead).to_owned(), false)
    };
    Some(Panel {
        account,
        account_label,
        status,
        failed,
    })
}
