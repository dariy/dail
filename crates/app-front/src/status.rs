//! Status projections: sync state and problems, read from `AppState.status`, the accounts and the
//! outbox. Remote failures are state (`ARCHITECTURE.md §10`), so a status bar is just their
//! projection.

use app_core::{AccountId, AccountStatus, AppState, OpState, ProviderPreset, RequestId};

use crate::format;
use crate::i18n::{self, Key, Lang, tr};

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
    /// Trust a certificate the account's server offered (`trust_certificate`, after its step
    /// shows the certificate; ADR 0016).
    Trust(AccountId),
    /// A command the dail server refused after an offline period (ADR 0017): the front end
    /// draws [`crate::actions::rejection_actions`] for it (*Retry all*, *Dismiss*).
    Refused(RequestId),
}

impl Fix {
    /// The button's label.
    pub fn label(self, lang: Lang) -> String {
        let key = match self {
            Self::Password(_) => Key::FixPassword,
            Self::Retry(_) => Key::FixRetry,
            Self::Test(_) => Key::FixTest,
            Self::Trust(_) => Key::FixTrust,
            Self::Refused(_) => Key::RetryAll,
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
/// failed syncs, then the commands a dail server refused after an offline period (oldest first).
pub fn problems(state: &AppState, lang: Lang) -> Vec<Problem> {
    let mut out = Vec::new();
    if let Some(error) = &state.status.persist_error {
        out.push(Problem {
            text: tr(lang, Key::StoreFailed, &[]),
            detail: i18n::message_in(lang, error),
            hint: None,
            fix: None,
        });
    }
    let quarantined = &state.status.quarantined;
    if !quarantined.is_empty() {
        let count = quarantined.len().to_string();
        let detail = quarantined
            .iter()
            .map(|row| format!("{} {}: {}", row.table, row.key, row.error))
            .collect::<Vec<_>>()
            .join("\n");
        out.push(Problem {
            text: tr(lang, Key::Quarantined, &[("count", &count)]),
            detail,
            hint: Some(tr(lang, Key::QuarantinedHint, &[])),
            fix: None,
        });
    }
    if let Some(error) = &state.status.keyring_error {
        out.push(Problem {
            text: tr(lang, Key::KeyringFailed, &[]),
            detail: i18n::message_in(lang, error),
            hint: Some(tr(lang, Key::KeyringHint, &[])),
            fix: None,
        });
    }
    for account in state.accounts.values() {
        if let AccountStatus::Failed { error, .. } = &account.status {
            let cause = cause(error);
            let fix = match cause {
                _ if trust_offered(state, account.id, cause) => Fix::Trust(account.id),
                Cause::Password | Cause::Other => Fix::Password(account.id),
                _ => Fix::Test(account.id),
            };
            out.push(Problem {
                text: tr(lang, Key::LoginFailed, &[("email", &account.email)]),
                detail: i18n::message_in(lang, error),
                hint: hint(cause, account.preset, lang),
                fix: Some(fix),
            });
        }
    }
    for (id, sync) in &state.status.sync {
        if let Some(issue) = &sync.last_error {
            let account = state.accounts.get(id);
            let email =
                account.map_or(crate::i18n::text(lang, Key::AccountX), |a| a.email.as_str());
            let preset = account.map_or(ProviderPreset::Custom, |a| a.preset);
            let cause = cause(&issue.error);
            let fix = if trust_offered(state, *id, cause) {
                Fix::Trust(*id)
            } else if cause == Cause::Password {
                Fix::Password(*id)
            } else {
                Fix::Retry(*id)
            };
            out.push(Problem {
                text: tr(lang, Key::SyncFailed, &[("email", email)]),
                detail: i18n::message_in(lang, &issue.error),
                hint: hint(cause, preset, lang),
                fix: Some(fix),
            });
        }
    }
    for rejection in state.link.iter().flat_map(|link| &link.rejected) {
        out.push(Problem {
            text: crate::server::rejection_text(rejection, lang),
            detail: i18n::message_in(lang, &rejection.error),
            hint: Some(tr(lang, Key::RefusedOfflineHint, &[])),
            fix: Some(Fix::Refused(rejection.request_id)),
        });
    }
    out
}

/// Whether a certificate failure of `account` can be fixed by trusting a certificate its server
/// offered.
fn trust_offered(state: &AppState, account: AccountId, cause: Cause) -> bool {
    cause == Cause::Certificate && !crate::certs::offers(state, account).is_empty()
}

/// What heavy work runs now, as a status bar shows it: the step and, where it can be counted,
/// how far it got.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    /// `Loading older mail · 1240 of 8000`.
    pub text: String,
    /// The share done, `0.0..=1.0`, when the work can be counted (the history backfill).
    pub progress: Option<f32>,
}

/// Body downloads counted as heavy work: one is the message being opened, more is the prefetch.
const BODIES_SHOWN: usize = 2;

/// The heavy work in flight, first match: an account's first contact (`Loading folders…`: it
/// syncs and has no mailbox yet), new envelopes (`Loading new mail · Inbox (+3)`), older ones
/// (`Loading older mail · 1240 of 8000`, the cached and wanted counts of the backfilling
/// mailboxes, with the progress), body prefetch (`Downloading 12 messages`); `None` when only
/// a quick check or nothing runs. A history being loaded is shown with its progress even beside
/// new mail, since it is the long part.
pub fn activity(state: &AppState, lang: Lang) -> Option<Activity> {
    let status = &state.status;
    let listing = status.sync.iter().any(|(account, sync)| {
        sync.is_syncing() && !state.mailboxes.values().any(|m| m.account == *account)
    });
    if listing {
        return Some(Activity {
            text: tr(lang, Key::ActivityFolders, &[]),
            progress: None,
        });
    }
    let older = older_progress(state);
    let fetching: Vec<&app_core::Mailbox> = status
        .fetching
        .iter()
        .filter_map(|id| state.mailboxes.get(id))
        .collect();
    if let Some(first) = fetching.first() {
        let mailbox = crate::text::clean(&crate::mailbox::mailbox_name_in(lang, first));
        let text = match fetching.len() - 1 {
            0 => tr(lang, Key::ActivityNewMail, &[("mailbox", &mailbox)]),
            more => tr(
                lang,
                Key::ActivityNewMailMore,
                &[("mailbox", &mailbox), ("more", &more.to_string())],
            ),
        };
        return Some(Activity {
            text,
            progress: older.map(|(cached, wanted)| fraction(cached, wanted)),
        });
    }
    if let Some((cached, wanted)) = older {
        return Some(Activity {
            text: tr(
                lang,
                Key::ActivityOlder,
                &[
                    ("cached", &cached.to_string()),
                    ("wanted", &wanted.to_string()),
                ],
            ),
            progress: Some(fraction(cached, wanted)),
        });
    }
    let bodies = status.bodies_in_flight.len();
    (bodies >= BODIES_SHOWN).then(|| Activity {
        text: tr(lang, Key::ActivityBodies, &[("n", &bodies.to_string())]),
        progress: None,
    })
}

/// Cached and wanted messages over the mailboxes whose history is being fetched, in one pass
/// over the messages; `None` when none is.
fn older_progress(state: &AppState) -> Option<(u64, u64)> {
    let backfilling = &state.status.backfilling;
    if backfilling.is_empty() {
        return None;
    }
    let mut cached: std::collections::BTreeMap<app_core::MailboxId, u64> =
        backfilling.iter().map(|id| (*id, 0)).collect();
    for message in state.messages.values() {
        if !message.is_placeholder()
            && let Some(n) = cached.get_mut(&message.mailbox)
        {
            *n += 1;
        }
    }
    let window = state.settings.initial_window;
    let (mut done, mut wanted) = (0_u64, 0_u64);
    for (id, n) in cached {
        let Some(mailbox) = state.mailboxes.get(&id) else {
            continue;
        };
        let window = match mailbox.sync_window {
            Some(0) => u64::MAX,
            Some(w) => u64::from(w),
            None => window,
        };
        let want = if mailbox.history_complete {
            n
        } else {
            window.min(u64::from(mailbox.total)).max(n)
        };
        done += n;
        wanted += want;
    }
    Some((done, wanted))
}

/// `done / total` within `0.0..=1.0` (a total of 0 is done).
fn fraction(done: u64, total: u64) -> f32 {
    if total == 0 {
        return 1.0;
    }
    // reason: a share for a progress bar; precision beyond f32 is not visible.
    #[allow(clippy::cast_precision_loss)]
    let share = done as f32 / total as f32;
    share.clamp(0.0, 1.0)
}

/// The sync summary: what heavy work runs ([`activity`]), else `syncing…`, `synced 2m ago ·
/// next in 1m` or `not synced`.
pub fn sync_text(state: &AppState, now_ms: i64, lang: Lang) -> String {
    if let Some(activity) = activity(state, lang) {
        return activity.text;
    }
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
        .filter_map(|op| op.last_error.as_deref().map(i18n::message))
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
        return AccountState::Failed(crate::text::clean(&i18n::message(error)));
    }
    let sync = state.status.sync.get(&account);
    if let Some(issue) = sync.and_then(|s| s.last_error.as_ref().or(s.folder_error.as_ref())) {
        return AccountState::Failed(crate::text::clean(&i18n::message(&issue.error)));
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
