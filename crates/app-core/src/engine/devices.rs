//! The devices paired with a dail server (ADR 0017): `list_devices`, `start_pairing`,
//! `revoke_device`, and the runtime's `device_paired`, `device_seen` and `serve_failed`.

use serde_json::json;

use crate::account::Secret;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::Effect;
use crate::ids::DeviceId;
use crate::link::{Device, DeviceRole, PAIRING_TTL_MS, pairing_code};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::{Engine, invalid, not_found};

/// The longest device name, in characters.
pub const MAX_DEVICE_NAME_CHARS: usize = 64;

/// The trimmed `name` of a device, or `error_002_invalid_argument`.
fn device_name(name: &str) -> Result<String, CoreError> {
    let name = name.trim();
    let count = name.chars().count();
    if count == 0 || count > MAX_DEVICE_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(invalid(
            "name",
            "must be 1 to 64 characters without control characters",
        ));
    }
    Ok(name.to_owned())
}

impl Engine {
    /// `list_devices`: every paired device, in pairing order.
    pub(super) fn list_devices(&self, receipt: CommandReceipt) -> Outcome {
        let devices: Vec<&Device> = self.state.devices.values().collect();
        Outcome::new(receipt.with_data(json!({ "devices": devices })))
    }

    /// `start_pairing`: a single-use code derived from the call's seed, in the receipt only.
    pub(super) fn start_pairing(
        &self,
        role: DeviceRole,
        name: &str,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let name = device_name(name)?;
        let code = pairing_code(ctx.id_seed);
        let expires_at_ms = ctx.now_ms.saturating_add(PAIRING_TTL_MS);
        let data = json!({
            "code": code,
            "role": role,
            "name": name,
            "expires_at_ms": expires_at_ms,
            "undoable": false,
        });
        Ok(
            Outcome::new(receipt.with_data(data)).with_effect(Effect::OpenPairing {
                code: Secret::new(code),
                role,
                name,
                expires_at_ms,
            }),
        )
    }

    /// `revoke_device`: forgets the device; the server drops its token and connections.
    pub(super) fn revoke_device(
        &mut self,
        id: DeviceId,
        confirm: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !confirm {
            return Err(CoreError::ConfirmationRequired {
                command: "revoke_device".to_owned(),
            });
        }
        let Some(device) = self.state.devices.shift_remove(&id) else {
            return Err(not_found("device", id));
        };
        let data = json!({
            "revoked": id,
            "name": device.name,
            "undoable": false,
        });
        Ok(
            Outcome::new(receipt.affecting(EntityRef::Device(id)).with_data(data))
                .with_effect(Effect::RevokeDevice { id }),
        )
    }

    /// `device_paired`: the device joins the list (replacing one with its ID) and the last
    /// pairing failure is resolved.
    pub(super) fn ingest_device_paired(
        &mut self,
        device: Device,
        receipt: CommandReceipt,
    ) -> Outcome {
        let id = device.id;
        self.state.devices.insert(id, device);
        self.state.status.serve_error = None;
        Outcome::new(
            receipt
                .affecting(EntityRef::Device(id))
                .affecting(EntityRef::Status),
        )
    }

    /// `device_seen`: records the device's last connection; an unknown device is ignored.
    pub(super) fn ingest_device_seen(
        &mut self,
        id: DeviceId,
        at_ms: i64,
        receipt: CommandReceipt,
    ) -> Outcome {
        match self.state.devices.get_mut(&id) {
            Some(device) => {
                device.last_seen_ms = Some(at_ms);
                Outcome::new(receipt.affecting(EntityRef::Device(id)))
            }
            None => Outcome::new(receipt.with_data(json!({ "ignored": true }))),
        }
    }

    /// `serve_failed`: the server could not open a code or revoke a device.
    pub(super) fn ingest_serve_failed(
        &mut self,
        error: String,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.status.serve_error = Some(error);
        Outcome::new(receipt.affecting(EntityRef::Status))
    }
}
