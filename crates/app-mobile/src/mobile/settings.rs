//! The settings page (`open_settings`): the sections as tabs, each a list of rows whose
//! choices run `set_setting`, `set_theme` or `set_key_preset` directly. General also holds the
//! phone's *Background & notifications* group. Settings that only the desktop reads
//! (`text_scale`, `tui_colors`, `history_cap`) are not shown; the desktop and `config.toml`
//! change them.

use app_core::{
    AccountStatus, AppState, Command, CommandName, KeyPreset, SettingKey, SettingsSection,
    ThemeMode,
};
use app_front::text::clean;
use app_theme::{Appearance, builtin};

use super::{Act, Mobile};
use crate::event::{External, SystemScreen};
use crate::frame::{Button, Choice, SettingRow, SettingsGroup, SettingsPage};

/// The tabs, in order.
const TABS: [(SettingsSection, &str); 4] = [
    (SettingsSection::Appearance, "Appearance"),
    (SettingsSection::Keys, "Keyboard"),
    (SettingsSection::General, "General"),
    (SettingsSection::Accounts, "Accounts"),
];

/// The values offered for a numeric setting, with their labels.
fn presets(key: SettingKey) -> &'static [(u64, &'static str)] {
    match key {
        SettingKey::UndoSendMs => &[
            (0, "Off"),
            (5_000, "5 s"),
            (10_000, "10 s"),
            (20_000, "20 s"),
            (30_000, "30 s"),
        ],
        SettingKey::PollSecs => &[
            (60, "1 min"),
            (120, "2 min"),
            (300, "5 min"),
            (900, "15 min"),
            (3_600, "1 h"),
        ],
        SettingKey::InitialWindow => {
            &[(200, "200"), (500, "500"), (1_000, "1000"), (5_000, "5000")]
        }
        SettingKey::ListDensity => &[(0, "Cozy"), (1, "Compact"), (2, "Preview")],
        SettingKey::Notifications => &[(0, "Off"), (1, "On")],
        SettingKey::BackgroundSync => &[(0, "Only while open"), (1, "Periodic"), (2, "Push")],
        SettingKey::BackgroundIntervalMins => &[
            (15, "15 min"),
            (30, "30 min"),
            (60, "1 h"),
            (120, "2 h"),
            (360, "6 h"),
            (720, "12 h"),
        ],
        SettingKey::HistoryCap
        | SettingKey::Theme
        | SettingKey::TextScale
        | SettingKey::TuiColors
        // The phone has no mark-as-read timer yet (UX-5 is the desktop's).
        | SettingKey::MarkReadAfterMs
        | SettingKey::StaleSecs
        | SettingKey::FullSyncSecs
        | SettingKey::AfterTriage => &[],
    }
}

/// The label of a setting on the phone.
const fn label(key: SettingKey) -> &'static str {
    match key {
        SettingKey::UndoSendMs => "Undo send",
        SettingKey::PollSecs => "Check the inbox every",
        SettingKey::InitialWindow => "Messages fetched first",
        SettingKey::ListDensity => "List density",
        SettingKey::Notifications => "Notifications",
        SettingKey::BackgroundSync => "Mail checks in the background",
        SettingKey::BackgroundIntervalMins => "Check every",
        SettingKey::HistoryCap => "Undo history",
        SettingKey::Theme => "Theme",
        SettingKey::TextScale => "Text size",
        SettingKey::TuiColors => "Terminal colours",
        SettingKey::MarkReadAfterMs => "Mark as read after",
        SettingKey::StaleSecs => "Refresh a mailbox on open after",
        SettingKey::FullSyncSecs => "Sync every folder every",
        SettingKey::AfterTriage => "After archiving",
    }
}

const fn hint(key: SettingKey) -> Option<&'static str> {
    match key {
        SettingKey::UndoSendMs => Some("How long a send can still be cancelled."),
        SettingKey::PollSecs => Some("While the app is open."),
        SettingKey::BackgroundSync => Some(
            "Periodic checks wait for the phone's maintenance windows. Push keeps a silent \
             notification while it watches the inboxes.",
        ),
        SettingKey::Notifications => Some("New mail and failed sends, while dail is off screen."),
        SettingKey::InitialWindow => Some("Per mailbox, on its first sync."),
        _ => None,
    }
}

impl Mobile {
    fn choice(&mut self, label: &str, selected: bool, cmd: Command) -> Choice {
        Choice {
            label: label.to_owned(),
            selected,
            action: self.add(Act::Run(cmd)),
        }
    }

    /// A numeric setting as a row of choices (the current value added if it is none of them).
    fn setting_row(&mut self, state: &AppState, key: SettingKey) -> SettingRow {
        let current = key.get(&state.settings);
        let mut values: Vec<(u64, String)> = presets(key)
            .iter()
            .map(|(v, l)| (*v, (*l).to_owned()))
            .collect();
        if !values.iter().any(|(v, _)| *v == current) {
            values.push((current, current.to_string()));
        }
        let value = values
            .iter()
            .find(|(v, _)| *v == current)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        let choices = values
            .into_iter()
            .map(|(v, l)| {
                self.choice(
                    &l,
                    v == current,
                    Command::SetSetting {
                        key,
                        value: v.into(),
                    },
                )
            })
            .collect();
        SettingRow {
            label: label(key).to_owned(),
            value,
            hint: hint(key).map(str::to_owned),
            choices,
            actions: Vec::new(),
        }
    }

    fn link(&mut self, label: &str, act: Act) -> Button {
        Button {
            icon: None,
            label: label.to_owned(),
            action: self.add(act),
        }
    }

    /// The settings page, while open.
    pub(super) fn settings_page(&mut self, state: &AppState) -> Option<SettingsPage> {
        let section = state.view.settings?;
        let tabs = TABS
            .iter()
            .map(|(s, title)| {
                self.choice(
                    title,
                    *s == section,
                    Command::OpenSettings { section: Some(*s) },
                )
            })
            .collect();
        let groups = match section {
            SettingsSection::Appearance => self.appearance(state),
            SettingsSection::Keys => self.keyboard(state),
            SettingsSection::General => self.general(state),
            SettingsSection::Accounts => self.accounts(state),
        };
        let name = match section {
            SettingsSection::Appearance => "appearance",
            SettingsSection::Keys => "keys",
            SettingsSection::General => "general",
            SettingsSection::Accounts => "accounts",
        };
        Some(SettingsPage {
            section: name.to_owned(),
            tabs,
            groups,
        })
    }

    fn appearance(&mut self, state: &AppState) -> Vec<SettingsGroup> {
        let mut settings = state.settings.clone();
        settings.migrate_legacy_theme();
        let modes = [
            (ThemeMode::System, "System", 0),
            (ThemeMode::Light, "Light", 1),
            (ThemeMode::Dark, "Dark", 2),
        ];
        let current = modes
            .iter()
            .find(|m| m.2 == settings.theme)
            .map_or("System", |m| m.1);
        let choices = modes
            .iter()
            .map(|(mode, label, value)| {
                self.choice(
                    label,
                    *value == settings.theme,
                    Command::SetTheme {
                        mode: Some(*mode),
                        light: None,
                        dark: None,
                    },
                )
            })
            .collect();
        let mut rows = vec![SettingRow {
            label: "Mode".to_owned(),
            value: current.to_owned(),
            hint: Some("System follows Android's dark theme.".to_owned()),
            choices,
            actions: Vec::new(),
        }];
        for (appearance, label, chosen) in [
            (Appearance::Light, "Light theme", &settings.theme_light),
            (Appearance::Dark, "Dark theme", &settings.theme_dark),
        ] {
            let themes: Vec<_> = builtin::all()
                .into_iter()
                .filter(|t| t.appearance == appearance)
                .collect();
            let value = themes
                .iter()
                .find(|t| t.id == **chosen)
                .map_or_else(|| clean(chosen), |t| t.name.to_string());
            let choices = themes
                .iter()
                .map(|t| {
                    let id = t.id.to_string();
                    let (light, dark) = match appearance {
                        Appearance::Light => (Some(id), None),
                        Appearance::Dark => (None, Some(id)),
                    };
                    self.choice(
                        &t.name,
                        t.id == **chosen,
                        Command::SetTheme {
                            mode: None,
                            light,
                            dark,
                        },
                    )
                })
                .collect();
            rows.push(SettingRow {
                label: label.to_owned(),
                value,
                hint: None,
                choices,
                actions: Vec::new(),
            });
        }
        rows.push(self.setting_row(state, SettingKey::ListDensity));
        vec![SettingsGroup {
            title: "Appearance".to_owned(),
            hint: Some("Text size follows Android's font size.".to_owned()),
            rows,
        }]
    }

    fn keyboard(&mut self, state: &AppState) -> Vec<SettingsGroup> {
        let preset = state.settings.key_preset;
        let choices = [(KeyPreset::Gmail, "Gmail"), (KeyPreset::Vim, "Vim")]
            .into_iter()
            .map(|(p, label)| self.choice(label, p == preset, Command::SetKeyPreset { preset: p }))
            .collect();
        let change = self.link("Change a key…", Act::Flow(CommandName::SetKeybinding));
        let top = SettingRow {
            label: "Keys".to_owned(),
            value: match preset {
                KeyPreset::Gmail => "Gmail",
                KeyPreset::Vim => "Vim",
            }
            .to_owned(),
            hint: None,
            choices,
            actions: vec![change],
        };
        let mut bound = Vec::new();
        for &name in CommandName::ALL {
            let keys = crate::keys::deliverable(&state.settings, name);
            if keys.is_empty() {
                continue;
            }
            bound.push(SettingRow {
                label: name.spec().title.to_owned(),
                value: keys.join(", "),
                hint: None,
                choices: Vec::new(),
                actions: Vec::new(),
            });
        }
        vec![
            SettingsGroup {
                title: "Hardware keyboard".to_owned(),
                hint: Some(
                    "For a keyboard on a tablet or a Chromebook: the desktop's keys.".to_owned(),
                ),
                rows: vec![top],
            },
            SettingsGroup {
                title: "Bindings".to_owned(),
                hint: None,
                rows: bound,
            },
        ]
    }

    fn general(&mut self, state: &AppState) -> Vec<SettingsGroup> {
        let general = [
            SettingKey::UndoSendMs,
            SettingKey::PollSecs,
            SettingKey::InitialWindow,
        ]
        .into_iter()
        .map(|key| self.setting_row(state, key))
        .collect();
        let mut background: Vec<SettingRow> = [SettingKey::BackgroundSync]
            .into_iter()
            .map(|key| self.setting_row(state, key))
            .collect();
        if state.settings.background_sync != 0 {
            background.push(self.setting_row(state, SettingKey::BackgroundIntervalMins));
        }
        background.push(self.setting_row(state, SettingKey::Notifications));
        let battery = self.link("Battery optimization", Act::System(SystemScreen::Battery));
        let channels = self.link(
            "Notification settings",
            Act::System(SystemScreen::Notifications),
        );
        background.push(SettingRow {
            label: "System settings".to_owned(),
            value: String::new(),
            hint: Some(
                "Some phones stop background checks until dail is exempt from battery \
                 optimization."
                    .to_owned(),
            ),
            choices: Vec::new(),
            actions: vec![battery, channels],
        });
        vec![
            SettingsGroup {
                title: "General".to_owned(),
                hint: None,
                rows: general,
            },
            SettingsGroup {
                title: "Background & notifications".to_owned(),
                hint: None,
                rows: background,
            },
        ]
    }

    fn accounts(&mut self, state: &AppState) -> Vec<SettingsGroup> {
        let mut rows = Vec::new();
        for account in state.accounts.values() {
            let id = account.id;
            let value = match &account.status {
                AccountStatus::Unverified => "Not tested".to_owned(),
                AccountStatus::Verified { .. } => "Connected".to_owned(),
                AccountStatus::Failed { error, .. } => clean(&format!("Failed: {error}")),
            };
            let actions = vec![
                self.link("Test", Act::Run(Command::TestAccount { id })),
                self.link("Password", Act::Reenter(id)),
                self.link("Remove", Act::RemoveAccount(id)),
            ];
            rows.push(SettingRow {
                label: clean(&account.email),
                value,
                hint: (!account.display_name.is_empty()).then(|| clean(&account.display_name)),
                choices: Vec::new(),
                actions,
            });
        }
        let add = self.link("Add an account", Act::Setup(super::setup::SetupAct::Open));
        rows.push(SettingRow {
            label: "Another account".to_owned(),
            value: String::new(),
            hint: Some("Gmail, Hostinger Mail or any IMAP and SMTP server.".to_owned()),
            choices: Vec::new(),
            actions: vec![add],
        });
        vec![SettingsGroup {
            title: "Accounts".to_owned(),
            hint: None,
            rows,
        }]
    }

    /// Asks the host for a system settings screen.
    pub(super) fn system(&mut self, screen: SystemScreen) {
        self.externals.push(External::SystemSettings { screen });
    }
}
