//! Account commands (`add_account`, `test_account`, `update_account`, `remove_account`) and the
//! events that report keyring and verification results.

use crate::account::{
    Account, AccountStatus, Endpoint, ProviderPreset, Secret, SecretRef, ServerDefault,
};
use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, Event, PersistDelta};
use crate::ids::AccountId;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::{Applied, Engine, invalid, not_found};

/// Arguments of `add_account`.
pub(super) struct NewAccount {
    pub preset: ProviderPreset,
    pub email: String,
    pub display_name: Option<String>,
    pub password: Option<Secret>,
    pub imap: Option<Endpoint>,
    pub smtp: Option<Endpoint>,
}

/// Arguments of `update_account`.
pub(super) struct AccountPatch {
    pub id: AccountId,
    pub display_name: Option<String>,
    pub imap: Option<Endpoint>,
    pub smtp: Option<Endpoint>,
}

impl Engine {
    pub(super) fn add_account(
        &mut self,
        new: NewAccount,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let email = validate_email(&new.email)?;
        if self
            .state
            .accounts
            .values()
            .any(|account| account.email.eq_ignore_ascii_case(&email))
        {
            return Err(CoreError::AlreadyExists {
                entity: "account".to_owned(),
                key: email,
            });
        }
        let profile = new.preset.profile();
        let imap = resolve_endpoint("imap", new.imap, profile.imap, new.preset)?;
        let smtp = resolve_endpoint("smtp", new.smtp, profile.smtp, new.preset)?;
        let display_name = match new.display_name {
            Some(name) => validate_display_name(&name)?,
            None => email.clone(),
        };
        if let Some(password) = &new.password
            && password.expose().is_empty()
        {
            return Err(invalid("password", "must not be empty"));
        }
        let id: AccountId = ctx.ids().next_id();
        if self.state.accounts.contains_key(&id) {
            return Err(CoreError::AlreadyExists {
                entity: "account".to_owned(),
                key: id.to_string(),
            });
        }

        let account = Account {
            id,
            display_name,
            email,
            preset: new.preset,
            imap,
            smtp,
            secret: SecretRef::for_account(id),
            status: AccountStatus::Unverified,
            identities: Vec::new(),
        };
        self.state.accounts.insert(id, account.clone());

        let label = format!("Add account {}", account.email);
        let mut outcome = Outcome::new(
            receipt
                .affecting(EntityRef::Account(id))
                .with_data(account_json(&account)),
        )
        .with_effect(Effect::PersistState {
            delta: PersistDelta::Account(account.clone()),
        });
        if let Some(value) = new.password {
            outcome = outcome.with_effect(Effect::StoreSecret {
                account: id,
                secret: account.secret,
                value,
            });
        }
        Ok(Applied {
            outcome,
            undo: Some((label, vec![Command::RemoveAccount { id, confirm: true }])),
        })
    }

    pub(super) fn test_account(
        &self,
        id: AccountId,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let account = self.account(id)?;
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::Account(id))
                .with_data(serde_json::json!({ "account": id, "verification": "started" })),
        )
        .with_effect(Effect::VerifyAccount {
            account: id,
            login: account.email.clone(),
            imap: account.imap.clone(),
            smtp: account.smtp.clone(),
            secret: account.secret.clone(),
        }))
    }

    /// `set_password`: the keyring gets the new password, then the login is tested.
    pub(super) fn set_password(
        &mut self,
        id: AccountId,
        password: Secret,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if password.expose().is_empty() {
            return Err(invalid("password", "must not be empty"));
        }
        let account = self.account(id)?.clone();
        let tested = Account {
            status: AccountStatus::Unverified,
            ..account
        };
        self.state.accounts.insert(id, tested.clone());
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::Account(id))
                .with_data(serde_json::json!({ "account": id, "verification": "started" })),
        )
        .with_effect(Effect::PersistState {
            delta: PersistDelta::Account(tested.clone()),
        })
        .with_effect(Effect::StoreSecret {
            account: id,
            secret: tested.secret.clone(),
            value: password,
        })
        .with_effect(Effect::VerifyAccount {
            account: id,
            login: tested.email.clone(),
            imap: tested.imap.clone(),
            smtp: tested.smtp.clone(),
            secret: tested.secret,
        }))
    }

    pub(super) fn update_account(
        &mut self,
        patch: AccountPatch,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        if patch.display_name.is_none() && patch.imap.is_none() && patch.smtp.is_none() {
            return Err(invalid(
                "display_name",
                "nothing to update: give display_name, imap or smtp",
            ));
        }
        let previous = self.account(patch.id)?.clone();
        let display_name = patch
            .display_name
            .as_deref()
            .map(validate_display_name)
            .transpose()?;
        if let Some(imap) = &patch.imap {
            validate_endpoint("imap", imap)?;
        }
        if let Some(smtp) = &patch.smtp {
            validate_endpoint("smtp", smtp)?;
        }

        let mut account = previous.clone();
        if let Some(name) = display_name {
            account.display_name = name;
        }
        if let Some(imap) = patch.imap.clone() {
            account.imap = imap;
        }
        if let Some(smtp) = patch.smtp.clone() {
            account.smtp = smtp;
        }
        if account.imap != previous.imap || account.smtp != previous.smtp {
            account.status = AccountStatus::Unverified;
        }
        self.state.accounts.insert(account.id, account.clone());

        let inverse = Command::UpdateAccount {
            id: patch.id,
            display_name: patch.display_name.map(|_| previous.display_name.clone()),
            imap: patch.imap.map(|_| previous.imap.clone()),
            smtp: patch.smtp.map(|_| previous.smtp.clone()),
        };
        let outcome = Outcome::new(
            receipt
                .affecting(EntityRef::Account(account.id))
                .with_data(account_json(&account)),
        )
        .with_effect(Effect::PersistState {
            delta: PersistDelta::Account(account.clone()),
        });
        Ok(Applied {
            outcome,
            undo: Some((format!("Edit account {}", account.email), vec![inverse])),
        })
    }

    pub(super) fn remove_account(
        &mut self,
        id: AccountId,
        confirm: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !confirm {
            return Err(CoreError::ConfirmationRequired {
                command: "remove_account".to_owned(),
            });
        }
        let Some(account) = self.state.accounts.shift_remove(&id) else {
            return Err(not_found("account", id));
        };
        let cache = self.forget_account_cache(id);
        let mut outcome = Outcome::new(receipt.affecting(EntityRef::Account(id)).with_data(
            serde_json::json!({
                "removed": id,
                "email": account.email,
                "undoable": false,
            }),
        ))
        .with_effect(Effect::PersistState {
            delta: PersistDelta::AccountRemoved(id),
        });
        if !cache.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(cache),
            });
        }
        Ok(outcome.with_effect(Effect::DeleteSecret {
            account: id,
            secret: account.secret,
        }))
    }

    /// Applies an account-related event. Events about accounts that have been removed in the
    /// meantime only update the global status.
    pub(super) fn ingest_account_event(
        &mut self,
        event: Event,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let at_ms = ctx.now_ms;
        let (account, status) = match event {
            Event::AccountVerified { account } => (account, AccountStatus::Verified { at_ms }),
            Event::AccountVerifyFailed { account, error } => {
                (account, AccountStatus::Failed { error, at_ms })
            }
            Event::SecretStored { .. } | Event::SecretDeleted { .. } => {
                self.state.status.keyring_error = None;
                return Outcome::new(receipt.affecting(EntityRef::Status));
            }
            Event::SecretFailed { account, error } => {
                self.state.status.keyring_error = Some(format!("account {account}: {error}"));
                (
                    account,
                    AccountStatus::Failed {
                        error: format!("keyring: {error}"),
                        at_ms,
                    },
                )
            }
            Event::Tick { .. }
            | Event::Persisted {}
            | Event::PersistFailed { .. }
            | Event::MailboxesListed { .. }
            | Event::EnvelopesFetched { .. }
            | Event::FlagsChanged { .. }
            | Event::BodyFetched { .. }
            | Event::BodyFetchFailed { .. }
            | Event::RemoteOpSucceeded { .. }
            | Event::RemoteOpFailed { .. }
            | Event::SyncFailed { .. }
            | Event::FolderManaged { .. }
            | Event::FolderFailed { .. }
            | Event::AttachmentInspected { .. }
            | Event::AttachmentFailed { .. }
            | Event::AttachmentsSaved { .. }
            | Event::MailboxWatched { .. }
            | Event::OlderFetched { .. }
            | Event::StoreCompacted { .. }
            | Event::IndexSearched { .. }
            | Event::ServersDiscovered { .. }
            | Event::DiscoveryFailed { .. }
            | Event::IndexSearchFailed { .. }
            | Event::CompactFailed { .. }
            | Event::BackfillFailed { .. }
            | Event::WatchFailed { .. }
            | Event::AttachmentsSaveFailed { .. }
            | Event::Closing {}
            | Event::DraftSaved { .. }
            | Event::DraftSaveFailed { .. }
            | Event::DraftDeleted { .. }
            | Event::DraftDeleteFailed { .. }
            | Event::MessageSent { .. }
            | Event::SendFailed { .. }
            | Event::SearchResults { .. }
            | Event::SearchFailed { .. } => {
                return Outcome::new(receipt);
            }
        };
        let receipt = match self.state.accounts.get_mut(&account) {
            Some(entry) => {
                entry.status = status;
                receipt.affecting(EntityRef::Account(account))
            }
            None => receipt,
        };
        let receipt = if self.state.status.keyring_error.is_some() {
            receipt.affecting(EntityRef::Status)
        } else {
            receipt
        };
        Outcome::new(receipt)
    }

    fn account(&self, id: AccountId) -> Result<&Account, CoreError> {
        self.state
            .accounts
            .get(&id)
            .ok_or_else(|| not_found("account", id))
    }
}

fn account_json(account: &Account) -> serde_json::Value {
    // reason: serializing a plain struct cannot fail; `Null` is a harmless fallback.
    serde_json::to_value(account).unwrap_or_default()
}

/// Trims the address and checks that it looks like `local@domain`.
pub(super) fn validate_email(email: &str) -> Result<String, CoreError> {
    let email = email.trim();
    let valid = match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && !domain.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !email.chars().any(|c| c.is_whitespace() || c.is_control())
        }
        None => false,
    };
    if valid {
        Ok(email.to_owned())
    } else {
        Err(invalid(
            "email",
            "expected an address like name@example.com",
        ))
    }
}

fn validate_display_name(name: &str) -> Result<String, CoreError> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(invalid(
            "display_name",
            "must be non-empty and contain no control characters",
        ));
    }
    Ok(name.to_owned())
}

fn validate_endpoint(field: &str, endpoint: &Endpoint) -> Result<(), CoreError> {
    let host_ok = !endpoint.host.is_empty()
        && endpoint.host.len() <= 253
        && endpoint
            .host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '[' | ']'));
    if !host_ok {
        return Err(invalid(field, "host must be a DNS name or IP address"));
    }
    if endpoint.port == 0 {
        return Err(invalid(field, "port must be between 1 and 65535"));
    }
    Ok(())
}

/// The explicit endpoint if given, else the preset's default; validated.
fn resolve_endpoint(
    field: &str,
    explicit: Option<Endpoint>,
    default: Option<ServerDefault>,
    preset: ProviderPreset,
) -> Result<Endpoint, CoreError> {
    let endpoint = explicit
        .or_else(|| default.map(ServerDefault::to_endpoint))
        .ok_or_else(|| {
            invalid(
                field,
                format!("required for the {} preset", preset.as_str()),
            )
        })?;
    validate_endpoint(field, &endpoint)?;
    Ok(endpoint)
}

impl Engine {
    /// `discover_servers`: asks the runtime (the answer is recorded when it arrives), or returns
    /// the servers already found for the address.
    pub(super) fn discover_servers(
        &self,
        email: &str,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let email = email.trim().to_owned();
        let domain = email
            .split_once('@')
            .map(|(_, d)| d.to_ascii_lowercase())
            .filter(|d| {
                d.contains('.')
                    && d.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
            })
            .ok_or_else(|| invalid("email", "expected an address like me@example.org"))?;
        // The servers already found for this address are the answer; a failure is retried.
        if let Some(crate::state::Discovery {
            email: known,
            found: Ok(found),
        }) = &self.state.status.discovery
            && *known == email
        {
            let mut data =
                serde_json::json!({ "email": email, "domain": domain, "state": "found" });
            if let (Some(data), Ok(serde_json::Value::Object(found))) =
                (data.as_object_mut(), serde_json::to_value(found))
            {
                data.extend(found);
            }
            return Ok(Outcome::new(receipt.with_data(data)));
        }
        Ok(Outcome::new(receipt.with_data(
            serde_json::json!({ "email": email, "domain": domain, "state": "started" }),
        ))
        .with_effect(Effect::DiscoverServers { email, domain }))
    }

    pub(super) fn ingest_discovery(
        &mut self,
        email: String,
        found: Result<crate::state::DiscoveredServers, String>,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.status.discovery = Some(crate::state::Discovery { email, found });
        Outcome::new(receipt.affecting(EntityRef::Status))
    }
}
