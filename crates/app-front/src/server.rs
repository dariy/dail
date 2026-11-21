//! The dail server as the front ends show it (ADR 0017, SRV-6): the link of a device to its
//! server (the status bar's *Online* / *Connecting…* / *Offline · 3 queued* and its details),
//! the commands the server refused after an offline period (problems), and the devices paired
//! with the server (the *Devices* settings). Projections of `state.link` and `state.devices`;
//! the actions on them are in [`crate::actions`].

use app_core::{AppState, DeviceId, DeviceRole, LinkStatus, RequestId};

use crate::format::{self, Zone};
use crate::i18n::{self, Key, Lang, tr};
use crate::text::clean;

/// How the link looks: the status dot's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Connected.
    Online,
    /// Dialing, or waiting for the first snapshot.
    Busy,
    /// Not connected.
    Offline,
}

/// The link in a status bar: `Online`, `Connecting…`, `Offline · 3 queued`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// The text.
    pub text: String,
    /// Its tone.
    pub tone: Tone,
}

/// The link's status in words.
fn status_text(status: LinkStatus, lang: Lang) -> &'static str {
    i18n::text(
        lang,
        match status {
            LinkStatus::Online => Key::LinkOnline,
            LinkStatus::Connecting => Key::LinkConnecting,
            LinkStatus::Offline => Key::LinkOffline,
        },
    )
}

/// The status bar's link text; `None` when this profile is not linked to a server.
pub fn summary(state: &AppState, lang: Lang) -> Option<Summary> {
    let link = state.link.as_ref()?;
    let status = status_text(link.status, lang);
    let text = match link.queued.len() {
        0 => status.to_owned(),
        n => tr(
            lang,
            Key::LinkQueued,
            &[("status", status), ("n", &n.to_string())],
        ),
    };
    let tone = match link.status {
        LinkStatus::Online => Tone::Online,
        LinkStatus::Connecting => Tone::Busy,
        LinkStatus::Offline => Tone::Offline,
    };
    Some(Summary { text, tone })
}

/// One line of the link's details: a label and its value (one or more lines).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The label.
    pub label: String,
    /// The value, a line each (the fingerprint takes two).
    pub lines: Vec<String>,
}

fn field(lang: Lang, label: Key, lines: Vec<String>) -> Field {
    Field {
        label: i18n::text(lang, label).to_owned(),
        lines,
    }
}

/// The link's details, for the status bar's popover and the settings: the server, the
/// connection (with the queue), the pinned certificate (or the one offered while not paired),
/// the mirror's revision and the last error. Empty when this profile is not linked.
pub fn details(state: &AppState, lang: Lang) -> Vec<Field> {
    let Some(link) = state.link.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(server) = &link.server {
        out.push(field(
            lang,
            Key::ConnectServerLabel,
            vec![clean(&format!("{}:{}", server.host, server.port))],
        ));
    }
    let mut connection = summary(state, lang).map(|s| s.text).unwrap_or_default();
    if !link.server.as_ref().is_some_and(|s| s.is_pinned()) {
        connection = format!("{connection} · {}", i18n::text(lang, Key::LinkNotPaired));
    }
    out.push(field(lang, Key::LinkStatusLabel, vec![connection]));
    if let Some(fingerprint) = link
        .server
        .as_ref()
        .and_then(|s| s.fingerprint.as_ref().or(s.offered.as_ref()))
    {
        out.push(field(
            lang,
            Key::PairingFingerprint,
            crate::connect::fingerprint_lines(fingerprint),
        ));
    }
    out.push(field(
        lang,
        Key::LinkRevision,
        vec![link.revision.to_string()],
    ));
    if !link.queued.is_empty() {
        out.push(field(
            lang,
            Key::LinkQueuedLabel,
            vec![link.queued.len().to_string()],
        ));
    }
    if let Some(error) = &link.error {
        out.push(field(
            lang,
            Key::SettingsLastError,
            vec![clean(&i18n::message_in(lang, error))],
        ));
    }
    out
}

/// A command the server refused after an offline period, as the problems list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    /// The refused submission (for `dismiss_offline_rejection`).
    pub id: RequestId,
    /// `Refused by the server: Archive` (the command's registry title).
    pub text: String,
    /// The server's error, translated (the code kept), cleaned.
    pub error: String,
    /// When it was refused: `refused 09:41`.
    pub when: String,
}

/// The refused commands, oldest first.
pub fn rejections(state: &AppState, lang: Lang, now_ms: i64, zone: Zone) -> Vec<Rejection> {
    let Some(link) = state.link.as_ref() else {
        return Vec::new();
    };
    link.rejected
        .iter()
        .map(|r| Rejection {
            id: r.request_id,
            text: rejection_text(r, lang),
            error: clean(&i18n::message_in(lang, &r.error)),
            when: tr(
                lang,
                Key::RefusedAt,
                &[("when", &format::short_date(r.at_ms, now_ms, zone))],
            ),
        })
        .collect()
}

/// `Refused by the server: <title>` for `rejection`.
pub fn rejection_text(rejection: &app_core::OfflineRejection, lang: Lang) -> String {
    tr(
        lang,
        Key::RefusedOffline,
        &[(
            "command",
            i18n::title_in(lang, rejection.command.command_name()),
        )],
    )
}

/// A device paired with the server, as the *Devices* settings list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    /// The device (for `revoke_device`).
    pub id: DeviceId,
    /// Its name, cleaned.
    pub name: String,
    /// Its role in words.
    pub role: String,
    /// `paired Nov 14`.
    pub paired: String,
    /// `last seen 5m ago`, or `never connected`.
    pub seen: String,
}

/// The role's name in the lists.
pub fn role_text(role: DeviceRole, lang: Lang) -> &'static str {
    i18n::text(
        lang,
        match role {
            DeviceRole::User => Key::DeviceOfYours,
            DeviceRole::Agent => Key::AnAgent,
        },
    )
}

/// The paired devices, in pairing order. They come with the server's state (a mirror gets them
/// in its diffs), so the list follows without polling.
pub fn devices(state: &AppState, lang: Lang, now_ms: i64, zone: Zone) -> Vec<DeviceRow> {
    state
        .devices
        .values()
        .map(|d| DeviceRow {
            id: d.id,
            name: clean(&d.name),
            role: role_text(d.role, lang).to_owned(),
            paired: tr(
                lang,
                Key::DevicePaired,
                &[("date", &format::short_date(d.paired_at_ms, now_ms, zone))],
            ),
            seen: d.last_seen_ms.map_or_else(
                || i18n::text(lang, Key::DeviceNeverSeen).to_owned(),
                |at| tr(lang, Key::DeviceSeen, &[("age", &format::age(at, now_ms))]),
            ),
        })
        .collect()
}

/// Whether the settings show a *Devices* section: devices are paired with the server, or this
/// front end can pair one (it runs on the server, or mirrors one).
pub fn shows_devices(state: &AppState) -> bool {
    !state.devices.is_empty() || crate::pairing::can_pair(state)
}
