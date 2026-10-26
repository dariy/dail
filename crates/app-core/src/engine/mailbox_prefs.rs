//! `pin_mailbox` / `unpin_mailbox` / `hide_mailbox` / `show_mailbox` (UX-8): the sidebar's
//! *Favourites* and hidden folders, settings saved with the others. They name folders by
//! account and path, which survive a rebuilt cache.

use crate::command::Command;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::MailboxId;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::{Applied, Engine, not_found};

/// Which of the four commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MailboxPref {
    Pin,
    Unpin,
    Hide,
    Show,
}

impl Engine {
    pub(super) fn mailbox_pref(
        &mut self,
        id: MailboxId,
        pref: MailboxPref,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let mailbox = self
            .state
            .mailboxes
            .get(&id)
            .ok_or_else(|| not_found("mailbox", id))?;
        let key = mailbox.key();
        let path = mailbox.path.clone();
        let settings = &mut self.state.settings;
        let changed = match pref {
            MailboxPref::Pin => {
                let new = !settings.pinned_mailboxes.contains(&key);
                if new {
                    settings.pinned_mailboxes.push(key.clone());
                }
                new
            }
            MailboxPref::Unpin => {
                let before = settings.pinned_mailboxes.len();
                settings.pinned_mailboxes.retain(|k| *k != key);
                settings.pinned_mailboxes.len() != before
            }
            MailboxPref::Hide => settings.hidden_mailboxes.insert(key.clone()),
            MailboxPref::Show => settings.hidden_mailboxes.remove(&key),
        };
        let receipt = receipt
            .affecting(EntityRef::Settings)
            .affecting(EntityRef::Mailbox(id))
            .with_data(serde_json::json!({
                "mailbox": id,
                "pinned": settings.pinned_mailboxes.contains(&key),
                "hidden": settings.hidden_mailboxes.contains(&key),
                "changed": changed,
            }));
        if !changed {
            return Ok(Applied::plain(Outcome::new(receipt)));
        }
        let outcome = Outcome::new(receipt).with_effect(Effect::PersistState {
            delta: PersistDelta::Settings(self.state.settings.clone()),
        });
        let (label, inverse) = match pref {
            MailboxPref::Pin => (format!("Pin {path}"), Command::UnpinMailbox { id }),
            MailboxPref::Unpin => (format!("Unpin {path}"), Command::PinMailbox { id }),
            MailboxPref::Hide => (format!("Hide {path}"), Command::ShowMailbox { id }),
            MailboxPref::Show => (format!("Show {path}"), Command::HideMailbox { id }),
        };
        Ok(Applied {
            outcome,
            undo: Some((label, vec![inverse])),
        })
    }
}
