//! The settings view (UI-12): `view.settings` shows the sections as lists of keys and values
//! over the panes. `Tab` moves between the sections, `j`/`k` between the rows, and `Enter` opens
//! the palette at the step that changes the row. There is no appearance section: the TUI follows
//! the terminal's colours (ADR 0004), so `appearance` shows the general settings.

use app_core::{AccountStatus, AppState, Command, CommandName, SettingKey, SettingsSection};
use app_front::palette::{Flow, flow};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::request::Request;
use crate::text::clean;
use crate::view::{bold, dim, reverse};

/// The sections the TUI shows, in `Tab` order.
const SECTIONS: [(SettingsSection, &str); 3] = [
    (SettingsSection::General, "General"),
    (SettingsSection::Keys, "Keys"),
    (SettingsSection::Accounts, "Accounts"),
];

/// One row: a key, its value, and the command that changes it.
struct Row {
    key: String,
    value: String,
    change: Option<CommandName>,
}

/// The section the TUI shows for `section` (appearance has none of its own).
const fn shown(section: SettingsSection) -> SettingsSection {
    match section {
        SettingsSection::Appearance => SettingsSection::General,
        other => other,
    }
}

fn rows(state: &AppState, section: SettingsSection) -> Vec<Row> {
    let settings = &state.settings;
    match shown(section) {
        SettingsSection::Keys => {
            let mut out = vec![Row {
                key: "preset".to_owned(),
                value: format!("{:?}", settings.key_preset).to_lowercase(),
                change: Some(CommandName::SetKeyPreset),
            }];
            for &name in CommandName::ALL {
                let keys = crate::keys::deliverable(settings, name);
                if !keys.is_empty() {
                    out.push(Row {
                        key: name.spec().title.to_owned(),
                        value: keys.join(", "),
                        change: Some(CommandName::SetKeybinding),
                    });
                }
            }
            out
        }
        SettingsSection::Accounts => {
            let mut out: Vec<Row> = state
                .accounts
                .values()
                .map(|a| Row {
                    key: clean(&a.email),
                    value: match &a.status {
                        AccountStatus::Unverified => "not tested".to_owned(),
                        AccountStatus::Verified { .. } => "connected".to_owned(),
                        AccountStatus::Failed { error, .. } => clean(&format!("failed: {error}")),
                    },
                    change: Some(CommandName::TestAccount),
                })
                .collect();
            out.push(Row {
                key: "Add an account".to_owned(),
                value: String::new(),
                change: Some(CommandName::AddAccount),
            });
            out
        }
        SettingsSection::General | SettingsSection::Appearance => update_rows(state)
            .into_iter()
            .chain(SettingKey::ALL.iter().map(|key| Row {
                key: key.as_str().to_owned(),
                value: key.get(settings).to_string(),
                change: Some(CommandName::SetSetting),
            }))
            .chain([
                Row {
                    key: "theme_light".to_owned(),
                    value: clean(&settings.theme_light),
                    change: Some(CommandName::SetTheme),
                },
                Row {
                    key: "theme_dark".to_owned(),
                    value: clean(&settings.theme_dark),
                    change: Some(CommandName::SetTheme),
                },
            ])
            .collect(),
    }
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
                        key: "  release notes".to_owned(),
                        value: clean(url),
                        change: None,
                    });
                    continue;
                }
            };
            out.push(Row {
                key: format!("  {}", action.label()),
                value,
                change: Some(change),
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
    /// The key was not the view's: let the keymap have it.
    pub unhandled: bool,
}

/// The settings view's cursor.
#[derive(Debug, Default)]
pub struct SettingsView {
    cursor: usize,
}

impl SettingsView {
    /// Handles a key while the settings are shown.
    pub fn key(&mut self, key: KeyEvent, state: &AppState) -> Outcome {
        let Some(section) = state.view.settings else {
            return Outcome {
                unhandled: true,
                ..Outcome::default()
            };
        };
        let rows = rows(state, section);
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
                let Some(name) = rows.get(self.cursor).and_then(|r| r.change) else {
                    return outcome;
                };
                match (name, flow(name, state)) {
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
        let rows = rows(state, section);
        self.cursor = self.cursor.min(rows.len().saturating_sub(1));
        frame.render_widget(Clear, area);
        let block = if borders {
            Block::bordered().border_style(bold()).title(" Settings ")
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
                [Span::styled(format!(" {title} "), style), Span::raw(" ")]
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
                "Tab section · j/k move · Enter change · Esc close",
                dim(),
            ))),
            footer,
        );
    }
}
