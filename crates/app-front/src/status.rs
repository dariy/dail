//! Status projections: sync state and problems, read from `AppState.status`, the accounts and the
//! outbox. Remote failures are state (`ARCHITECTURE.md §10`), so a status bar is just their
//! projection.

use app_core::{AccountId, AccountStatus, AppState, OpState, ProviderPreset};

use crate::format;
use crate::i18n::{Key, Lang, tr};

/// One problem to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Short text for the bar.
    pub text: String,
    /// The full diagnostic, shown on hover.
    pub detail: String,
    /// What it probably means and what to do, if the diagnostic says.
    pub hint: Option<String>,
    /// The action that may fix it.
    pub fix: Option<Fix>,
}

/// An action a front-end offers next to a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    /// Enter the account's password again (`set_password`, from a password form).
    Password(AccountId),
    /// Sync the account again (`sync_now`).
    Retry(AccountId),
    /// Test the account's login again (`test_account`).
    Test(AccountId),
}

impl Fix {
    /// The button's label.
    pub fn label(self, lang: Lang) -> String {
        let key = match self {
            Self::Password(_) => Key::FixPassword,
            Self::Retry(_) => Key::FixRetry,
            Self::Test(_) => Key::FixTest,
        };
        tr(lang, key, &[])
    }
}

/// What a diagnostic from a server, the network or the keyring is about. The texts come from
/// `app-sync` (`IMAP host:port: …`, `IMAP LOGIN failed: …`, lettre's SMTP replies) and servers
/// word them freely, so this is a best guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The login was refused, or no password is stored.
    Password,
    /// The mailbox is full.
    Quota,
    /// The server and dail share no TLS ciphers.
    TlsCiphers,
    /// The server's certificate is not trusted.
    Certificate,
    /// The server could not be reached.
    Network,
    /// Anything else.
    Other,
}

/// The [`Cause`] of `error`.
pub fn cause(error: &str) -> Cause {
    let lower = error.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&["tls ciphers dail implements"]) {
        Cause::TlsCiphers
    } else if has(&["certificate", "unknownissuer", "notvalidforname"]) {
        Cause::Certificate
    } else if has(&[
        "login failed",
        "authenticationfailed",
        "authentication failed",
        "invalid credentials",
        "username and password not accepted",
        "application-specific password",
        "(535)",
        "(534)",
        "535 5.7",
        "535-5.7",
        "no password stored",
    ]) {
        Cause::Password
    } else if has(&[
        "overquota",
        "quota",
        "mailbox full",
        "mailbox is full",
        "insufficient storage",
        "storage allocation",
    ]) {
        Cause::Quota
    } else if has(&[
        "connect failed",
        "timed out",
        "unreachable",
        "connection refused",
        "connection reset",
        "failed to lookup",
        "name or service not known",
        "no such host",
        "broken pipe",
    ]) {
        Cause::Network
    } else {
        Cause::Other
    }
}

/// What `cause` means for an account of `preset`, and what to do.
pub const fn hint_key(cause: Cause, preset: ProviderPreset) -> Option<Key> {
    Some(match (cause, preset) {
        (Cause::Password, ProviderPreset::Gmail) => Key::HintPasswordGmail,
        (Cause::Password, _) => Key::HintPassword,
        (Cause::Quota, _) => Key::HintQuota,
        (Cause::TlsCiphers, _) => Key::HintTlsCiphers,
        (Cause::Certificate, _) => Key::HintCertificate,
        (Cause::Network, _) => Key::HintNetwork,
        (Cause::Other, _) => return None,
    })
}

/// [`hint_key`] in `lang`.
pub fn hint(cause: Cause, preset: ProviderPreset, lang: Lang) -> Option<String> {
    hint_key(cause, preset).map(|key| tr(lang, key, &[]))
}

/// The hint for a failed send of a draft from `account`.
pub fn send_hint(state: &AppState, account: AccountId, error: &str, lang: Lang) -> Option<String> {
    let preset = state
        .accounts
        .get(&account)
        .map_or(ProviderPreset::Custom, |a| a.preset);
    hint(cause(error), preset, lang)
}

/// The problems in `state`, most important first: store and keyring failures, failed logins,
/// failed syncs.
pub fn problems(state: &AppState, lang: Lang) -> Vec<Problem> {
    let mut out = Vec::new();
    if let Some(error) = &state.status.persist_error {
        out.push(Problem {
            text: tr(lang, Key::StoreFailed, &[]),
            detail: error.clone(),
            hint: None,
            fix: None,
        });
    }
    if let Some(error) = &state.status.keyring_error {
        out.push(Problem {
            text: tr(lang, Key::KeyringFailed, &[]),
            detail: error.clone(),
            hint: Some(tr(lang, Key::KeyringHint, &[])),
            fix: None,
        });
    }
    for account in state.accounts.values() {
        if let AccountStatus::Failed { error, .. } = &account.status {
            let cause = cause(error);
            let fix = match cause {
                Cause::Password | Cause::Other => Fix::Password(account.id),
                _ => Fix::Test(account.id),
            };
            out.push(Problem {
                text: tr(lang, Key::LoginFailed, &[("email", &account.email)]),
                detail: error.clone(),
                hint: hint(cause, account.preset, lang),
                fix: Some(fix),
            });
        }
    }
    for (id, sync) in &state.status.sync {
        if let Some(issue) = &sync.last_error {
            let account = state.accounts.get(id);
            let email = account.map_or("account", |a| a.email.as_str());
            let preset = account.map_or(ProviderPreset::Custom, |a| a.preset);
            let cause = cause(&issue.error);
            let fix = if cause == Cause::Password {
                Fix::Password(*id)
            } else {
                Fix::Retry(*id)
            };
            out.push(Problem {
                text: tr(lang, Key::SyncFailed, &[("email", email)]),
                detail: issue.error.clone(),
                hint: hint(cause, preset, lang),
                fix: Some(fix),
            });
        }
    }
    out
}

/// The sync summary: `syncing…`, `synced 2m ago · next in 1m` or `not synced`.
pub fn sync_text(state: &AppState, now_ms: i64, lang: Lang) -> String {
    if state
        .status
        .sync
        .values()
        .any(app_core::SyncStatus::is_syncing)
    {
        return tr(lang, Key::Syncing, &[]);
    }
    let last = state
        .accounts
        .keys()
        .map(|id| state.status.sync.get(id).and_then(|s| s.last_sync_ms))
        .collect::<Option<Vec<i64>>>()
        .and_then(|times| times.into_iter().min());
    match last {
        Some(at) => {
            let synced = tr(lang, Key::Synced, &[("age", &format::age(at, now_ms))]);
            match state.status.next_poll_ms {
                Some(next) if next > now_ms => {
                    let when = format::until(next, now_ms);
                    format!("{synced} · {}", tr(lang, Key::NextPoll, &[("when", &when)]))
                }
                _ => synced,
            }
        }
        None if state.accounts.is_empty() => tr(lang, Key::NoAccounts, &[]),
        None => tr(lang, Key::NotSynced, &[]),
    }
}

/// The outbox summary: `2 pending` or `2 pending · 1 failed`, `None` when it is empty. The
/// flag says whether any op failed.
pub fn pending_text(state: &AppState, lang: Lang) -> Option<(String, bool)> {
    let pending = state.outbox.len();
    if pending == 0 {
        return None;
    }
    let failed = state
        .outbox
        .values()
        .filter(|op| op.state == OpState::Failed)
        .count();
    let (pending, failed_text) = (pending.to_string(), failed.to_string());
    Some(if failed > 0 {
        (
            tr(
                lang,
                Key::PendingFailed,
                &[("pending", &pending), ("failed", &failed_text)],
            ),
            true,
        )
    } else {
        (tr(lang, Key::Pending, &[("pending", &pending)]), false)
    })
}

/// The errors of the pending ops, for a tooltip or a detail view.
pub fn pending_errors(state: &AppState) -> Vec<String> {
    state
        .outbox
        .values()
        .filter_map(|op| op.last_error.clone())
        .collect()
}

/// Whether a server search round is running.
pub fn searching(state: &AppState) -> bool {
    state.status.searches.iter().any(|s| s.pending > 0)
}

/// Whether the user can start a sync now (there are accounts and none is syncing).
pub fn can_sync(state: &AppState) -> bool {
    !state.accounts.is_empty() && state.status.sync.values().all(|s| !s.is_syncing())
}

/// One account's sync state as the sidebar shows it next to the account's name (UX-9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountState {
    /// Nothing to report.
    Idle,
    /// A listing or fetch is in flight.
    Syncing,
    /// The login or the last sync failed: the diagnostic, cleaned, for the hover text.
    Failed(String),
    /// Changes wait for the server after failed attempts (the server can't be reached).
    Offline {
        /// Ops waiting.
        waiting: usize,
    },
}

/// The sync state of `account`: a failure first, then syncing, then waiting changes.
pub fn account_state(state: &AppState, account: AccountId) -> AccountState {
    if let Some(AccountStatus::Failed { error, .. }) =
        state.accounts.get(&account).map(|a| &a.status)
    {
        return AccountState::Failed(crate::text::clean(error));
    }
    let sync = state.status.sync.get(&account);
    if let Some(issue) = sync.and_then(|s| s.last_error.as_ref()) {
        return AccountState::Failed(crate::text::clean(&issue.error));
    }
    if sync.is_some_and(app_core::SyncStatus::is_syncing) {
        return AccountState::Syncing;
    }
    let waiting = state
        .outbox
        .values()
        .filter(|op| op.account == account && op.state == OpState::Queued && op.attempts > 0)
        .count();
    if waiting > 0 {
        AccountState::Offline { waiting }
    } else {
        AccountState::Idle
    }
}
