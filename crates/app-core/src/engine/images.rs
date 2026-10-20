//! `show_images_from` / `hide_images_from`: the senders whose messages show their inline
//! (`cid:`) images, a setting (`Settings.images_from`) saved with the others.

use crate::command::Command;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::{Applied, Engine, invalid};

/// The longest address accepted (RFC 5321 path limit).
const MAX_ADDRESS: usize = 320;

impl Engine {
    pub(super) fn images_from(
        &mut self,
        sender: &str,
        show: bool,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let sender = sender_key(sender)?;
        let senders = &mut self.state.settings.images_from;
        let changed = if show {
            senders.insert(sender.clone())
        } else {
            senders.remove(&sender)
        };
        let receipt = receipt
            .affecting(EntityRef::Settings)
            .with_data(serde_json::json!({
                "sender": sender,
                "showing": show,
                "changed": changed,
            }));
        if !changed {
            // Nothing to persist or undo.
            return Ok(Applied::plain(Outcome::new(receipt)));
        }
        let outcome = Outcome::new(receipt).with_effect(Effect::PersistState {
            delta: PersistDelta::Settings(self.state.settings.clone()),
        });
        let (label, inverse) = if show {
            (
                format!("Show images from {sender}"),
                Command::HideImagesFrom { sender },
            )
        } else {
            (
                format!("Hide images from {sender}"),
                Command::ShowImagesFrom { sender },
            )
        };
        Ok(Applied {
            outcome,
            undo: Some((label, vec![inverse])),
        })
    }
}

/// The address as kept in the setting: trimmed and in lower case. It must look like an address
/// (`local@domain`, no spaces or control characters).
pub fn sender_key(sender: &str) -> Result<String, CoreError> {
    let sender = sender.trim().to_lowercase();
    let well_formed = sender.len() <= MAX_ADDRESS
        && sender
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty())
        && !sender.chars().any(|c| c.is_whitespace() || c.is_control());
    if well_formed {
        Ok(sender)
    } else {
        Err(invalid(
            "sender",
            "expected an email address like news@example.org",
        ))
    }
}
