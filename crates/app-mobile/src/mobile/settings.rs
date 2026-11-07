//! The settings page (`open_settings`): the sections as tabs, each a list of rows whose
//! choices run `set_setting`, `set_theme` or `set_key_preset` directly. General also holds the
//! phone's *Background & notifications* group. Settings that only the desktop reads
//! (`text_scale`, `tui_colors`, `history_cap`) are not shown; the desktop and `config.toml`
//! change them.

use app_core::{
    AccountStatus, AppState, Command, CommandName, KeyPreset, SettingKey, SettingsSection,
    ThemeMode,
};
use app_front::i18n::{self, Key};
use app_front::text::clean;
use app_theme::{Appearance, builtin};

use super::{Act, Mobile};
use crate::event::{External, SystemScreen};
use crate::frame::{Button, Choice, SettingRow, SettingsGroup, SettingsPage};

/// The tabs, in order.
const TABS: [(SettingsSection, Key); 5] = [
    (SettingsSection::Appearance, Key::Appearance),
    (SettingsSection::Keys, Key::Keyboard),
    (SettingsSection::General, Key::General),
    (SettingsSection::Accounts, Key::Accounts),
    (SettingsSection::Rules, Key::RulesTab),
];

/// The values offered for a numeric setting, with their labels.
fn presets(key: SettingKey) -> Vec<(u64, &'static str)> {
    match key {
        SettingKey::UndoSendMs => vec![
            (0, i18n::s(Key::Off)),
            (5_000, "5 s"),
            (10_000, "10 s"),
            (20_000, "20 s"),
            (30_000, "30 s"),
        ],
        SettingKey::PollSecs => vec![
            (60, "1 min"),
            (120, "2 min"),
            (300, "5 min"),
            (900, "15 min"),
            (3_600, "1 h"),
        ],
        SettingKey::InitialWindow => {
            vec![(200, "200"), (500, "500"), (1_000, "1000"), (5_000, "5000")]
        }
        SettingKey::Language => i18n::languages(i18n::current()).to_vec(),
        SettingKey::ListDensity => vec![
            (0, i18n::s(Key::CozyX)),
            (1, i18n::s(Key::CompactX)),
            (2, i18n::s(Key::Preview)),
        ],
        SettingKey::Notifications => vec![(0, i18n::s(Key::Off)), (1, i18n::s(Key::On))],
        SettingKey::BackgroundSync => vec![
            (0, i18n::s(Key::OnlyWhileOpen)),
            (1, i18n::s(Key::Periodic)),
            (2, i18n::s(Key::Push)),
        ],
        SettingKey::BackgroundIntervalMins => vec![
            (15, "15 min"),
            (30, "30 min"),
            (60, "1 h"),
            (120, "2 h"),
            (360, "6 h"),
            (720, "12 h"),
        ],
        SettingKey::MarkReadAfterMs => vec![
            (0, i18n::s(Key::Never)),
            (250, i18n::s(Key::Immediately)),
            (2_000, i18n::s(Key::After2s)),
            (5_000, i18n::s(Key::After5s)),
        ],
        SettingKey::HistoryCap
        | SettingKey::Theme
        | SettingKey::TextScale
        | SettingKey::TuiColors
        | SettingKey::StaleSecs
        | SettingKey::FullSyncSecs
        | SettingKey::AfterTriage => Vec::new(),
    }
}

/// The label of a setting on the phone.
fn label(key: SettingKey) -> &'static str {
    match key {
        SettingKey::UndoSendMs => i18n::s(Key::UndoSend),
        SettingKey::PollSecs => i18n::s(Key::CheckTheInboxEvery),
        SettingKey::InitialWindow => i18n::s(Key::MessagesFetchedFirst),
        SettingKey::Language => i18n::s(Key::LanguageSetting),
        SettingKey::ListDensity => i18n::s(Key::ListDensity),
        SettingKey::Notifications => i18n::s(Key::Notifications),
        SettingKey::BackgroundSync => i18n::s(Key::MailChecksInTheBackground),
        SettingKey::BackgroundIntervalMins => i18n::s(Key::CheckEvery),
        SettingKey::HistoryCap => i18n::s(Key::UndoHistory),
        SettingKey::Theme => i18n::s(Key::Theme),
        SettingKey::TextScale => i18n::s(Key::TextSize),
        SettingKey::TuiColors => i18n::s(Key::TerminalColours),
        SettingKey::MarkReadAfterMs => i18n::s(Key::MarkAsReadAfter),
        SettingKey::StaleSecs => i18n::s(Key::RefreshAMailboxOnOpenAfter),
        SettingKey::FullSyncSecs => i18n::s(Key::SyncEveryFolderEvery),
        SettingKey::AfterTriage => i18n::s(Key::AfterArchiving),
    }
}

fn hint(key: SettingKey) -> Option<&'static str> {
    match key {
        SettingKey::UndoSendMs => Some(i18n::s(Key::HowLongASendCanStillBe)),
        SettingKey::PollSecs => Some(i18n::s(Key::WhileTheAppIsOpen)),
        SettingKey::BackgroundSync => Some(
            "Periodic checks wait for the phone's maintenance windows. Push keeps a silent \
             notification while it watches the inboxes.",
        ),
        SettingKey::Notifications => Some(i18n::s(Key::NewMailAndFailedSendsWhileDail)),
        SettingKey::InitialWindow => Some(i18n::s(Key::PerMailboxOnItsFirstSync)),
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
        // The phones have an About screen of their own (MISC-3): this section is the desktop's.
        let section = match state.view.settings? {
            SettingsSection::About => SettingsSection::General,
            other => other,
        };
        let tabs = TABS
            .iter()
            .map(|(s, title)| {
                self.choice(
                    i18n::s(*title),
                    *s == section,
                    Command::OpenSettings { section: Some(*s) },
                )
            })
            .collect();
        let groups = match section {
            SettingsSection::Appearance => self.appearance(state),
            SettingsSection::Keys => self.keyboard(state),
            SettingsSection::General | SettingsSection::About => self.general(state),
            SettingsSection::Accounts => self.accounts(state),
            SettingsSection::Rules => self.rules(state),
        };
        let name = match section {
            SettingsSection::Appearance => "appearance",
            SettingsSection::Keys => "keys",
            SettingsSection::General | SettingsSection::About => "general",
            SettingsSection::Accounts => "accounts",
            SettingsSection::Rules => "rules",
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
            (ThemeMode::System, i18n::s(Key::System), 0),
            (ThemeMode::Light, i18n::s(Key::Light), 1),
            (ThemeMode::Dark, i18n::s(Key::Dark), 2),
        ];
        let current = modes
            .iter()
            .find(|m| m.2 == settings.theme)
            .map_or(i18n::s(Key::System), |m| m.1);
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
            label: i18n::s(Key::Mode).to_owned(),
            value: current.to_owned(),
            hint: Some(i18n::s(Key::SystemFollowsAndroidSDarkTheme).to_owned()),
            choices,
            actions: Vec::new(),
        }];
        for (appearance, label, chosen) in [
            (
                Appearance::Light,
                i18n::s(Key::LightTheme),
                &settings.theme_light,
            ),
            (
                Appearance::Dark,
                i18n::s(Key::DarkTheme),
                &settings.theme_dark,
            ),
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
        rows.push(self.setting_row(state, SettingKey::Language));
        rows.push(self.setting_row(state, SettingKey::ListDensity));
        vec![SettingsGroup {
            title: i18n::s(Key::Appearance).to_owned(),
            hint: Some(i18n::s(Key::TextSizeFollowsAndroidSFontSize).to_owned()),
            rows,
        }]
    }

    fn keyboard(&mut self, state: &AppState) -> Vec<SettingsGroup> {
        let preset = state.settings.key_preset;
        let choices = [(KeyPreset::Gmail, "Gmail"), (KeyPreset::Vim, "Vim")]
            .into_iter()
            .map(|(p, label)| self.choice(label, p == preset, Command::SetKeyPreset { preset: p }))
            .collect();
        let change = self.link(
            i18n::s(Key::ChangeAKey),
            Act::Flow(CommandName::SetKeybinding),
        );
        let top = SettingRow {
            label: i18n::s(Key::Keys).to_owned(),
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
                label: app_front::i18n::title(name).to_owned(),
                value: keys.join(", "),
                hint: None,
                choices: Vec::new(),
                actions: Vec::new(),
            });
        }
        vec![
            SettingsGroup {
                title: i18n::s(Key::HardwareKeyboard).to_owned(),
                hint: Some(i18n::s(Key::ForAKeyboardOnATabletOr).to_owned()),
                rows: vec![top],
            },
            SettingsGroup {
                title: i18n::s(Key::Bindings).to_owned(),
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
            SettingKey::MarkReadAfterMs,
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
        let battery = self.link(
            i18n::s(Key::BatteryOptimization),
            Act::System(SystemScreen::Battery),
        );
        let channels = self.link(
            i18n::s(Key::NotificationSettings),
            Act::System(SystemScreen::Notifications),
        );
        background.push(SettingRow {
            label: i18n::s(Key::SystemSettings).to_owned(),
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
                title: i18n::s(Key::General).to_owned(),
                hint: None,
                rows: general,
            },
            SettingsGroup {
                title: i18n::s(Key::BackgroundNotifications).to_owned(),
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
                AccountStatus::Unverified => i18n::s(Key::NotTestedX).to_owned(),
                AccountStatus::Verified { .. } => i18n::s(Key::ConnectedX).to_owned(),
                AccountStatus::Failed { error, .. } => {
                    clean(&i18n::f(Key::Failed, &[("error", error)]))
                }
            };
            let actions = vec![
                self.link(i18n::s(Key::Test), Act::Run(Command::TestAccount { id })),
                self.link(i18n::s(Key::Password), Act::Reenter(id)),
                self.link(i18n::s(Key::Remove), Act::RemoveAccount(id)),
            ];
            rows.push(SettingRow {
                label: clean(&account.email),
                value,
                hint: (!account.display_name.is_empty()).then(|| clean(&account.display_name)),
                choices: Vec::new(),
                actions,
            });
        }
        let add = self.link(
            i18n::s(Key::AddAnAccount),
            Act::Setup(super::setup::SetupAct::Open),
        );
        rows.push(SettingRow {
            label: i18n::s(Key::AnotherAccount).to_owned(),
            value: String::new(),
            hint: Some(i18n::s(Key::GmailHostingerMailOrAnyIMAPAnd).to_owned()),
            choices: Vec::new(),
            actions: vec![add],
        });
        vec![SettingsGroup {
            title: i18n::s(Key::Accounts).to_owned(),
            hint: None,
            rows,
        }]
    }

    /// The Rules section (ADR 0015, SET-5): a row per sorting rule (on/off, *Edit…*,
    /// *Delete*), then *Add a rule…* and *Apply to this mailbox*, and the push. Rule names and
    /// summaries are untrusted: cleaned plain text.
    fn rules(&mut self, state: &AppState) -> Vec<SettingsGroup> {
        let panel = app_front::rules::panel(state);
        let mut rows = Vec::new();
        for rule in &panel.rows {
            let id = rule.id;
            let choices = [(true, Key::On), (false, Key::Off)]
                .into_iter()
                .map(|(enabled, label)| {
                    self.choice(
                        i18n::s(label),
                        rule.enabled == enabled,
                        Command::EnableRule { id, enabled },
                    )
                })
                .collect();
            let actions = vec![
                self.link(i18n::s(Key::EditRule), Act::EditRule(id)),
                self.link(
                    i18n::s(Key::RuleDelete),
                    Act::Run(Command::DeleteRule { id }),
                ),
            ];
            rows.push(SettingRow {
                label: rule.name.clone(),
                value: i18n::s(if rule.enabled { Key::On } else { Key::Off }).to_owned(),
                hint: Some(rule.summary.clone()),
                choices,
                actions,
            });
        }
        let mut more = vec![self.link(i18n::s(Key::AddRule), Act::AddRule)];
        if let Some(mailbox) = panel.mailbox {
            more.push(self.link(
                i18n::s(Key::RulesApplyHere),
                Act::Run(Command::ApplyRules { mailbox }),
            ));
        }
        rows.push(SettingRow {
            label: i18n::s(Key::AddRule).to_owned(),
            value: String::new(),
            hint: panel
                .rows
                .is_empty()
                .then(|| i18n::s(Key::RulesEmpty).to_owned()),
            choices: Vec::new(),
            actions: more,
        });
        let store = if panel.dirty && panel.can_push {
            vec![self.link(i18n::s(Key::RulesStore), Act::Run(Command::PushSettings {}))]
        } else {
            Vec::new()
        };
        let server = SettingRow {
            label: i18n::s(Key::SettingsSyncState).to_owned(),
            value: i18n::s(if panel.dirty {
                Key::RulesNotStored
            } else {
                Key::RulesStored
            })
            .to_owned(),
            hint: None,
            choices: Vec::new(),
            actions: store,
        };
        vec![
            SettingsGroup {
                title: i18n::s(Key::RulesTitle).to_owned(),
                hint: Some(i18n::s(Key::RulesHint).to_owned()),
                rows,
            },
            SettingsGroup {
                title: i18n::s(Key::SyncedSettings).to_owned(),
                hint: Some(i18n::s(Key::SettingsShared).to_owned()),
                rows: vec![server],
            },
        ]
    }

    /// Asks the host for a system settings screen.
    pub(super) fn system(&mut self, screen: SystemScreen) {
        self.externals.push(External::SystemSettings { screen });
    }
}
