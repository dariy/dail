//! The settings page (UI-12): `view.settings` shows it in place of the list and the reader, with
//! tabs for Appearance, Keyboard, General and Accounts. It only shows the settings; every
//! control sends a command (`set_theme`, `set_setting`, `set_key_preset`, …), so agents and the
//! CLI change the same things.

use app_core::{
    AccountId, AccountStatus, AppState, Category, Command, CommandName, KeyPreset, SettingKey,
    SettingsSection, ThemeMode,
};
use app_front::palette::{Ask, Flow};
use app_theme::{Appearance, Icon, builtin};
use egui::{Sense, Ui, Vec2};

use crate::bridge::Request;
use crate::theme::{self, Tokens};
use crate::ui;

/// UI-local state of the page.
#[derive(Debug, Default)]
pub struct SettingsState {
    /// The key list's filter.
    filter: String,
    /// The account whose *Remove…* was clicked once and waits for the confirmation.
    confirm_remove: Option<AccountId>,
}

/// What the page asked for that is not a command.
#[derive(Debug, Default)]
pub struct SettingsActions {
    /// Open the account wizard.
    pub wizard: bool,
    /// Open the password form for this account.
    pub password: Option<AccountId>,
    /// Open the palette at this step (*Change…* on a key binding).
    pub ask: Option<Ask>,
}

/// The tabs, in order, with their icons and titles.
const TABS: [(SettingsSection, Icon, &str); 4] = [
    (SettingsSection::Appearance, Icon::Contrast, "Appearance"),
    (SettingsSection::Keys, Icon::Keyboard, "Keyboard"),
    (SettingsSection::General, Icon::Settings, "General"),
    (SettingsSection::Accounts, Icon::Person, "Accounts"),
];

impl SettingsState {
    /// Renders the page for `section`.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        state: &AppState,
        section: SettingsSection,
        queue: &mut Vec<Request>,
    ) -> SettingsActions {
        let mut actions = SettingsActions::default();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.m.space.sm;
            ui.label(ui::text(
                &t.type_scale.headline_lg,
                "Settings",
                t.text_strong,
            ));
            ui::badge(
                ui,
                t,
                concat!("DAIL ", env!("CARGO_PKG_VERSION")),
                ui::Badge::Outline,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::icon_button(ui, t, t.text_secondary, Icon::Close, "Close settings (Esc)")
                    .clicked()
                {
                    queue.push(Request::user(Command::CloseSettings {}));
                }
            });
        });
        ui.add_space(t.m.space.md);
        let items: Vec<(Icon, &str)> = TABS
            .iter()
            .map(|(_, icon, title)| (*icon, *title))
            .collect();
        let active = TABS.iter().position(|(s, _, _)| *s == section).unwrap_or(0);
        if let Some(i) = ui::tabs(ui, t, &items, active)
            && let Some((section, _, _)) = TABS.get(i)
        {
            queue.push(Request::user(Command::OpenSettings {
                section: Some(*section),
            }));
        }
        ui::divider(ui, t);
        ui.add_space(t.m.space.lg);
        egui::ScrollArea::vertical()
            .id_salt("settings")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(t.m.frame.text_width_max + t.m.frame.sidebar);
                match section {
                    SettingsSection::Appearance => appearance(ui, t, state, queue),
                    SettingsSection::Keys => {
                        actions.ask = keys(ui, t, state, &mut self.filter, queue);
                    }
                    SettingsSection::General => general(ui, t, state, queue),
                    SettingsSection::Accounts => {
                        self.accounts(ui, t, state, &mut actions, queue);
                    }
                }
            });
        actions
    }

    fn accounts(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        state: &AppState,
        actions: &mut SettingsActions,
        queue: &mut Vec<Request>,
    ) {
        for account in state.accounts.values() {
            ui::card(t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = t.m.space.md;
                    ui::avatar(ui, t, &account.display_name, t.m.controls.avatar_lg);
                    ui.vertical(|ui| {
                        ui.label(ui::text(
                            &t.type_scale.headline_md,
                            account.email.as_str(),
                            t.text_strong,
                        ));
                        let status = match &account.status {
                            AccountStatus::Unverified => "not tested yet".to_owned(),
                            AccountStatus::Verified { .. } => "connected".to_owned(),
                            AccountStatus::Failed { error, .. } => format!("failed: {error}"),
                        };
                        ui.label(ui::text(
                            &t.type_scale.label_sm,
                            format!(
                                "{:?} · IMAP {}:{} · SMTP {}:{} · {status}",
                                account.preset,
                                account.imap.host,
                                account.imap.port,
                                account.smtp.host,
                                account.smtp.port
                            ),
                            t.text_muted,
                        ));
                    });
                });
                ui.add_space(t.m.space.sm);
                ui.horizontal_wrapped(|ui| {
                    if ui::secondary(ui, t, "Test", None, true) {
                        queue.push(Request::user(Command::TestAccount { id: account.id }));
                    }
                    if ui::secondary(ui, t, "Re-enter password", None, true) {
                        actions.password = Some(account.id);
                    }
                    if self.confirm_remove == Some(account.id) {
                        let remove = ui::text(
                            &t.type_scale.body_md,
                            format!("Remove {} and its mail cache", account.email),
                            t.danger,
                        );
                        if ui::ghost(ui, t, remove).clicked() {
                            queue.push(Request::user(Command::RemoveAccount {
                                id: account.id,
                                confirm: true,
                            }));
                            self.confirm_remove = None;
                        }
                        let keep = ui::text(&t.type_scale.body_md, "Keep it", t.text_secondary);
                        if ui::ghost(ui, t, keep).clicked() {
                            self.confirm_remove = None;
                        }
                    } else {
                        let remove = ui::text(&t.type_scale.body_md, "Remove…", t.text_secondary);
                        if ui::ghost(ui, t, remove).clicked() {
                            self.confirm_remove = Some(account.id);
                        }
                    }
                });
            });
            ui.add_space(t.m.space.sm);
        }
        if ui::primary(ui, t, "Add account", None, true) {
            actions.wizard = true;
        }
    }
}

/// A row of the page: a label and a description on the left, the control on the right.
fn row(ui: &mut Ui, t: &Tokens, label: &str, detail: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(t.m.rows.palette + t.m.space.md);
        ui.vertical(|ui| {
            ui.label(ui::text(&t.type_scale.body_md, label, t.text_strong));
            if !detail.is_empty() {
                ui.label(ui::text(&t.type_scale.body_sm, detail, t.text_muted));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
    ui::divider(ui, t);
}

/// A segmented control over the allowed values of a setting; sends `set_setting`.
fn choices(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    key: SettingKey,
    options: &[(u64, &str)],
    queue: &mut Vec<Request>,
) {
    let current = key.get(&state.settings);
    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
    let chosen = options
        .iter()
        .position(|(v, _)| *v == current)
        .unwrap_or(usize::MAX);
    if let Some(i) = ui::segmented(ui, t, &labels, chosen)
        && let Some((value, _)) = options.get(i)
        && *value != current
    {
        queue.push(Request::user(Command::SetSetting {
            key,
            value: (*value).into(),
        }));
    }
}

fn appearance(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
    let settings = &state.settings;
    row(
        ui,
        t,
        "Theme",
        "Follow the desktop, or always light or dark",
        |ui| {
            let modes = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
            let chosen = modes
                .iter()
                .position(|m| *m == ThemeMode::of(settings.theme))
                .unwrap_or(0);
            if let Some(i) = ui::segmented(ui, t, &["System", "Light", "Dark"], chosen)
                && i != chosen
            {
                queue.push(Request::user(Command::SetTheme {
                    mode: modes.get(i).copied(),
                    light: None,
                    dark: None,
                }));
            }
        },
    );
    for (appearance, title, current) in [
        (Appearance::Light, "Light theme", &settings.theme_light),
        (Appearance::Dark, "Dark theme", &settings.theme_dark),
    ] {
        ui.add_space(t.m.space.md);
        ui::caption(ui, t, title);
        ui.add_space(t.m.space.xs);
        ui.horizontal_wrapped(|ui| {
            for theme in builtin::all()
                .into_iter()
                .filter(|th| th.appearance == appearance)
            {
                let chosen = theme.id == current.as_str();
                if theme_card(ui, t, &theme, chosen).clicked() && !chosen {
                    let id = theme.id.to_string();
                    let (light, dark) = match appearance {
                        Appearance::Light => (Some(id), None),
                        Appearance::Dark => (None, Some(id)),
                    };
                    queue.push(Request::user(Command::SetTheme {
                        mode: None,
                        light,
                        dark,
                    }));
                }
            }
        });
    }
    ui.add_space(t.m.space.md);
    row(
        ui,
        t,
        "Density",
        "How tall the rows of the thread list are",
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::ListDensity,
                &[(0, "Cozy 48"), (1, "Compact 40"), (2, "Preview 72")],
                queue,
            );
        },
    );
    row(
        ui,
        t,
        "Text size",
        "Ctrl + plus and minus change it for a while",
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::TextScale,
                &[(90, "90 %"), (100, "100 %"), (125, "125 %"), (150, "150 %")],
                queue,
            );
        },
    );
}

/// A selectable card with a small preview of a theme: its canvas, chrome, text and inverse.
fn theme_card(ui: &mut Ui, t: &Tokens, theme: &app_theme::Theme, chosen: bool) -> egui::Response {
    let size = Vec2::new(t.m.frame.sidebar * 0.75, t.m.controls.avatar_lg * 2.5);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, true, chosen, theme.name.as_ref())
    });
    let p = ui.painter();
    let stroke = if chosen {
        egui::Stroke::new(t.m.stroke.indicator, t.focus)
    } else {
        egui::Stroke::new(t.m.stroke.hairline, t.border)
    };
    let preview = egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width(), rect.height() - t.m.rows.palette),
    );
    p.rect_filled(
        preview,
        ui::radius(t.m.radius.md),
        theme::color(theme.content.canvas),
    );
    let chrome = egui::Rect::from_min_size(
        preview.min,
        Vec2::new(preview.width() / 4.0, preview.height()),
    );
    p.rect_filled(
        chrome,
        ui::radius(t.m.radius.md),
        theme::color(theme.chrome.base),
    );
    let line = |y: f32, w: f32, c| {
        let r = egui::Rect::from_min_size(
            egui::pos2(chrome.right() + t.m.space.sm, preview.top() + y),
            Vec2::new(w, t.m.space.xs + 2.0),
        );
        p.rect_filled(r, ui::radius(t.m.radius.sm), c);
    };
    line(
        t.m.space.md,
        preview.width() * 0.4,
        theme::color(theme.content.text_strong),
    );
    line(
        t.m.space.xl,
        preview.width() * 0.55,
        theme::color(theme.content.text_muted),
    );
    let button = egui::Rect::from_min_size(
        egui::pos2(
            chrome.right() + t.m.space.sm,
            preview.bottom() - t.m.space.xl,
        ),
        Vec2::new(t.m.space.xl * 1.5, t.m.space.md),
    );
    p.rect_filled(
        button,
        ui::radius(t.m.radius.sm),
        theme::color(theme.content.inverse_bg),
    );
    p.rect_stroke(
        rect,
        ui::radius(t.m.radius.md),
        stroke,
        egui::StrokeKind::Inside,
    );
    p.text(
        egui::pos2(
            rect.left() + t.m.space.sm,
            rect.bottom() - t.m.rows.palette / 2.0,
        ),
        egui::Align2::LEFT_CENTER,
        theme.name.as_ref(),
        ui::font(&t.type_scale.body_sm),
        if chosen {
            t.text_strong
        } else {
            t.text_secondary
        },
    );
    response
}

/// The first sentence of a command's doc comment.
fn first_sentence(name: CommandName) -> String {
    let text = name
        .spec()
        .description()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let end = text.find(". ").map_or(text.len(), |at| at + 1);
    text.get(..end).unwrap_or_default().to_owned()
}

fn keys(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    filter: &mut String,
    queue: &mut Vec<Request>,
) -> Option<Ask> {
    let mut ask = None;
    row(ui, t, "Preset", "The keys everything starts from", |ui| {
        let presets = [KeyPreset::Gmail, KeyPreset::Vim];
        let chosen = presets
            .iter()
            .position(|p| *p == state.settings.key_preset)
            .unwrap_or(0);
        if let Some(i) = ui::segmented(ui, t, &["Gmail", "Vim"], chosen)
            && i != chosen
            && let Some(preset) = presets.get(i)
        {
            queue.push(Request::user(Command::SetKeyPreset { preset: *preset }));
        }
    });
    ui.add_space(t.m.space.md);
    ui::field(t).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui::icon(ui, Icon::Search, t.m.controls.icon_sm, t.text_muted, false);
            ui.add(
                egui::TextEdit::singleline(filter)
                    .hint_text("Filter the commands")
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY),
            );
        });
    });
    ui.add_space(t.m.space.md);
    let query = filter.trim().to_lowercase();
    for (category, title) in [
        (Category::Mail, "Mail"),
        (Category::Compose, "Compose"),
        (Category::Navigation, "Navigation"),
        (Category::Account, "Account"),
        (Category::App, "App"),
    ] {
        let names: Vec<(CommandName, Vec<String>)> = CommandName::ALL
            .iter()
            .copied()
            .filter(|n| n.spec().category == category)
            .map(|n| (n, app_front::keymap::keys_for(&state.settings, n)))
            .filter(|(n, keys)| {
                !keys.is_empty()
                    && (query.is_empty() || n.spec().title.to_lowercase().contains(&query))
            })
            .collect();
        if names.is_empty() {
            continue;
        }
        ui::caption(ui, t, title);
        ui.add_space(t.m.space.xs);
        for (name, keys) in names {
            ui.horizontal(|ui| {
                ui.set_min_height(t.m.rows.palette);
                ui.vertical(|ui| {
                    ui.label(ui::text(
                        &t.type_scale.body_md,
                        name.spec().title,
                        t.text_strong,
                    ));
                    ui.label(ui::text(
                        &t.type_scale.body_sm,
                        first_sentence(name),
                        t.text_muted,
                    ));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let change = ui::text(&t.type_scale.label_md, "Change…", t.text_secondary);
                    let response = ui::ghost(ui, t, change);
                    let label = format!("Change the keys of {}", name.spec().title);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, &label)
                    });
                    if response.clicked()
                        && let Flow::Ask(step) =
                            app_front::palette::flow(CommandName::SetKeybinding, state)
                    {
                        ask = Some(step);
                    }
                    for key in keys.iter().rev() {
                        ui::kbd(ui, t, key);
                    }
                });
            });
            ui::divider(ui, t);
        }
        ui.add_space(t.m.space.md);
    }
    ask
}

fn general(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
    row(
        ui,
        t,
        "Undo send",
        "How long a sent message waits before it leaves",
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::UndoSendMs,
                &[
                    (0, "Off"),
                    (5_000, "5 s"),
                    (10_000, "10 s"),
                    (30_000, "30 s"),
                ],
                queue,
            );
        },
    );
    row(
        ui,
        t,
        "Check for mail",
        "How often the inbox is polled without IDLE",
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::PollSecs,
                &[
                    (60, "1 min"),
                    (120, "2 min"),
                    (300, "5 min"),
                    (900, "15 min"),
                ],
                queue,
            );
        },
    );
    row(
        ui,
        t,
        "Mark as read",
        "When an open conversation is marked read (other values: set_setting in the palette)",
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::MarkReadAfterMs,
                &[
                    (0, "Never"),
                    (250, "Immediately"),
                    (2_000, "After 2 s"),
                    (5_000, "After 5 s"),
                ],
                queue,
            );
        },
    );
    row(
        ui,
        t,
        "Notifications",
        "New mail, failed sends and agent requests while dail is in the background",
        |ui| {
            let on = state.settings.notifications == 1;
            if ui::switch(ui, t, on, "Notifications").clicked() {
                queue.push(Request::user(Command::SetSetting {
                    key: SettingKey::Notifications,
                    value: u64::from(!on).into(),
                }));
            }
        },
    );
    row(
        ui,
        t,
        "Undo history",
        "How many steps undo remembers",
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::HistoryCap,
                &[(100, "100"), (200, "200"), (500, "500"), (1_000, "1000")],
                queue,
            );
        },
    );
}
