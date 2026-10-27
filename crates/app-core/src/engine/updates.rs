//! Updates of dail itself (Wave 7, ADR 0011): `check_for_updates`, `download_update`,
//! `install_update`, `skip_update_version`, `set_update_policy`, their events, and the periodic
//! check, which is due `interval_hours` after the last one (from `Ctx::now_ms`).

use crate::command::Command;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::update::{
    UpdateChecks, UpdateHost, UpdateManifest, UpdatePolicy, UpdateRelease, UpdateStatus,
    artifact_for, is_newer, parse_version,
};

use super::{Applied, Engine, invalid};

impl Engine {
    fn update_receipt(&self, receipt: CommandReceipt) -> CommandReceipt {
        // reason: serializing a plain struct cannot fail; `Null` is a harmless fallback.
        let data = serde_json::to_value(&self.state.update).unwrap_or_default();
        receipt.affecting(EntityRef::Update).with_data(data)
    }

    /// Starts a check unless one, a download or an install is running. `None` if nothing was
    /// started (busy, or the runtime has not reported the running version yet).
    fn start_check(&mut self) -> Option<Effect> {
        if self.state.update.status.is_busy() {
            return None;
        }
        let running = self.state.update.host.as_ref()?.version.clone();
        self.state.update.status = UpdateStatus::Checking;
        self.state.update.next_check_ms = None;
        Some(Effect::CheckForUpdates { running })
    }

    pub(super) fn check_for_updates(
        &mut self,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if self.state.update.host.is_none() {
            return Err(invalid(
                "update",
                "this build cannot update itself (the runtime reported no version)",
            ));
        }
        let effect = self.start_check();
        let outcome = Outcome::new(self.update_receipt(receipt));
        Ok(match effect {
            Some(effect) => outcome.with_effect(effect),
            None => outcome,
        })
    }

    pub(super) fn download_update(
        &mut self,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let UpdateStatus::Available { release } = &self.state.update.status else {
            return Err(invalid(
                "update",
                "no update is available; run check_for_updates first",
            ));
        };
        let self_updates = self
            .state
            .update
            .host
            .as_ref()
            .is_some_and(|h| h.install.self_updates());
        let Some(artifact) = release.artifact.clone().filter(|_| self_updates) else {
            return Err(invalid(
                "update",
                format!(
                    "this install cannot download the update; open {}",
                    release.notes_url
                ),
            ));
        };
        let release = release.clone();
        let version = release.version.clone();
        self.state.update.status = UpdateStatus::Downloading { release };
        Ok(Outcome::new(self.update_receipt(receipt))
            .with_effect(Effect::DownloadUpdate { version, artifact }))
    }

    pub(super) fn install_update(
        &mut self,
        confirm: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !confirm {
            return Err(CoreError::ConfirmationRequired {
                command: "install_update".to_owned(),
            });
        }
        let UpdateStatus::Ready { release, path } = &self.state.update.status else {
            return Err(invalid(
                "update",
                "no update is downloaded; run download_update first",
            ));
        };
        let Some(install) = self.state.update.host.as_ref().map(|h| h.install) else {
            return Err(invalid("update", "the runtime reported no install kind"));
        };
        let effect = Effect::InstallUpdate {
            version: release.version.clone(),
            install,
            path: path.clone(),
        };
        self.state.update.status = UpdateStatus::Installing {
            release: release.clone(),
        };
        Ok(Outcome::new(self.update_receipt(receipt)).with_effect(effect))
    }

    pub(super) fn skip_update_version(
        &mut self,
        version: Option<String>,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let version = version.map(|v| v.trim().to_owned());
        if let Some(v) = &version
            && parse_version(v).is_none()
        {
            return Err(invalid("version", "expected a version like 0.2.0, or null"));
        }
        let previous = self.state.settings.updates.skipped_version.clone();
        if previous == version {
            return Ok(Applied::plain(Outcome::new(self.update_receipt(receipt))));
        }
        self.state
            .settings
            .updates
            .skipped_version
            .clone_from(&version);
        if let (Some(skipped), UpdateStatus::Available { release }) =
            (&version, &self.state.update.status)
            && release.version == *skipped
        {
            self.state.update.status = UpdateStatus::Idle;
        }
        let label = version.as_ref().map_or_else(
            || "Offer every version again".to_owned(),
            |v| format!("Skip version {v}"),
        );
        let outcome = Outcome::new(self.update_receipt(receipt).affecting(EntityRef::Settings))
            .with_effect(Effect::PersistState {
                delta: PersistDelta::Settings(self.state.settings.clone()),
            });
        Ok(Applied {
            outcome,
            undo: Some((
                label,
                vec![Command::SkipUpdateVersion { version: previous }],
            )),
        })
    }

    pub(super) fn set_update_policy(
        &mut self,
        checks: Option<UpdateChecks>,
        interval_hours: Option<u64>,
        now_ms: i64,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        if let Some(hours) = interval_hours
            && !(UpdatePolicy::MIN_INTERVAL_HOURS..=UpdatePolicy::MAX_INTERVAL_HOURS)
                .contains(&hours)
        {
            return Err(invalid("interval_hours", "expected 1 to 720 hours"));
        }
        let before = self.state.settings.updates.clone();
        let policy = &mut self.state.settings.updates;
        if let Some(checks) = checks {
            policy.checks = checks;
        }
        if let Some(hours) = interval_hours {
            policy.interval_hours = hours;
        }
        if *policy == before {
            return Ok(Applied::plain(Outcome::new(self.update_receipt(receipt))));
        }
        let mut outcome = Outcome::new(self.update_receipt(receipt).affecting(EntityRef::Settings))
            .with_effect(Effect::PersistState {
                delta: PersistDelta::Settings(self.state.settings.clone()),
            });
        outcome.effects.extend(self.update_due(now_ms));
        let inverse = Command::SetUpdatePolicy {
            checks: Some(before.checks),
            interval_hours: Some(before.interval_hours),
        };
        Ok(Applied {
            outcome,
            undo: Some(("Change update checks".to_owned(), vec![inverse])),
        })
    }

    /// The periodic check (C3): starts it when checks are on and the last one is
    /// `interval_hours` old (or there was none); otherwise schedules a tick for when it is due.
    pub(super) fn update_due(&mut self, now_ms: i64) -> Vec<Effect> {
        let policy = &self.state.settings.updates;
        if policy.checks != UpdateChecks::On || self.state.update.host.is_none() {
            return Vec::new();
        }
        let due = self
            .state
            .update
            .last_check_ms
            .map_or(now_ms, |at| at.saturating_add(policy.interval_ms()));
        if now_ms < due {
            // One tick per due time: every mail tick asks again.
            if self.state.update.next_check_ms == Some(due) {
                return Vec::new();
            }
            self.state.update.next_check_ms = Some(due);
            return vec![Effect::ScheduleTick { at_ms: due }];
        }
        self.start_check().into_iter().collect()
    }

    pub(super) fn ingest_update_host(
        &mut self,
        host: UpdateHost,
        now_ms: i64,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.update.host = Some(host);
        let effects = self.update_due(now_ms);
        let mut outcome = Outcome::new(self.update_receipt(receipt));
        outcome.effects.extend(effects);
        outcome
    }

    pub(super) fn ingest_update_checked(
        &mut self,
        manifest: UpdateManifest,
        now_ms: i64,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.update.last_check_ms = Some(now_ms);
        let running = self.state.update.host.as_ref().map(|h| h.version.as_str());
        let skipped = self.state.settings.updates.skipped_version.as_deref();
        let offered = running.is_some_and(|running| is_newer(&manifest.version, running))
            && skipped != Some(manifest.version.as_str());
        self.state.update.status = if offered {
            let artifact = self
                .state
                .update
                .host
                .as_ref()
                .and_then(|host| artifact_for(&manifest, host))
                .cloned();
            UpdateStatus::Available {
                release: UpdateRelease {
                    version: manifest.version,
                    published_ms: manifest.published_ms,
                    notes_url: manifest.notes_url,
                    artifact,
                },
            }
        } else {
            UpdateStatus::Idle
        };
        self.after_check(now_ms, receipt)
    }

    pub(super) fn ingest_update_downloaded(
        &mut self,
        version: String,
        path: String,
        receipt: CommandReceipt,
    ) -> Outcome {
        if let UpdateStatus::Downloading { release } = &self.state.update.status
            && release.version == version
        {
            self.state.update.status = UpdateStatus::Ready {
                release: release.clone(),
                path,
            };
        }
        Outcome::new(self.update_receipt(receipt))
    }

    pub(super) fn ingest_update_installed(
        &mut self,
        version: String,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.update.status = UpdateStatus::Installed { version };
        Outcome::new(self.update_receipt(receipt))
    }

    pub(super) fn ingest_update_failed(
        &mut self,
        reason: String,
        now_ms: i64,
        receipt: CommandReceipt,
    ) -> Outcome {
        let checking = matches!(self.state.update.status, UpdateStatus::Checking);
        self.state.update.status = UpdateStatus::Failed { reason };
        if checking {
            // A failed check counts as a check: the next one waits a full interval.
            self.state.update.last_check_ms = Some(now_ms);
            return self.after_check(now_ms, receipt);
        }
        Outcome::new(self.update_receipt(receipt))
    }

    /// After a check: the tick for the next periodic one.
    fn after_check(&mut self, now_ms: i64, receipt: CommandReceipt) -> Outcome {
        let effects = self.update_due(now_ms);
        let mut outcome = Outcome::new(self.update_receipt(receipt));
        outcome.effects.extend(effects);
        outcome
    }
}
