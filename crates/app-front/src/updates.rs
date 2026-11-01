//! Updates of dail itself as the front-ends show them (Wave 7 F1–F3, ADR 0011): the status line
//! and the buttons of the update panel, the badge that says a new version waits, and the
//! first-run question that turns periodic checks on or off.
//!
//! Everything reads `state.update` and `state.settings.updates`; the commands are the registry's
//! (`check_for_updates`, `download_update`, `install_update`, `skip_update_version`,
//! `set_update_policy`). Installing goes through [`crate::palette::install_update`], which asks
//! first.

use crate::i18n::Key;
use app_core::{AppState, Command, UpdateChecks, UpdateRelease, UpdateStatus};

use crate::format;
use crate::palette::{self, Flow};

/// The intervals offered for periodic checks, in hours, with their labels.
pub fn intervals() -> [(u64, &'static str); 4] {
    use crate::i18n::s;
    [
        (6, "6 h"),
        (24, s(Key::Daily)),
        (168, s(Key::Weekly)),
        (720, s(Key::Monthly)),
    ]
}

/// The first-run question (F3).
pub fn question() -> &'static str {
    crate::i18n::s(Key::UpdateQuestion)
}

/// What answering the question means, under it.
pub fn question_detail() -> &'static str {
    crate::i18n::s(Key::UpdateQuestionDetail)
}

/// A button of the update panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `check_for_updates`.
    Check,
    /// `download_update` (desktop installs only).
    Download,
    /// `install_update`, after the confirmation step.
    Install,
    /// `skip_update_version` for this version.
    Skip(String),
    /// Open the release page in the browser (a host action, no command).
    Open(String),
}

impl Action {
    /// The button's label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Check => crate::i18n::s(Key::CheckNow),
            Self::Download => crate::i18n::s(Key::Download),
            Self::Install => crate::i18n::s(Key::Install),
            Self::Skip(_) => crate::i18n::s(Key::SkipThisVersion),
            Self::Open(_) => crate::i18n::s(Key::ReleaseNotesX),
        }
    }

    /// What pressing it does: a command, the confirmation step, or `Hidden` for
    /// [`Action::Open`], which the front-end does itself.
    pub fn flow(&self, state: &AppState) -> Flow {
        match self {
            Self::Check => Flow::Run(Command::CheckForUpdates {}),
            Self::Download => Flow::Run(Command::DownloadUpdate {}),
            Self::Install => palette::install_update(state),
            Self::Skip(version) => Flow::Run(Command::SkipUpdateVersion {
                version: Some(version.clone()),
            }),
            Self::Open(_) => Flow::Hidden,
        }
    }

    /// The command it runs without a confirmation, if any (the mobile banner's buttons).
    pub fn command(&self) -> Option<Command> {
        match self {
            Self::Check => Some(Command::CheckForUpdates {}),
            Self::Download => Some(Command::DownloadUpdate {}),
            Self::Skip(version) => Some(Command::SkipUpdateVersion {
                version: Some(version.clone()),
            }),
            Self::Install | Self::Open(_) => None,
        }
    }
}

/// The update panel: About on mobile, the General settings on the desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// `dail 0.4.0`, the running build.
    pub version: String,
    /// Where an update stands, one line.
    pub status: String,
    /// When the last check finished, `checked 3h ago`, if there was one.
    pub checked: Option<String>,
    /// The buttons, in order.
    pub actions: Vec<Action>,
}

/// The running version: the runtime's report, or `fallback` (the front-end's own
/// `CARGO_PKG_VERSION`) before it.
pub fn version(state: &AppState, fallback: &str) -> String {
    let running = state
        .update
        .host
        .as_ref()
        .map_or(fallback, |h| h.version.as_str());
    format!("dail {running}")
}

/// The panel for `state` at `now_ms`. `fallback` is the front-end's version, shown while the
/// runtime has not reported its own.
pub fn panel(state: &AppState, now_ms: i64, fallback: &str) -> Panel {
    let update = &state.update;
    let can_update = update.host.is_some();
    let self_updates = update
        .host
        .as_ref()
        .is_some_and(|h| h.install.self_updates());
    let mut actions = Vec::new();
    let status = match &update.status {
        _ if !can_update => crate::i18n::s(Key::ThisBuildDoesNotCheckForUpdates).to_owned(),
        UpdateStatus::Idle => {
            actions.push(Action::Check);
            if update.last_check_ms.is_some() {
                crate::i18n::s(Key::DailIsUpToDate).to_owned()
            } else {
                crate::i18n::s(Key::NotCheckedYet).to_owned()
            }
        }
        UpdateStatus::Checking => crate::i18n::s(Key::CheckingForUpdates).to_owned(),
        UpdateStatus::Available { release } => {
            if self_updates && release.artifact.is_some() {
                actions.push(Action::Download);
            }
            actions.push(Action::Open(release.notes_url.clone()));
            actions.push(Action::Skip(release.version.clone()));
            crate::i18n::f(Key::DailIsAvailable, &[("version", &release.version)])
        }
        UpdateStatus::Downloading { release } => {
            crate::i18n::f(Key::DownloadingDail, &[("version", &release.version)])
        }
        UpdateStatus::Ready { release, .. } => {
            actions.push(Action::Install);
            actions.push(Action::Open(release.notes_url.clone()));
            actions.push(Action::Skip(release.version.clone()));
            crate::i18n::f(
                Key::DailIsDownloadedAndVerified,
                &[("version", &release.version)],
            )
        }
        UpdateStatus::Installing { release } => {
            crate::i18n::f(Key::InstallingDail, &[("version", &release.version)])
        }
        UpdateStatus::Installed { version } => crate::i18n::f(
            Key::DailIsInstalledRestartDailToUse,
            &[("version", version)],
        ),
        UpdateStatus::Failed { reason } => {
            actions.push(Action::Check);
            crate::i18n::f(Key::UpdateFailed, &[("reason", reason)])
        }
    };
    Panel {
        version: version(state, fallback),
        status,
        checked: update
            .last_check_ms
            .filter(|_| can_update)
            .map(|ms| crate::i18n::f(Key::CheckedX, &[("age", &format::age(ms, now_ms))])),
        actions,
    }
}

/// The release waiting for the user, if any (available, downloaded, or being fetched).
pub fn pending(state: &AppState) -> Option<&UpdateRelease> {
    match &state.update.status {
        UpdateStatus::Available { release }
        | UpdateStatus::Downloading { release }
        | UpdateStatus::Ready { release, .. }
        | UpdateStatus::Installing { release } => Some(release),
        _ => None,
    }
}

/// The badge text: `Update 1.2.3` while a newer version waits, `Restart to update` once one is
/// installed; `None` otherwise.
pub fn badge(state: &AppState) -> Option<String> {
    if let UpdateStatus::Installed { .. } = state.update.status {
        return Some(crate::i18n::s(Key::RestartToUpdate).to_owned());
    }
    pending(state).map(|release| crate::i18n::f(Key::Update, &[("version", &release.version)]))
}

/// Whether to ask the first-run question: checks were never chosen, this build can check, and
/// the setup is done (an account exists), so the question does not crowd the wizard.
pub fn ask_first_run(state: &AppState) -> bool {
    state.settings.updates.checks == UpdateChecks::Unset
        && state.update.host.is_some()
        && !state.accounts.is_empty()
}

/// The answer to the first-run question: periodic checks on or off.
pub fn answer(on: bool) -> Command {
    Command::SetUpdatePolicy {
        checks: Some(if on {
            UpdateChecks::On
        } else {
            UpdateChecks::Off
        }),
        interval_hours: None,
    }
}

/// The label of the current interval (`Daily`), or `every N h` for one not in [`intervals`].
pub fn interval_label(hours: u64) -> String {
    intervals().iter().find(|(h, _)| *h == hours).map_or_else(
        || crate::i18n::f(Key::EveryH, &[("n", &hours.to_string())]),
        |(_, l)| (*l).to_owned(),
    )
}
