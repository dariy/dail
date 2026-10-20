//! `dail doctor`: checks a profile and prints one JSON line per check,
//! `{"check": …, "status": "ok" | "warn" | "fail", "detail": …}`. Exit code 1 if any failed.
//!
//! With the profile's lock free, the files are checked directly (the lock is held meanwhile, so
//! nothing else opens them). With dail running on it, the running process is asked for its
//! state instead. Passwords are looked up to see that they exist; their values are never
//! printed.

use std::process::ExitCode;

use anyhow::bail;
use app_core::{AccountStatus, AppState};
use app_io::store::backup;
use app_io::{ConfigStore, ProfileDirs, RedbStore, Store as _};
use app_sync::ipc::{self, Claim};
use app_sync::secrets::{KeyringSecrets, SecretStore};
use app_sync::{Connection, tls};
use serde_json::json;

use crate::cli::{BackendKind, GlobalArgs};
use crate::output::emit;
use crate::runtime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Fail,
}

struct Report {
    pretty: bool,
    failed: bool,
}

impl Report {
    fn check(
        &mut self,
        check: &str,
        status: Status,
        detail: impl Into<String>,
    ) -> anyhow::Result<()> {
        self.failed |= status == Status::Fail;
        let status = match status {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "fail",
        };
        emit(
            self.pretty,
            &json!({ "check": check, "status": status, "detail": detail.into() }),
        )
    }
}

pub async fn doctor(global: &GlobalArgs) -> anyhow::Result<ExitCode> {
    if global.backend == BackendKind::Memory {
        bail!("`dail doctor` checks a profile on disk; drop `--backend memory`");
    }
    let Some(root) = runtime::profile_root(global)? else {
        bail!("no profile directory");
    };
    let dirs = ProfileDirs::at(&root);
    let mut report = Report {
        pretty: global.pretty,
        failed: false,
    };
    report.check(
        "version",
        Status::Ok,
        format!("dail {}", env!("CARGO_PKG_VERSION")),
    )?;
    report.check("profile", Status::Ok, root.display().to_string())?;
    match tls::client_config() {
        Ok(_) => report.check("tls", Status::Ok, "the TLS client (app-tls) is ready")?,
        Err(err) => report.check("tls", Status::Fail, err.to_string())?,
    }
    match ipc::claim(&root).await? {
        Claim::Attached(client) => {
            report.check(
                "running",
                Status::Warn,
                "dail is running on this profile: its files are not checked (close it and run \
                 `dail doctor` again); its state follows",
            )?;
            let state = Connection::Attached(client).state().await?;
            accounts(&mut report, &state, None)?;
            status(&mut report, &state)?;
        }
        Claim::Owned(lock) => {
            files(&mut report, &dirs)?;
            drop(lock);
        }
    }
    Ok(if report.failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// The files of a profile nobody has open.
fn files(report: &mut Report, dirs: &ProfileDirs) -> anyhow::Result<()> {
    let config = match ConfigStore::open(dirs.config_file()) {
        Ok(config) => config,
        Err(err) => {
            report.check(
                "config",
                Status::Fail,
                format!("{err}; fix it, or copy config.toml from a backup (below)"),
            )?;
            backups(report, dirs)?;
            return Ok(());
        }
    };
    let mut state = config.load_snapshot()?.unwrap_or_default();
    report.check(
        "config",
        Status::Ok,
        format!(
            "{}: {} account(s)",
            dirs.config_file().display(),
            state.accounts.len()
        ),
    )?;
    if dirs.state_db().is_file() {
        match RedbStore::open(dirs.state_db()) {
            Ok(cache) => {
                match cache.check_integrity() {
                    Ok(true) => {}
                    Ok(false) => report.check(
                        "store",
                        Status::Warn,
                        "state.redb had damage that redb repaired",
                    )?,
                    Err(err) => report.check("store", Status::Fail, err.to_string())?,
                }
                match cache.load_into(&mut state) {
                    Ok(()) => report.check(
                        "store",
                        Status::Ok,
                        format!(
                            "{}: {} mailboxes, {} messages, {} drafts, {} pending changes",
                            dirs.state_db().display(),
                            state.mailboxes.len(),
                            state.messages.len(),
                            state.drafts.len(),
                            state.outbox.len()
                        ),
                    )?,
                    Err(err) => report.check("store", Status::Fail, err.to_string())?,
                }
            }
            Err(err) => report.check(
                "store",
                Status::Fail,
                format!("{err}; the next start sets it aside and restores the newest backup"),
            )?,
        }
    } else {
        report.check(
            "store",
            Status::Ok,
            "no state.redb yet (made on the first sync)",
        )?;
    }
    blobs(report, dirs)?;
    let index = dirs.index_dir();
    if index.is_dir() {
        match app_io::fulltext::FullText::open(&index) {
            Ok(_) => report.check("index", Status::Ok, index.display().to_string())?,
            Err(err) => report.check(
                "index",
                Status::Warn,
                format!("{err}; it is rebuilt from the downloaded mail on the next start"),
            )?,
        }
    }
    backups(report, dirs)?;
    for broken in backup::broken_files(dirs) {
        report.check(
            "broken",
            Status::Warn,
            format!(
                "{} was set aside after it did not open; delete it once you no longer need it",
                broken.display()
            ),
        )?;
    }
    accounts(report, &state, Some(&KeyringSecrets))
}

/// Downloaded messages: how many, how large, leftovers of interrupted writes.
fn blobs(report: &mut Report, dirs: &ProfileDirs) -> anyhow::Result<()> {
    let (mut count, mut bytes, mut partial) = (0_u64, 0_u64, 0_u64);
    let mut stack = vec![dirs.blobs_dir()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.to_string_lossy().ends_with(".tmp") {
                partial += 1;
            } else {
                count += 1;
                bytes += entry.metadata().map_or(0, |m| m.len());
            }
        }
    }
    let detail = format!("{count} downloaded messages, {} MiB", bytes / (1024 * 1024));
    if partial > 0 {
        report.check(
            "blobs",
            Status::Warn,
            format!("{detail}; {partial} unfinished write(s), removed by `compact_store`"),
        )
    } else {
        report.check("blobs", Status::Ok, detail)
    }
}

fn backups(report: &mut Report, dirs: &ProfileDirs) -> anyhow::Result<()> {
    let list = backup::list(dirs)?;
    match list.first() {
        Some(newest) => report.check(
            "backups",
            Status::Ok,
            format!(
                "{} backup(s), newest {} ({}); restore a file by copying it into the profile \
                 while dail is closed",
                list.len(),
                newest.name,
                newest.path.display()
            ),
        ),
        None => report.check(
            "backups",
            Status::Warn,
            format!(
                "none yet in {}: one is made each day when dail opens the profile",
                dirs.backups_dir().display()
            ),
        ),
    }
}

/// Each account's login status and, with `secrets`, whether its password is in the keyring.
fn accounts(
    report: &mut Report,
    state: &AppState,
    secrets: Option<&dyn SecretStore>,
) -> anyhow::Result<()> {
    for account in state.accounts.values() {
        let check = format!("account {}", account.email);
        match &account.status {
            AccountStatus::Failed { error, .. } => {
                report.check(&check, Status::Fail, format!("login failed: {error}"))?;
            }
            AccountStatus::Unverified => {
                report.check(&check, Status::Warn, "not tested yet (`test_account`)")?;
            }
            AccountStatus::Verified { .. } => {
                report.check(&check, Status::Ok, "login verified")?;
            }
        }
        let Some(secrets) = secrets else {
            continue;
        };
        match secrets.get(&account.secret) {
            Ok(Some(_)) => report.check(&check, Status::Ok, "password stored in the keyring")?,
            Ok(None) => report.check(
                &check,
                Status::Fail,
                format!(
                    "no password in the keyring: `dail account password --email {}`",
                    account.email
                ),
            )?,
            Err(err) => report.check(
                "keyring",
                Status::Fail,
                format!("{err}; is a keyring service running and unlocked?"),
            )?,
        }
    }
    Ok(())
}

/// Problems the running process has in its state.
fn status(report: &mut Report, state: &AppState) -> anyhow::Result<()> {
    if let Some(error) = &state.status.persist_error {
        report.check("store", Status::Fail, error.clone())?;
    }
    if let Some(error) = &state.status.keyring_error {
        report.check("keyring", Status::Fail, error.clone())?;
    }
    for (account, sync) in &state.status.sync {
        if let Some(issue) = &sync.last_error {
            let email = state
                .accounts
                .get(account)
                .map_or("account", |a| a.email.as_str());
            report.check(&format!("sync {email}"), Status::Warn, issue.error.clone())?;
        }
    }
    Ok(())
}
