//! Sending addresses (`ARCHITECTURE.md §4.2`): `set_identity` / `remove_identity` (the
//! account's aliases, names and signatures, saved with the account), `set_mailbox_identity`
//! (the default `From` of drafts written from a mailbox) and `set_draft_from`.

use crate::account::Identity;
use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::{AccountId, DraftId, MailboxId};
use crate::message::Envelope;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::draft_saves::save_timer;
use super::drafts::draft_json;
use super::sends::ensure_editable;
use super::sync::CacheChanges;
use super::{Applied, Engine, invalid, not_found};

/// The longest address accepted (RFC 5321 path limit).
const MAX_ADDRESS: usize = 320;
/// The longest signature accepted.
const MAX_SIGNATURE: usize = 4096;

/// Lower case, trimmed, `local@domain`, no spaces or controls.
fn address(email: &str) -> Result<String, CoreError> {
    let email = email.trim().to_owned();
    let well_formed = email.len() <= MAX_ADDRESS
        && email
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'))
        && !email.chars().any(|c| c.is_whitespace() || c.is_control());
    if well_formed {
        Ok(email)
    } else {
        Err(invalid(
            "email",
            "expected an address like sales@example.org",
        ))
    }
}

impl Engine {
    pub(super) fn set_identity(
        &mut self,
        account: AccountId,
        identity: Identity,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let email = address(&identity.email)?;
        let clean = |text: Option<String>| {
            text.map(|t| {
                t.chars()
                    .filter(|c| *c == '\n' || !c.is_control())
                    .collect::<String>()
            })
            .filter(|t| !t.trim().is_empty())
        };
        let identity = Identity {
            email,
            name: clean(identity.name).map(|n| n.replace('\n', " ")),
            signature: clean(identity.signature),
        };
        if identity
            .signature
            .as_ref()
            .is_some_and(|s| s.len() > MAX_SIGNATURE)
        {
            return Err(invalid(
                "signature",
                format!("at most {MAX_SIGNATURE} bytes"),
            ));
        }
        let entry = self
            .state
            .accounts
            .get_mut(&account)
            .ok_or_else(|| not_found("account", account))?;
        let at = entry
            .identities
            .iter()
            .position(|i| i.email.eq_ignore_ascii_case(&identity.email));
        let previous = at.map(|i| entry.identities[i].clone());
        match at {
            Some(i) => entry.identities[i] = identity.clone(),
            None => entry.identities.push(identity.clone()),
        }
        let inverse = previous.map_or_else(
            || Command::RemoveIdentity {
                account,
                email: identity.email.clone(),
            },
            |old| Command::SetIdentity {
                account,
                email: old.email,
                name: old.name,
                signature: old.signature,
            },
        );
        let label = format!("Set {}", identity.email);
        Ok(self.account_changed(account, identity.email, receipt, label, inverse))
    }

    pub(super) fn remove_identity(
        &mut self,
        account: AccountId,
        email: &str,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let entry = self
            .state
            .accounts
            .get_mut(&account)
            .ok_or_else(|| not_found("account", account))?;
        let at = entry
            .identities
            .iter()
            .position(|i| i.email.eq_ignore_ascii_case(email.trim()))
            .ok_or_else(|| not_found("identity", email.trim()))?;
        let removed = entry.identities.remove(at);
        let inverse = Command::SetIdentity {
            account,
            email: removed.email.clone(),
            name: removed.name,
            signature: removed.signature,
        };
        let label = format!("Remove {}", removed.email);
        Ok(self.account_changed(account, removed.email, receipt, label, inverse))
    }

    fn account_changed(
        &self,
        account: AccountId,
        email: String,
        receipt: CommandReceipt,
        label: String,
        inverse: Command,
    ) -> Applied {
        let mut outcome = Outcome::new(
            receipt
                .affecting(EntityRef::Account(account))
                .with_data(serde_json::json!({ "account": account, "email": email })),
        );
        if let Some(entry) = self.state.accounts.get(&account) {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Account(entry.clone()),
            });
        }
        Applied {
            outcome,
            undo: Some((label, vec![inverse])),
        }
    }

    pub(super) fn set_mailbox_identity(
        &mut self,
        mailbox: MailboxId,
        email: Option<String>,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let mb = self
            .state
            .mailboxes
            .get(&mailbox)
            .ok_or_else(|| not_found("mailbox", mailbox))?;
        let email = match email {
            Some(email) => Some(self.identity_of(mb.account, &email)?.email),
            None => None,
        };
        let previous = mb.identity.clone();
        if let Some(mb) = self.state.mailboxes.get_mut(&mailbox) {
            mb.identity.clone_from(&email);
        }
        let mut changes = CacheChanges::default();
        changes.mailboxes.insert(mailbox);
        let delta = self.delta(changes);
        let outcome = Outcome::new(
            receipt
                .affecting(EntityRef::Mailbox(mailbox))
                .with_data(serde_json::json!({ "mailbox": mailbox, "email": email })),
        )
        .with_effect(Effect::PersistState {
            delta: PersistDelta::Mail(delta),
        });
        Ok(Applied {
            outcome,
            undo: Some((
                "Change the mailbox's address".to_owned(),
                vec![Command::SetMailboxIdentity {
                    mailbox,
                    email: previous,
                }],
            )),
        })
    }

    pub(super) fn set_draft_from(
        &mut self,
        id: DraftId,
        email: &str,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let draft = self
            .state
            .drafts
            .get(&id)
            .ok_or_else(|| not_found("draft", id))?;
        ensure_editable(draft)?;
        let identity = self.identity_of(draft.account, email)?;
        let account = self
            .state
            .accounts
            .get(&draft.account)
            .ok_or_else(|| not_found("account", draft.account))?;
        let previous = draft
            .from
            .as_ref()
            .map_or_else(|| account.email.clone(), |from| from.email.clone());
        let from = (!identity.email.eq_ignore_ascii_case(&account.email))
            .then(|| account.from_addr(&identity));
        let mut draft = draft.clone();
        draft.from = from;
        draft.updated_ms = ctx.now_ms;
        let json = draft_json(&draft);
        self.state.drafts.insert(id, draft);
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        let outcome = self
            .draft_outcome(receipt.affecting(EntityRef::Draft(id)), json, changes)
            .with_effect(save_timer(ctx.now_ms));
        Ok(Applied {
            outcome,
            undo: Some((
                "Change the sender".to_owned(),
                vec![Command::SetDraftFrom {
                    id,
                    email: previous,
                }],
            )),
        })
    }

    /// `account`'s identity for `email`, or an error naming the field.
    fn identity_of(&self, account: AccountId, email: &str) -> Result<Identity, CoreError> {
        self.state
            .accounts
            .get(&account)
            .ok_or_else(|| not_found("account", account))?
            .identity(email)
            .ok_or_else(|| {
                invalid(
                    "email",
                    format!("{} is not an address of the account", email.trim()),
                )
            })
    }

    /// The identity a new draft of `account` is sent from: the one the answered message was
    /// addressed to, else the default of `mailbox`, else none (the account's own address).
    pub(super) fn identity_for(
        &self,
        account: AccountId,
        mailbox: Option<MailboxId>,
        original: Option<&Envelope>,
    ) -> Option<Identity> {
        let entry = self.state.accounts.get(&account)?;
        let addressed = original.and_then(|o| {
            o.to.iter()
                .chain(&o.cc)
                .find_map(|a| {
                    entry
                        .identities
                        .iter()
                        .find(|i| i.email.eq_ignore_ascii_case(&a.email))
                })
                .cloned()
        });
        addressed
            .or_else(|| {
                let mb = self.state.mailboxes.get(&mailbox?)?;
                (mb.account == account)
                    .then(|| mb.identity.as_deref().and_then(|e| entry.identity(e)))
                    .flatten()
            })
            .or_else(|| entry.identity(&entry.email))
    }
}
