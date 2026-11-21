//! The settings view (UI-12): `view.settings` shows the sections as lists of keys and values
//! over the panes. `Tab` moves between the sections, `j`/`k` between the rows, and `Enter` opens
//! the palette at the step that changes the row (`language`: a list of names). There is no appearance section: the TUI follows
//! the terminal's colours (ADR 0004), so `appearance` shows the general settings.

use crate::text::{f, s};
use app_core::{
    AccountStatus, AppState, Command, CommandName, RuleId, SettingKey, SettingsSection,
};
use app_front::actions::Run;
use app_front::format::Zone;
use app_front::i18n::Key;
use app_front::palette::{Flow, edit_rule, flow, new_rule, set_setting};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::request::Request;
use crate::text::clean;
use crate::view::{bold, dim, reverse};

/// The sections the TUI shows, in `Tab` order.
const SECTIONS: [(SettingsSection, Key); 4] = [
    (SettingsSection::General, Key::General),
    (SettingsSection::Keys, Key::Keys),
    (SettingsSection::Accounts, Key::Accounts),
    (SettingsSection::Rules, Key::RulesTab),
];

/// One row: a key, its value, and the command that changes it.
struct Row {
    key: String,
    value: String,
    change: Option<CommandName>,
    /// What `Enter` does on a row of the Rules section.
    rule: Option<RuleRow>,
    /// What `Enter` does on a row of the dail server's (SRV-6).
    action: Option<RowAction>,
}

/// What `Enter` does on a row of the Accounts section's dail server and devices (SRV-6).
#[derive(Debug, Clone)]
enum RowAction {
    /// An entry of a shared action list (`app_front::actions`).
    Run(Run),
    /// *Connect to a dail server*: the setup at that step.
    Connect,
}

/// A row of the Rules section (SET-5).
#[derive(Debug, Clone, Copy)]
enum RuleRow {
    /// A rule: `Enter` edits it, `Space` turns it on or off, `d` deletes it.
    Rule(RuleId, bool),
    /// *Add a rule…*
    Add,
    /// *Apply to this mailbox*.
    Apply,
    /// *Store on the server*.
    Push,
}

impl Row {
    const fn plain(key: String, value: String, change: Option<CommandName>) -> Self {
        Self {
            key,
            value,
            change,
            rule: None,
            action: None,
        }
    }
}

/// The section the TUI shows for `section` (appearance, about and the phone's notifications and
/// root have none of their own).
const fn shown(section: SettingsSection) -> SettingsSection {
    match section {
        SettingsSection::Appearance
        | SettingsSection::About
        | SettingsSection::Notifications
        | SettingsSection::Index => SettingsSection::General,
        other => other,
    }
}

fn rows(state: &AppState, section: SettingsSection, when: (i64, Zone)) -> Vec<Row> {
    let settings = &state.settings;
    match shown(section) {
        SettingsSection::Keys => {
            let mut out = vec![Row {
                key: "preset".to_owned(),
                value: format!("{:?}", settings.key_preset).to_lowercase(),
                change: Some(CommandName::SetKeyPreset),
                rule: None,
                action: None,
            }];
            for &name in CommandName::ALL {
                let keys = crate::keys::deliverable(settings, name);
                if !keys.is_empty() {
                    out.push(Row {
                        key: app_front::i18n::title(name).to_owned(),
                        value: keys.join(", "),
                        change: Some(CommandName::SetKeybinding),
                        rule: None,
                        action: None,
                    });
                }
            }
            out
        }
        SettingsSection::Rules => rule_rows(state),
        SettingsSection::Accounts => {
            let mut out: Vec<Row> = state
                .accounts
                .values()
                .map(|a| Row {
                    key: clean(&a.email),
                    value: match &a.status {
                        AccountStatus::Unverified => s(Key::NotTested).to_owned(),
                        AccountStatus::Verified { .. } => s(Key::Connected).to_owned(),
                        AccountStatus::Failed { error, .. } => {
                            f(Key::FailedError, &[("error", &clean(error))])
                        }
                    },
                    change: Some(CommandName::TestAccount),
                    rule: None,
                    action: None,
                })
                .collect();
            out.push(Row {
                key: s(Key::AddAnAccount).to_owned(),
                value: String::new(),
                change: Some(CommandName::AddAccount),
                rule: None,
                action: None,
            });
            if app_front::connect::availability(state) == app_front::connect::Availability::Open {
                out.push(Row {
                    key: s(Key::ConnectToServer).to_owned(),
                    value: String::new(),
                    change: None,
                    rule: None,
                    action: Some(RowAction::Connect),
                });
            }
            out.extend(server_rows(state, when));
            out
        }
        SettingsSection::General
        | SettingsSection::Appearance
        | SettingsSection::About
        | SettingsSection::Notifications
        | SettingsSection::Index => update_rows(state)
            .into_iter()
            .chain(SettingKey::ALL.iter().map(|key| {
                Row {
                    key: key.as_str().to_owned(),
                    value: match key {
                        SettingKey::Language => app_front::i18n::language_name(
                            app_front::i18n::current(),
                            key.get(settings),
                        )
                        .to_owned(),
                        _ => key.get(settings).to_string(),
                    },
                    change: Some(CommandName::SetSetting),
                    rule: None,
                    action: None,
                }
            }))
            .chain([
                Row {
                    key: "theme_light".to_owned(),
                    value: clean(&settings.theme_light),
                    change: Some(CommandName::SetTheme),
                    rule: None,
                    action: None,
                },
                Row {
                    key: "theme_dark".to_owned(),
                    value: clean(&settings.theme_dark),
                    change: Some(CommandName::SetTheme),
                    rule: None,
                    action: None,
                },
            ])
            .chain(synced_rows(state))
            .collect(),
    }
}

/// A row that runs `entry` of a shared action list, indented under what it acts on.
fn entry_row(entry: &app_front::actions::Entry, indent: &str) -> Row {
    Row {
        key: format!("{indent}{}", s(entry.label)),
        value: String::new(),
        change: None,
        rule: None,
        action: Some(RowAction::Run(entry.run.clone())),
    }
}

/// An information row (no action).
fn info(key: String, value: String) -> Row {
    Row::plain(key, value, None)
}

/// The dail server in the Accounts section (ADR 0017, SRV-6): the link's details and actions
/// (*Retry refused changes*, *Disconnect…*), the commands the server refused after an offline
/// period with *Retry all* and *Dismiss* (the problems the status line names), then *Devices*:
/// each paired device with *Revoke…*, and *Pair a device…*.
fn server_rows(state: &AppState, (now_ms, zone): (i64, Zone)) -> Vec<Row> {
    let lang = app_front::i18n::current();
    let mut out = Vec::new();
    if state.link.is_some() {
        out.push(info(s(Key::DailServer).to_owned(), String::new()));
        for field in app_front::server::details(state, lang) {
            let mut lines = field.lines.into_iter();
            out.push(info(
                format!("  {}", field.label),
                lines.next().unwrap_or_default(),
            ));
            out.extend(lines.map(|line| info(String::new(), line)));
        }
        for rejection in app_front::server::rejections(state, lang, now_ms, zone) {
            out.push(info(
                format!("  {}", rejection.text),
                format!("{} · {}", rejection.error, rejection.when),
            ));
            for entry in app_front::actions::rejection_actions(state, rejection.id) {
                out.push(entry_row(&entry, "    "));
            }
        }
        for entry in app_front::actions::link_actions(state) {
            out.push(entry_row(&entry, "  "));
        }
    }
    if app_front::server::shows_devices(state) {
        out.push(info(s(Key::Devices).to_owned(), String::new()));
        let devices = app_front::server::devices(state, lang, now_ms, zone);
        if devices.is_empty() {
            out.push(info(format!("  {}", s(Key::NoDevices)), String::new()));
        }
        for device in devices {
            out.push(info(
                format!("  {}", device.name),
                format!("{} · {} · {}", device.role, device.paired, device.seen),
            ));
            for entry in app_front::actions::device_actions(state, device.id) {
                out.push(entry_row(&entry, "    "));
            }
        }
        // *Pair a device…* (SRV-QR1) opens the pairing dialog.
        for entry in app_front::actions::server_actions(state) {
            out.push(entry_row(&entry, "  "));
        }
    }
    out
}

/// The Rules section (ADR 0015, SET-5): a row per sorting rule, then *Add a rule…*, *Apply to
/// this mailbox* and *Store on the server*. Names and summaries are cleaned (untrusted).
fn rule_rows(state: &AppState) -> Vec<Row> {
    let panel = app_front::rules::panel(state);
    let mut out: Vec<Row> = panel
        .rows
        .iter()
        .map(|rule| Row {
            key: rule.name.clone(),
            value: format!(
                "[{}] {}",
                if rule.enabled { "x" } else { " " },
                rule.summary
            ),
            change: None,
            rule: Some(RuleRow::Rule(rule.id, rule.enabled)),
            action: None,
        })
        .collect();
    if out.is_empty() {
        out.push(Row::plain(
            s(Key::RulesEmpty).to_owned(),
            String::new(),
            None,
        ));
    }
    out.push(Row {
        key: s(Key::AddRule).to_owned(),
        value: String::new(),
        change: None,
        rule: Some(RuleRow::Add),
        action: None,
    });
    out.push(Row {
        key: s(Key::RulesApplyHere).to_owned(),
        value: if panel.mailbox.is_some() {
            String::new()
        } else {
            s(Key::RulesApplyHint).to_owned()
        },
        change: None,
        rule: Some(RuleRow::Apply),
        action: None,
    });
    out.push(Row {
        key: s(Key::RulesStore).to_owned(),
        value: s(if panel.dirty {
            Key::RulesNotStored
        } else {
            Key::RulesStored
        })
        .to_owned(),
        change: None,
        rule: Some(RuleRow::Push),
        action: None,
    });
    out
}

/// What a key does on the Rules section's row `row`; `None` when the key is not the section's.
fn rule_key(code: KeyCode, row: Option<RuleRow>, state: &AppState) -> Option<Outcome> {
    let mut outcome = Outcome::default();
    let mut run = |cmd: Command| outcome.requests.push(Request::user(cmd));
    let mut flow = None;
    match (code, row) {
        (KeyCode::Enter, Some(RuleRow::Rule(id, _))) => flow = Some(edit_rule(state, id)),
        (KeyCode::Char(' '), Some(RuleRow::Rule(id, on))) => {
            run(Command::EnableRule { id, enabled: !on });
        }
        (KeyCode::Char('d') | KeyCode::Delete, Some(RuleRow::Rule(id, _))) => {
            run(Command::DeleteRule { id });
        }
        (KeyCode::Enter, Some(RuleRow::Add)) | (KeyCode::Char('a'), _) => {
            flow = Some(new_rule());
        }
        (KeyCode::Enter, Some(RuleRow::Apply)) | (KeyCode::Char('x'), _) => {
            if let Some(mailbox) = app_front::rules::panel(state).mailbox {
                run(Command::ApplyRules { mailbox });
            }
        }
        (KeyCode::Enter, Some(RuleRow::Push)) | (KeyCode::Char('p'), _) => {
            if app_front::rules::panel(state).can_push {
                run(Command::PushSettings {});
            }
        }
        (KeyCode::Enter, _) => {}
        _ => return None,
    }
    match flow {
        Some(Flow::Ask(step)) => outcome.ask = Some(step),
        Some(Flow::Run(cmd)) => outcome.requests.push(Request::user(cmd)),
        _ => {}
    }
    Some(outcome)
}

/// The settings on the mail server (ADR 0015, SET-3): the account that holds them and the
/// state of the last pull or push, with its error.
fn synced_rows(state: &AppState) -> Vec<Row> {
    let Some(panel) = app_front::synced::panel(state) else {
        return Vec::new();
    };
    vec![
        Row {
            key: s(Key::SettingsAccount).to_owned(),
            value: panel.account_label,
            change: Some(CommandName::SetSettingsAccount),
            rule: None,
            action: None,
        },
        Row {
            key: s(Key::SettingsSyncState).to_owned(),
            value: match panel.pending {
                Some(pending) => format!("{} · {pending}", panel.status),
                None => panel.status,
            },
            change: Some(CommandName::PullSettings),
            rule: None,
            action: None,
        },
        Row {
            key: String::new(),
            value: panel.shared.to_owned(),
            change: None,
            rule: None,
            action: None,
        },
    ]
}

/// The update rows (Wave 7 F1): the version and status, one row per button, and the policy.
fn update_rows(state: &AppState) -> Vec<Row> {
    use app_front::updates::{Action, interval_label, panel};

    let mut out = Vec::new();
    // Without a reported host this build cannot check: only the policy shows.
    if let Some(host) = &state.update.host {
        // No clock here: the age of the last check is left out.
        let panel = panel(state, 0, &host.version);
        out.push(Row {
            key: panel.version,
            value: clean(&panel.status),
            change: None,
            rule: None,
            action: None,
        });
        for action in &panel.actions {
            let (value, change) = match action {
                Action::Check => (String::new(), CommandName::CheckForUpdates),
                Action::Download => (String::new(), CommandName::DownloadUpdate),
                Action::Install => (String::new(), CommandName::InstallUpdate),
                Action::Skip(version) => (clean(version), CommandName::SkipUpdateVersion),
                // No browser from a terminal: the address, to copy.
                Action::Open(url) => {
                    out.push(Row {
                        key: s(Key::ReleaseNotes).to_owned(),
                        value: clean(url),
                        change: None,
                        rule: None,
                        action: None,
                    });
                    continue;
                }
            };
            out.push(Row {
                key: format!("  {}", action.label()),
                value,
                change: Some(change),
                rule: None,
                action: None,
            });
        }
    }
    let policy = &state.settings.updates;
    out.push(Row {
        key: "update_checks".to_owned(),
        value: format!(
            "{} ({})",
            serde_json::to_value(policy.checks)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            interval_label(policy.interval_hours)
        ),
        change: Some(CommandName::SetUpdatePolicy),
        rule: None,
        action: None,
    });
    out
}

/// What a key in the settings view did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Commands for the session.
    pub requests: Vec<Request>,
    /// A palette step to open.
    pub ask: Option<app_front::palette::Ask>,
    /// The account setup to open.
    pub wizard: bool,
    /// The *Pair a device…* dialog to open.
    pub pair: bool,
    /// The setup to open at *Connect to a dail server*.
    pub connect: bool,
    /// The key was not the view's: let the keymap have it.
    pub unhandled: bool,
}

/// The settings view's cursor.
#[derive(Debug)]
pub struct SettingsView {
    cursor: usize,
    /// The clock and the time zone of the dates shown (devices, refusals).
    when: (i64, Zone),
}

impl Default for SettingsView {
    fn default() -> Self {
        Self {
            cursor: 0,
            when: (0, app_front::format::utc),
        }
    }
}

impl SettingsView {
    /// Sets the clock and the time zone of the dates shown; the TUI calls it before each frame
    /// and key.
    pub const fn at(&mut self, now_ms: i64, zone: Zone) {
        self.when = (now_ms, zone);
    }

    /// Handles a key while the settings are shown.
    pub fn key(&mut self, key: KeyEvent, state: &AppState) -> Outcome {
        let Some(section) = state.view.settings else {
            return Outcome {
                unhandled: true,
                ..Outcome::default()
            };
        };
        let rows = rows(state, section, self.when);
        if shown(section) == SettingsSection::Rules
            && let Some(outcome) =
                rule_key(key.code, rows.get(self.cursor).and_then(|r| r.rule), state)
        {
            return outcome;
        }
        let mut outcome = Outcome::default();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(rows.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Tab | KeyCode::BackTab => {
                let at = SECTIONS
                    .iter()
                    .position(|(s, _)| *s == shown(section))
                    .unwrap_or(0);
                let next = if key.code == KeyCode::Tab {
                    (at + 1) % SECTIONS.len()
                } else {
                    (at + SECTIONS.len() - 1) % SECTIONS.len()
                };
                self.cursor = 0;
                outcome.requests.push(Request::user(Command::OpenSettings {
                    section: SECTIONS.get(next).map(|(s, _)| *s),
                }));
            }
            KeyCode::Enter => {
                let Some(row) = rows.get(self.cursor) else {
                    return outcome;
                };
                match &row.action {
                    Some(RowAction::Connect) => outcome.connect = true,
                    Some(RowAction::Run(Run::Pair)) => outcome.pair = true,
                    Some(RowAction::Run(run)) => match run.flow(state) {
                        Flow::Ask(step) => outcome.ask = Some(step),
                        Flow::Run(cmd) => outcome.requests.push(Request::user(cmd)),
                        Flow::Wizard => outcome.wizard = true,
                        Flow::Password(_) | Flow::Hidden => {}
                    },
                    None => {}
                }
                let Some(name) = row.change else {
                    return outcome;
                };
                // A setting's row has already chosen the setting.
                let setting = SettingKey::ALL.iter().find(|k| k.as_str() == row.key);
                let step = match setting {
                    Some(key) => set_setting(state, *key),
                    None => flow(name, state),
                };
                match (name, step) {
                    (CommandName::StartPairing, _) => outcome.pair = true,
                    (CommandName::TestAccount, _) => {
                        if let Some(account) = state.accounts.values().nth(self.cursor) {
                            outcome
                                .requests
                                .push(Request::user(Command::TestAccount { id: account.id }));
                        }
                    }
                    (_, Flow::Ask(step)) => outcome.ask = Some(step),
                    (_, Flow::Run(cmd)) => outcome.requests.push(Request::user(cmd)),
                    (_, Flow::Wizard) => outcome.wizard = true,
                    (_, Flow::Password(_) | Flow::Hidden) => {}
                }
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.cursor = 0;
                outcome
                    .requests
                    .push(Request::user(Command::CloseSettings {}));
            }
            _ => outcome.unhandled = true,
        }
        outcome
    }

    /// Draws the view over `area`.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, state: &AppState, borders: bool) {
        let Some(section) = state.view.settings else {
            return;
        };
        let rows = rows(state, section, self.when);
        self.cursor = self.cursor.min(rows.len().saturating_sub(1));
        frame.render_widget(Clear, area);
        let block = if borders {
            Block::bordered()
                .border_style(bold())
                .title(s(Key::SettingsTitle))
        } else {
            Block::new()
        };
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let tabs: Vec<Span<'static>> = SECTIONS
            .iter()
            .flat_map(|(s, title)| {
                let style = if *s == shown(section) {
                    reverse()
                } else {
                    dim()
                };
                [
                    Span::styled(format!(" {} ", crate::text::s(*title)), style),
                    Span::raw(" "),
                ]
            })
            .collect();
        lines.push(Line::from(tabs));
        lines.push(Line::from(""));
        let width = rows
            .iter()
            .map(|r| r.key.chars().count())
            .max()
            .unwrap_or(0)
            .min(32);
        let visible = usize::from(inner.height.saturating_sub(3));
        let first = self.cursor.saturating_sub(visible.saturating_sub(1));
        for (i, row) in rows.iter().enumerate().skip(first).take(visible) {
            let text = format!("{:width$}  {}", row.key, row.value);
            let style = if i == self.cursor {
                reverse()
            } else {
                ratatui::style::Style::new()
            };
            lines.push(Line::from(Span::styled(text, style)));
        }
        frame.render_widget(Paragraph::new(lines), inner);
        let footer = Rect {
            y: inner.bottom().saturating_sub(1),
            height: 1,
            ..inner
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                s(if shown(section) == SettingsSection::Rules {
                    Key::RulesFooter
                } else {
                    Key::SettingsFooter
                }),
                dim(),
            ))),
            footer,
        );
    }
}
