//! Desktop notifications ([`app_front::alerts`]) handed to the OS on a helper thread, so that a
//! slow notification service never stalls a frame.
//!
//! - Linux: the freedesktop `org.freedesktop.Notifications` service on the session bus (zbus,
//!   already in the tree for the keyring). The body is escaped, since servers may read it as
//!   markup (no links or remote images from mail text). A new notification of a kind replaces
//!   the last one of that kind.
//! - Windows: a toast (WinRT `ToastNotificationManager`). dail is not installed with an
//!   AppUserModelID yet (packaging, `ROADMAP.md`), so the toasts borrow PowerShell's, as
//!   unpackaged apps do; the texts are XML-escaped.
//! - Elsewhere: nothing.

use std::sync::mpsc;

use app_front::alerts::Alert;

/// Sends alerts to the desktop from a helper thread.
pub struct Notifier {
    tx: Option<mpsc::Sender<Alert>>,
}

impl Notifier {
    /// Starts the helper thread (a notifier that drops everything if it can't).
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel::<Alert>();
        let spawned = std::thread::Builder::new()
            .name("dail-notify".to_owned())
            .spawn(move || {
                let mut desktop = Desktop::new();
                for alert in rx {
                    if let Err(error) = desktop.show(&alert) {
                        tracing::warn!(%error, "desktop notification failed");
                    }
                }
            });
        match spawned {
            Ok(_) => Self { tx: Some(tx) },
            Err(err) => {
                tracing::warn!(error = %err, "no desktop notifications");
                Self { tx: None }
            }
        }
    }

    /// Shows `alert` on the desktop (in the background).
    pub fn show(&self, alert: Alert) {
        if let Some(tx) = &self.tx {
            // The thread only ends with the notifier.
            let _ = tx.send(alert);
        }
    }
}

/// `&`, `<`, `>`, `"` and `'` as entities: safe in freedesktop markup and in XML.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(target_os = "linux")]
#[derive(Default)]
struct Desktop {
    bus: Option<zbus::blocking::Connection>,
    /// The last notification of each kind, replaced by the next one.
    shown: std::collections::HashMap<&'static str, u32>,
}

#[cfg(target_os = "linux")]
impl Desktop {
    fn new() -> Self {
        Self::default()
    }

    fn show(&mut self, alert: &Alert) -> Result<(), String> {
        use app_front::alerts::AlertKind;
        use zbus::zvariant::Value;

        let bus = match &self.bus {
            Some(bus) => bus,
            None => self
                .bus
                .insert(zbus::blocking::Connection::session().map_err(|err| err.to_string())?),
        };
        let (kind, icon, category) = match alert.kind {
            AlertKind::NewMail => ("new_mail", "mail-unread", "email.arrived"),
            AlertKind::SendFailed => ("send_failed", "dialog-error", "email"),
            AlertKind::ApprovalNeeded => ("approval", "dialog-question", "email"),
        };
        let replaces = self.shown.get(kind).copied().unwrap_or(0);
        let hints: std::collections::HashMap<&str, Value<'_>> = [
            ("category", Value::from(category)),
            ("desktop-entry", Value::from("dail")),
        ]
        .into_iter()
        .collect();
        let reply = bus
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &(
                    "dail",
                    replaces,
                    icon,
                    alert.title.as_str(),
                    escape(&alert.body),
                    Vec::<&str>::new(),
                    hints,
                    -1_i32,
                ),
            )
            .map_err(|err| err.to_string())?;
        let id: u32 = reply.body().deserialize().map_err(|err| err.to_string())?;
        self.shown.insert(kind, id);
        Ok(())
    }
}

#[cfg(windows)]
struct Desktop {
    /// The toast notifier, made on first use.
    notifier: Option<windows::UI::Notifications::ToastNotifier>,
}

#[cfg(windows)]
impl Desktop {
    /// PowerShell's AppUserModelID, which unpackaged apps use for toasts.
    const APP_ID: &'static str =
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

    const fn new() -> Self {
        Self { notifier: None }
    }

    fn show(&mut self, alert: &Alert) -> Result<(), String> {
        use windows::Data::Xml::Dom::XmlDocument;
        use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
        use windows::core::HSTRING;

        let xml = format!(
            "<toast><visual><binding template=\"ToastGeneric\"><text>dail: {}</text><text>{}</text></binding></visual></toast>",
            escape(&alert.title),
            escape(&alert.body)
        );
        let document = XmlDocument::new().map_err(|err| err.to_string())?;
        document
            .LoadXml(&HSTRING::from(xml))
            .map_err(|err| err.to_string())?;
        let toast =
            ToastNotification::CreateToastNotification(&document).map_err(|err| err.to_string())?;
        let notifier = match &self.notifier {
            Some(notifier) => notifier,
            None => self.notifier.insert(
                ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(Self::APP_ID))
                    .map_err(|err| err.to_string())?,
            ),
        };
        notifier.Show(&toast).map_err(|err| err.to_string())
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
struct Desktop;

#[cfg(not(any(target_os = "linux", windows)))]
impl Desktop {
    const fn new() -> Self {
        Self
    }

    fn show(&mut self, _alert: &Alert) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_is_escaped() {
        assert_eq!(
            escape("<a href=\"x\">Tom & 'Jerry'</a>"),
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; &apos;Jerry&apos;&lt;/a&gt;"
        );
    }

    /// Shows a real notification: `DAIL_IT_NOTIFY=1 cargo test -p app-gui -- --ignored desktop`.
    #[test]
    #[ignore = "shows a notification on the desktop"]
    fn a_real_notification() {
        if std::env::var_os("DAIL_IT_NOTIFY").is_none() {
            return;
        }
        let mut desktop = Desktop::new();
        for body in [
            "first <b>not bold</b> & <a href=\"https://example.org\">no link</a>",
            "second (replaces the first)",
        ] {
            desktop
                .show(&Alert {
                    kind: app_front::alerts::AlertKind::NewMail,
                    title: "dail test".into(),
                    body: body.into(),
                })
                .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
}
