//! The settings page (UI-12): `view.settings` shows it in place of the list and the reader, with
//! tabs for Appearance, Keyboard, General and Accounts. It only shows the settings; every
//! control sends a command (`set_theme`, `set_setting`, `set_key_preset`, …), so agents and the
//! CLI change the same things.

use app_core::{
    AccountId, AccountStatus, AppState, Category, Command, CommandName, KeyPreset, SettingKey,
    SettingsSection, ThemeMode,
};
use app_front::i18n::Key;
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
    /// Open the palette at this step (*Change…* on a key binding, *Install…* of an update).
    pub ask: Option<Ask>,
    /// Open this release page in the browser (*Release notes* of an update).
    pub open: Option<String>,
}

/// The tabs, in order, with their icons and titles.
const TABS: [(SettingsSection, Icon, Key); 5] = [
    (SettingsSection::Appearance, Icon::Contrast, Key::Appearance),
    (SettingsSection::Keys, Icon::Keyboard, Key::Keyboard),
    (SettingsSection::General, Icon::Settings, Key::General),
    (SettingsSection::Accounts, Icon::Person, Key::Accounts),
    (SettingsSection::About, Icon::Info, Key::About),
];

impl SettingsState {
    /// Renders the page for `section`.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        state: &AppState,
        section: SettingsSection,
        now_ms: i64,
        queue: &mut Vec<Request>,
    ) -> SettingsActions {
        let mut actions = SettingsActions::default();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.m.space.sm;
            ui.label(ui::text(
                &t.type_scale.headline_lg,
                t.s(Key::Settings),
                t.text_strong,
            ));
            ui::badge(
                ui,
                t,
                concat!("DAIL ", env!("CARGO_PKG_VERSION")),
                ui::Badge::Outline,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::icon_button(
                    ui,
                    t,
                    t.text_secondary,
                    Icon::Close,
                    t.s(Key::CloseSettings),
                )
                .clicked()
                {
                    queue.push(Request::user(Command::CloseSettings {}));
                }
            });
        });
        ui.add_space(t.m.space.md);
        let items: Vec<(Icon, &str)> = TABS
            .iter()
            .map(|(_, icon, title)| (*icon, t.s(*title)))
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
                    SettingsSection::General => {
                        general(ui, t, state, queue);
                        updates(ui, t, state, now_ms, &mut actions, queue);
                        synced(ui, t, state, &mut actions);
                    }
                    SettingsSection::Accounts => {
                        self.accounts(ui, t, state, &mut actions, queue);
                    }
                    SettingsSection::About => about(ui, t, &mut actions),
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
                            AccountStatus::Unverified => t.s(Key::NotTestedYet).to_owned(),
                            AccountStatus::Verified { .. } => t.s(Key::Connected).to_owned(),
                            AccountStatus::Failed { error, .. } => t.f(
                                Key::FailedError,
                                &[("error", &app_front::i18n::message(error))],
                            ),
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
                    if ui::secondary(ui, t, t.s(Key::Test), None, true) {
                        queue.push(Request::user(Command::TestAccount { id: account.id }));
                    }
                    if ui::secondary(ui, t, t.s(Key::ReEnterPassword), None, true) {
                        actions.password = Some(account.id);
                    }
                    if self.confirm_remove == Some(account.id) {
                        let remove = ui::text(
                            &t.type_scale.body_md,
                            t.f(Key::RemoveAccountCache, &[("email", &account.email)]),
                            t.danger,
                        );
                        if ui::ghost(ui, t, remove).clicked() {
                            queue.push(Request::user(Command::RemoveAccount {
                                id: account.id,
                                confirm: true,
                            }));
                            self.confirm_remove = None;
                        }
                        let keep =
                            ui::text(&t.type_scale.body_md, t.s(Key::KeepIt), t.text_secondary);
                        if ui::ghost(ui, t, keep).clicked() {
                            self.confirm_remove = None;
                        }
                    } else {
                        let remove = ui::text(
                            &t.type_scale.body_md,
                            t.s(Key::RemoveDots),
                            t.text_secondary,
                        );
                        if ui::ghost(ui, t, remove).clicked() {
                            self.confirm_remove = Some(account.id);
                        }
                    }
                });
            });
            ui.add_space(t.m.space.sm);
        }
        if ui::primary(ui, t, t.s(Key::AddAccount), None, true) {
            actions.wizard = true;
        }
    }
}

/// A row of the page: a label and a description on the left, the control on the right.
fn row(ui: &mut Ui, t: &Tokens, label: &str, detail: &str, control: impl FnOnce(&mut Ui)) {
    split(
        ui,
        t,
        label,
        t.m.rows.palette + t.m.space.md,
        |ui| {
            ui.label(ui::text(&t.type_scale.body_md, label, t.text_strong));
            if !detail.is_empty() {
                ui.label(ui::text(&t.type_scale.body_sm, detail, t.text_muted));
            }
        },
        control,
    );
    ui::divider(ui, t);
}

/// The narrowest the title and detail of a row may get before its control moves below them.
const TEXT_MIN: f32 = 200.0;

/// Lays out `text` on the left and `control` on the right when both fit (MISC-38); otherwise
/// the control wraps to its own line below the text, still right-aligned. The control's width comes
/// from the previous frame, kept in egui's memory under `key`; a change asks for one more frame.
fn split(
    ui: &mut Ui,
    t: &Tokens,
    key: &str,
    min_height: f32,
    text: impl FnOnce(&mut Ui),
    control: impl FnOnce(&mut Ui),
) {
    let id = ui.scope_id().with(("settings-row", key));
    let known = ui.data(|d| d.get_temp::<f32>(id));
    let width = known.unwrap_or(0.0);
    let gap = t.m.space.md;
    let available = ui.available_width();
    let used = if width + gap + TEXT_MIN <= available {
        ui.horizontal(|ui| {
            ui.set_min_height(min_height);
            let text_width = (available - width - gap).max(TEXT_MIN);
            ui.allocate_ui_with_layout(
                egui::vec2(text_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_max_width(text_width);
                    text(ui);
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control)
                .response
                .rect
                .width()
        })
        .inner
    } else {
        ui.vertical(|ui| {
            ui.set_min_height(min_height);
            ui.add_space(t.m.space.xs);
            text(ui);
            ui.add_space(t.m.space.xs);
            // Right to left in both layouts, so a control's order does not change when it wraps.
            let used = ui
                .horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control)
                        .response
                        .rect
                        .width()
                })
                .inner;
            ui.add_space(t.m.space.xs);
            used
        })
        .inner
    };
    if known.is_none_or(|w| (w - used).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(id, used));
        ui.ctx().request_repaint();
    }
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
    row(ui, t, t.s(Key::Theme), t.s(Key::ThemeHint), |ui| {
        let modes = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
        let chosen = modes
            .iter()
            .position(|m| *m == ThemeMode::of(settings.theme))
            .unwrap_or(0);
        if let Some(i) = ui::segmented(
            ui,
            t,
            &[t.s(Key::System), t.s(Key::Light), t.s(Key::Dark)],
            chosen,
        ) && i != chosen
        {
            queue.push(Request::user(Command::SetTheme {
                mode: modes.get(i).copied(),
                light: None,
                dark: None,
            }));
        }
    });
    for (appearance, title, current) in [
        (
            Appearance::Light,
            t.s(Key::LightTheme),
            &settings.theme_light,
        ),
        (Appearance::Dark, t.s(Key::DarkTheme), &settings.theme_dark),
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
        t.s(Key::LanguageSetting),
        t.s(Key::LanguageHint),
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::Language,
                &app_front::i18n::languages(t.lang),
                queue,
            );
        },
    );
    row(ui, t, t.s(Key::Density), t.s(Key::DensityHint), |ui| {
        choices(
            ui,
            t,
            state,
            SettingKey::ListDensity,
            &[
                (0, t.s(Key::Cozy)),
                (1, t.s(Key::Compact)),
                (2, t.s(Key::PreviewDensity)),
            ],
            queue,
        );
    });
    row(ui, t, t.s(Key::TextSize), t.s(Key::TextSizeHint), |ui| {
        choices(
            ui,
            t,
            state,
            SettingKey::TextScale,
            &[(90, "90 %"), (100, "100 %"), (125, "125 %"), (150, "150 %")],
            queue,
        );
    });
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

fn keys(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    filter: &mut String,
    queue: &mut Vec<Request>,
) -> Option<Ask> {
    let mut ask = None;
    row(ui, t, t.s(Key::Preset), t.s(Key::PresetHint), |ui| {
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
                    .hint_text(t.s(Key::FilterCommands))
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY),
            );
        });
    });
    ui.add_space(t.m.space.md);
    let query = filter.trim().to_lowercase();
    for (category, title) in [
        (Category::Mail, t.s(Key::Mail)),
        (Category::Compose, t.s(Key::Compose)),
        (Category::Navigation, t.s(Key::Navigation)),
        (Category::Account, t.s(Key::Account)),
        (Category::App, t.s(Key::App)),
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
            let title = app_front::i18n::title(name);
            split(
                ui,
                t,
                title,
                t.m.rows.palette,
                |ui| {
                    ui.label(ui::text(&t.type_scale.body_md, title, t.text_strong));
                    ui.label(ui::text(
                        &t.type_scale.body_sm,
                        app_front::i18n::help(name),
                        t.text_muted,
                    ));
                },
                |ui| {
                    let change =
                        ui::text(&t.type_scale.label_md, t.s(Key::Change), t.text_secondary);
                    let response = ui::ghost(ui, t, change);
                    let label = t.f(Key::ChangeKeysOf, &[("name", title)]);
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
                },
            );
            ui::divider(ui, t);
        }
        ui.add_space(t.m.space.md);
    }
    ask
}

fn general(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
    row(ui, t, t.s(Key::UndoSend), t.s(Key::UndoSendHint), |ui| {
        choices(
            ui,
            t,
            state,
            SettingKey::UndoSendMs,
            &[
                (0, t.s(Key::Off)),
                (5_000, "5 s"),
                (10_000, "10 s"),
                (30_000, "30 s"),
            ],
            queue,
        );
    });
    row(ui, t, t.s(Key::CheckMail), t.s(Key::CheckMailHint), |ui| {
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
    });
    row(
        ui,
        t,
        t.s(Key::MarkAsRead),
        t.s(Key::MarkAsReadHint),
        |ui| {
            choices(
                ui,
                t,
                state,
                SettingKey::MarkReadAfterMs,
                &[
                    (0, t.s(Key::Never)),
                    (250, t.s(Key::Immediately)),
                    (2_000, t.s(Key::After2s)),
                    (5_000, t.s(Key::After5s)),
                ],
                queue,
            );
        },
    );
    row(
        ui,
        t,
        t.s(Key::Notifications),
        t.s(Key::NotificationsHint),
        |ui| {
            let on = state.settings.notifications == 1;
            if ui::switch(ui, t, on, t.s(Key::Notifications)).clicked() {
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
        t.s(Key::UndoHistory),
        t.s(Key::UndoHistoryHint),
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

/// The update panel (Wave 7 F1): the running version, where an update stands with its buttons,
/// and the periodic checks with their interval.
fn updates(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    now_ms: i64,
    actions: &mut SettingsActions,
    queue: &mut Vec<Request>,
) {
    use app_front::updates::{Action, intervals, panel};

    ui.add_space(t.m.space.lg);
    ui::caption(ui, t, t.s(Key::Updates));
    let panel = panel(state, now_ms, env!("CARGO_PKG_VERSION"));
    let detail = match &panel.checked {
        Some(checked) => format!("{} · {checked}", panel.status),
        None => panel.status.clone(),
    };
    row(ui, t, &panel.version, &detail, |ui| {
        // Right to left: the first action ends up rightmost.
        for action in &panel.actions {
            if !ui::secondary(ui, t, action.label(), None, true) {
                continue;
            }
            match (action, action.flow(state)) {
                (Action::Open(url), _) => actions.open = Some(url.clone()),
                (_, Flow::Run(cmd)) => queue.push(Request::user(cmd)),
                (_, Flow::Ask(step)) => actions.ask = Some(step),
                _ => {}
            }
        }
    });
    let policy = &state.settings.updates;
    row(
        ui,
        t,
        t.s(Key::CheckAutomatically),
        t.s(Key::CheckAutomaticallyHint),
        |ui| {
            let on = policy.checks == app_core::UpdateChecks::On;
            if ui::switch(ui, t, on, t.s(Key::CheckAutomatically)).clicked() {
                queue.push(Request::user(app_front::updates::answer(!on)));
            }
        },
    );
    row(ui, t, t.s(Key::HowOften), t.s(Key::HowOftenHint), |ui| {
        let labels: Vec<&str> = intervals().iter().map(|(_, l)| *l).collect();
        let chosen = intervals()
            .iter()
            .position(|(h, _)| *h == policy.interval_hours)
            .unwrap_or(usize::MAX);
        if let Some(i) = ui::segmented(ui, t, &labels, chosen)
            && let Some((hours, _)) = intervals().get(i)
            && *hours != policy.interval_hours
        {
            queue.push(Request::user(Command::SetUpdatePolicy {
                checks: None,
                interval_hours: Some(*hours),
            }));
        }
    });
}

/// The settings on the mail server (ADR 0015, SET-3): which account holds them, with
/// *Change…*, and where the last pull or push stands, or its error with its code.
fn synced(ui: &mut Ui, t: &Tokens, state: &AppState, actions: &mut SettingsActions) {
    let Some(panel) = app_front::synced::panel(state) else {
        return;
    };
    ui.add_space(t.m.space.lg);
    ui::caption(ui, t, t.s(Key::SyncedSettings));
    let detail = format!("{}\n{}", panel.account_label, t.s(Key::SettingsAccountHint));
    row(ui, t, t.s(Key::SettingsAccount), &detail, |ui| {
        let more = state.accounts.len() > 1;
        if ui::secondary(ui, t, t.s(Key::Change), None, more)
            && let Flow::Ask(step) =
                app_front::palette::flow(CommandName::SetSettingsAccount, state)
        {
            actions.ask = Some(step);
        }
    });
    row(ui, t, t.s(Key::SettingsSyncState), &panel.status, |_| {});
}

/// The About section (MISC-27): the name, the version, the licence, the links and the texts of
/// what the desktop app bundles.
fn about(ui: &mut Ui, t: &Tokens, actions: &mut SettingsActions) {
    ui.label(ui::text(
        &t.type_scale.headline_md,
        concat!("dail ", env!("CARGO_PKG_VERSION")),
        t.text_strong,
    ));
    ui.add_space(t.m.space.xs);
    ui.label(ui::text(
        &t.type_scale.body_md,
        t.f(
            Key::AboutDesktopText,
            &[("licence", env!("CARGO_PKG_LICENSE"))],
        ),
        t.text_secondary,
    ));
    ui.add_space(t.m.space.md);
    let repository = env!("CARGO_PKG_REPOSITORY");
    for (title, url) in [
        (t.s(Key::SourceCode), repository.to_owned()),
        (t.s(Key::Releases), format!("{repository}/releases")),
    ] {
        if ui::ghost(ui, t, title).on_hover_text(&url).clicked() {
            actions.open = Some(url);
        }
    }
    ui.add_space(t.m.space.lg);
    let mut licences: Vec<(&str, &str)> = app_theme::fonts::fonts()
        .iter()
        .map(|font| (font.name, font.license))
        .collect();
    licences.push(("Material Symbols", app_theme::icon::ICON_LICENSE));
    for (name, text) in licences {
        egui::CollapsingHeader::new(name).show(ui, |ui| {
            ui.label(ui::text(&t.type_scale.body_sm, text, t.text_secondary));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// MISC-38: a row keeps its control beside the title when both fit, and moves it below the
    /// title, inside the page, when they do not.
    #[test]
    fn a_rows_control_wraps_below_the_title_when_narrow() {
        for (width, narrow) in [(600.0, false), (300.0, true)] {
            let mut h = Harness::builder()
                .with_size(egui::vec2(800.0, 400.0))
                .build_ui(move |ui| {
                    let t = crate::theme::tokens(ui.ctx());
                    ui.set_max_width(width);
                    split(
                        ui,
                        &t,
                        "row",
                        0.0,
                        |ui| {
                            ui.label("Title");
                        },
                        |ui| {
                            ui.add_sized([200.0, 20.0], egui::Button::new("Control"));
                        },
                    );
                });
            h.run();
            let title = h.get_by_label("Title").rect();
            let control = h.get_by_label("Control").rect();
            assert_eq!(control.min.y >= title.max.y, narrow, "{width}: {control:?}");
            assert!(control.max.x <= title.min.x + width, "{width}: {control:?}");
        }
    }
}
