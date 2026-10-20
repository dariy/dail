//! `set_keybinding` / `set_key_preset`: the key bindings in `Settings`, saved with the other
//! settings. The front-ends build their keymaps from them (`app_front::keymap`); the engine only
//! checks that a command exists and that keys look like keys.

use crate::command::Command;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::registry::CommandName;
use crate::state::KeyPreset;

use super::{Applied, Engine, invalid};

/// The most keys one command may have.
const MAX_KEYS: usize = 8;
/// The longest key text (`Ctrl+Shift+PageDown` fits).
const MAX_KEY_TEXT: usize = 40;

impl Engine {
    pub(super) fn set_keybinding(
        &mut self,
        command: &str,
        keys: Option<Vec<String>>,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let name = CommandName::from_name(command.trim())
            .ok_or_else(|| invalid("command", format!("no command is named {command}")))?;
        let keys = match keys {
            Some(keys) => {
                let keys: Vec<String> = keys
                    .iter()
                    .map(|k| k.split_whitespace().collect::<Vec<_>>().join(" "))
                    .collect();
                let bad = keys.iter().find(|k| {
                    k.is_empty()
                        || k.len() > MAX_KEY_TEXT
                        || k.split(' ').count() > 3
                        || k.chars().any(char::is_control)
                });
                if let Some(bad) = bad {
                    return Err(invalid("keys", format!("not a key: {bad:?}")));
                }
                if keys.len() > MAX_KEYS {
                    return Err(invalid("keys", format!("at most {MAX_KEYS} keys")));
                }
                Some(keys)
            }
            None => None,
        };
        let settings = &mut self.state.settings;
        let previous = match &keys {
            Some(keys) => settings.keys.insert(name.as_str().to_owned(), keys.clone()),
            None => settings.keys.remove(name.as_str()),
        };
        let data = serde_json::json!({ "command": name.as_str(), "keys": keys });
        Ok(self.settings_changed(
            receipt,
            data,
            format!("Keys of {}", name.spec().title),
            Command::SetKeybinding {
                command: name.as_str().to_owned(),
                keys: previous,
            },
        ))
    }

    pub(super) fn set_key_preset(&mut self, preset: KeyPreset, receipt: CommandReceipt) -> Applied {
        let previous = std::mem::replace(&mut self.state.settings.key_preset, preset);
        self.settings_changed(
            receipt,
            serde_json::json!({ "preset": preset, "previous": previous }),
            "Key preset".to_owned(),
            Command::SetKeyPreset { preset: previous },
        )
    }

    fn settings_changed(
        &self,
        receipt: CommandReceipt,
        data: serde_json::Value,
        label: String,
        inverse: Command,
    ) -> Applied {
        let outcome = Outcome::new(receipt.affecting(EntityRef::Settings).with_data(data))
            .with_effect(Effect::PersistState {
                delta: PersistDelta::Settings(self.state.settings.clone()),
            });
        Applied {
            outcome,
            undo: Some((label, vec![inverse])),
        }
    }
}
