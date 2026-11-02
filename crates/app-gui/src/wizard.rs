//! The account wizard: preset → address → password → test → sync.
//!
//! It submits the same commands as `dail account add`: `add_account` (the password goes straight
//! into a write-only `Secret` and on to the keyring; the field is cleared at once), then
//! `test_account`, whose result arrives in the account's `status`. A failed test can be retried
//! with another password: the half-added account is removed first (`remove_account` with
//! `confirm`, it was created by this wizard a moment ago). Once verified, *Sync now* runs
//! `sync_now` for the account and closes the wizard.
//!
//! The same form changes the password of an existing account ([`Wizard::reenter`], e.g. from a
//! failed login in the status bar): password → `set_password` (keyring, then a login test) →
//! the result; another try keeps the account.

use app_core::{AccountId, AccountStatus, AppState, Command, Endpoint, ProviderPreset, Secret};
use app_front::i18n::Key;
use egui::{RichText, Ui};

use crate::bridge::{Background, Purpose, Reply, Request};
use crate::theme::Tokens;
use crate::ui;

/// Where the wizard is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Pick a provider.
    Preset,
    /// Address, name and (custom) servers.
    Address,
    /// The password.
    Password,
    /// `add_account` + `test_account` sent; waiting for the status.
    Test,
}

/// UI-local wizard state.
#[derive(Debug)]
pub struct Wizard {
    open: bool,
    /// Whether it was opened automatically because there are no accounts (shown once).
    offered: bool,
    step: Step,
    preset: ProviderPreset,
    email: String,
    display_name: String,
    imap: String,
    smtp: String,
    password: String,
    account: Option<AccountId>,
    /// Changing the password of `account` (which exists) instead of adding one.
    existing: bool,
    error: Option<String>,
    /// The address whose servers were asked for (`discover_servers`) and not yet filled in.
    looking: Option<String>,
    /// Why the last `discover_servers` was refused.
    lookup_error: Option<String>,
    /// What was shown last frame: the modal re-sizes a frame late, so a change asks for one more.
    shown: Option<(Step, u8)>,
}

impl Default for Wizard {
    fn default() -> Self {
        Self {
            open: false,
            offered: false,
            step: Step::Preset,
            preset: ProviderPreset::Gmail,
            email: String::new(),
            display_name: String::new(),
            imap: String::new(),
            smtp: String::new(),
            password: String::new(),
            account: None,
            existing: false,
            error: None,
            looking: None,
            lookup_error: None,
            shown: None,
        }
    }
}

const PRESETS: &[(ProviderPreset, Key, Key)] = &[
    (
        ProviderPreset::Gmail,
        Key::PresetGmail,
        Key::PresetGmailHint,
    ),
    (
        ProviderPreset::Hostinger,
        Key::PresetHostinger,
        Key::PresetHostingerHint,
    ),
    (
        ProviderPreset::Custom,
        Key::PresetOther,
        Key::PresetCustomHint,
    ),
];

impl Wizard {
    /// Whether the wizard is shown.
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// The current step.
    pub const fn step(&self) -> Step {
        self.step
    }

    /// Opens the wizard at the first step.
    pub fn open(&mut self) {
        *self = Self {
            open: true,
            offered: true,
            ..Self::default()
        };
    }

    /// Opens the password step for `account`, to replace its password (`set_password`).
    pub fn reenter(&mut self, state: &AppState, account: AccountId) {
        let Some(found) = state.accounts.get(&account) else {
            return;
        };
        *self = Self {
            open: true,
            offered: true,
            step: Step::Password,
            preset: found.preset,
            email: found.email.clone(),
            account: Some(account),
            existing: true,
            ..Self::default()
        };
    }

    /// Takes the reply to the wizard's `add_account`.
    pub fn accept(&mut self, reply: &Reply, queue: &mut Vec<Request>) {
        if reply.purpose == Purpose::Background(Background::WizardPassword) {
            match &reply.result {
                Ok(Ok(_)) => {}
                Ok(Err(err)) => self.fail(app_front::i18n::message(&err.to_string())),
                Err(err) => self.fail(app_front::i18n::message(&err.to_string())),
            }
            return;
        }
        if reply.purpose == Purpose::Background(Background::WizardLookup) {
            // A refused lookup ends the wait; its answer otherwise arrives in the state.
            let refused = match &reply.result {
                Ok(Ok(_)) => None,
                Ok(Err(err)) => Some(app_front::i18n::message(&err.to_string())),
                Err(err) => Some(app_front::i18n::message(&err.to_string())),
            };
            if let Some(error) = refused {
                self.looking = None;
                self.lookup_error = Some(error);
            }
            return;
        }
        if reply.purpose != Purpose::Background(Background::WizardAdd) {
            return;
        }
        match &reply.result {
            Ok(Ok(receipt)) => {
                match serde_json::from_value::<AccountId>(receipt.data["id"].clone()) {
                    Ok(id) => {
                        self.account = Some(id);
                        queue.push(Request::user(Command::TestAccount { id }));
                    }
                    Err(err) => self.fail(app_front::i18n::f(
                        Key::UnexpectedReceipt,
                        &[("error", &err.to_string())],
                    )),
                }
            }
            Ok(Err(err)) => self.fail(app_front::i18n::message(&err.to_string())),
            Err(err) => self.fail(app_front::i18n::message(&err.to_string())),
        }
    }

    fn fail(&mut self, error: String) {
        self.error = Some(error);
        self.step = Step::Password;
    }

    /// Renders the wizard (opening it on first run when there are no accounts).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        t: &Tokens,
        state: &AppState,
        queue: &mut Vec<Request>,
    ) {
        if !self.offered && state.accounts.is_empty() {
            self.open();
        }
        if !self.open {
            return;
        }
        let status = match self
            .account
            .and_then(|id| state.accounts.get(&id))
            .map(|a| &a.status)
        {
            None | Some(AccountStatus::Unverified) => 0,
            Some(AccountStatus::Verified { .. }) => 1,
            Some(AccountStatus::Failed { .. }) => 2,
        };
        if self.shown.replace((self.step, status)) != Some((self.step, status)) {
            ctx.request_repaint();
        }
        let modal = egui::Modal::new(egui::Id::unique("account-wizard"))
            .frame(ui::overlay(t).inner_margin(egui::Margin::same(t.m.space.xl as i8)))
            .backdrop_color(t.scrim)
            .show(ctx, |ui| {
                ui.set_width(t.m.overlays.dialog_width - 2.0 * t.m.space.xl);
                let title = if self.existing {
                    t.s(Key::ChangePassword)
                } else {
                    t.s(Key::AddAnAccount)
                };
                ui.label(ui::text(&t.type_scale.headline_lg, title, t.text_strong));
                let step = match (self.existing, self.step) {
                    (true, Step::Test) => t.f(Key::TestConnection, &[("email", &self.email)]),
                    (true, _) => self.email.clone(),
                    (false, Step::Preset) => t.s(Key::Provider1).to_owned(),
                    (false, Step::Address) => t.s(Key::Address2).to_owned(),
                    (false, Step::Password) => t.s(Key::Password3).to_owned(),
                    (false, Step::Test) => t.s(Key::Connection4).to_owned(),
                };
                ui::caption(ui, t, &step);
                ui.add_space(t.m.space.md);
                match self.step {
                    Step::Preset => self.preset_step(ui, t),
                    Step::Address => self.address_step(ui, t, state, queue),
                    Step::Password => self.password_step(ui, t, queue),
                    Step::Test => self.test_step(ui, t, state, queue),
                }
            });
        if modal.should_close() && self.step != Step::Test {
            self.open = false;
            self.password.clear();
        }
    }

    fn preset_step(&mut self, ui: &mut Ui, t: &Tokens) {
        for (preset, title, hint) in PRESETS {
            let chosen = self.preset == *preset;
            let response = ui.add(
                // The presets as selectable cards: `raised`, the chosen one outlined in `focus`.
                egui::Button::new(ui::text_at(
                    &t.type_scale.body_md,
                    600,
                    t.s(*title),
                    t.text_strong,
                ))
                .selected(chosen)
                .fill(if chosen { t.selected } else { t.raised })
                .stroke(egui::Stroke::new(
                    t.m.stroke.hairline,
                    if chosen { t.focus } else { t.border_subtle },
                ))
                .corner_radius(ui::radius(t.m.radius.md))
                .min_size(egui::vec2(
                    ui.available_width(),
                    t.m.controls.button + t.m.space.xs,
                )),
            );
            if response.clicked() {
                self.preset = *preset;
            }
            ui.label(
                RichText::new(t.s(*hint))
                    .font(ui::font(&t.type_scale.body_sm))
                    .color(t.text_muted),
            );
            ui.add_space(t.m.space.xs + 2.0);
        }
        ui.add_space(t.m.space.sm);
        ui.horizontal(|ui| {
            if ui::secondary(ui, t, t.s(Key::Cancel), Some("Esc"), true) {
                self.open = false;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::primary(ui, t, t.s(Key::Next), None, true) {
                    self.step = Step::Address;
                }
            });
        });
    }

    fn field(
        ui: &mut Ui,
        t: &Tokens,
        label: &str,
        value: &mut String,
        hint: &str,
    ) -> egui::Response {
        let label = ui.label(ui::text(&t.type_scale.label_md, label, t.text_secondary));
        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id)
    }

    fn address_problem(&self, t: &Tokens) -> Option<String> {
        let email = self.email.trim();
        if app_front::compose::parse_addr(email).is_none_or(|a| a.name.is_some()) {
            return Some(t.s(Key::EnterEmail).to_owned());
        }
        if self.preset == ProviderPreset::Custom {
            for (name, text) in [("IMAP", &self.imap), ("SMTP", &self.smtp)] {
                if let Err(err) = Endpoint::parse(text.trim()) {
                    return Some(format!("{name}: {err}"));
                }
            }
        }
        None
    }

    /// Fills the server fields from `status.discovery` once the lookup of `looking` is answered.
    fn take_discovery(&mut self, state: &AppState) {
        let Some(discovery) = &state.status.discovery else {
            return;
        };
        if self.looking.as_deref() != Some(discovery.email.as_str()) {
            return;
        }
        self.looking = None;
        if let Ok(found) = &discovery.found {
            self.imap = found.imap.to_string();
            self.smtp = found.smtp.to_string();
        }
    }

    fn address_step(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        state: &AppState,
        queue: &mut Vec<Request>,
    ) {
        Self::field(
            ui,
            t,
            t.s(Key::EmailAddress),
            &mut self.email,
            "you@example.com",
        );
        ui.add_space(t.m.space.xs + 2.0);
        Self::field(
            ui,
            t,
            t.s(Key::NameOptional),
            &mut self.display_name,
            t.s(Key::ShownInSidebar),
        );
        if self.preset == ProviderPreset::Custom {
            ui.add_space(t.m.space.xs + 2.0);
            Self::field(
                ui,
                t,
                t.s(Key::ImapServer),
                &mut self.imap,
                "imap.example.com:993",
            );
            ui.add_space(t.m.space.xs + 2.0);
            Self::field(
                ui,
                t,
                t.s(Key::SmtpServer),
                &mut self.smtp,
                "smtp.example.com:465",
            );
            ui.label(
                RichText::new(t.s(Key::HostPortHint))
                    .font(ui::font(&t.type_scale.body_sm))
                    .color(t.text_muted),
            );
            ui.add_space(t.m.space.xs + 2.0);
            self.take_discovery(state);
            let email = self.email.trim().to_owned();
            let valid = app_front::compose::parse_addr(&email).is_some_and(|a| a.name.is_none());
            ui.horizontal(|ui| {
                if ui::secondary(
                    ui,
                    t,
                    t.s(Key::FindServers),
                    None,
                    valid && self.looking.is_none(),
                ) {
                    queue.push(Request {
                        purpose: Purpose::Background(Background::WizardLookup),
                        cmd: Command::DiscoverServers {
                            email: email.clone(),
                        },
                    });
                    self.looking = Some(email.clone());
                    self.lookup_error = None;
                }
                let note = match (&state.status.discovery, &self.lookup_error) {
                    _ if self.looking.is_some() => t.s(Key::Looking).to_owned(),
                    (_, Some(error)) => error.clone(),
                    (Some(d), None) if d.email == email => match &d.found {
                        Ok(found) => t.f(Key::FoundAt, &[("source", &found.source)]),
                        Err(error) => t.f(Key::NotFound, &[("error", error)]),
                    },
                    (None, None) | (Some(_), None) => t.s(Key::FindServersHint).to_owned(),
                };
                ui.label(
                    RichText::new(note)
                        .font(ui::font(&t.type_scale.body_sm))
                        .color(t.text_muted),
                );
            });
        }
        let problem = self.address_problem(t);
        ui.add_space(t.m.space.md);
        ui.horizontal(|ui| {
            if ui::secondary(ui, t, t.s(Key::Back), None, true) {
                self.step = Step::Preset;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::primary(ui, t, t.s(Key::Next), None, problem.is_none()) {
                    self.step = Step::Password;
                }
                if let Some(problem) = &problem
                    && !self.email.is_empty()
                {
                    ui.label(
                        RichText::new(problem)
                            .font(ui::font(&t.type_scale.body_sm))
                            .color(t.text_muted),
                    );
                }
            });
        });
    }

    fn password_step(&mut self, ui: &mut Ui, t: &Tokens, queue: &mut Vec<Request>) {
        let label = if self.preset == ProviderPreset::Gmail {
            t.s(Key::AppPassword)
        } else {
            t.s(Key::Password)
        };
        let label = ui.label(ui::text(&t.type_scale.label_md, label, t.text_secondary));
        let response = ui
            .add(
                egui::TextEdit::singleline(&mut self.password)
                    .password(true)
                    .desired_width(f32::INFINITY),
            )
            .labelled_by(label.id);
        ui.label(
            RichText::new(t.s(Key::KeyringOnly))
                .font(ui::font(&t.type_scale.body_sm))
                .color(t.text_muted),
        );
        if let Some(error) = &self.error {
            ui.add_space(t.m.space.xs + 2.0);
            ui.colored_label(t.danger, error);
        }
        ui.add_space(t.m.space.md);
        let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            let back = if self.existing {
                t.s(Key::Cancel)
            } else {
                t.s(Key::Back)
            };
            if ui::secondary(ui, t, back, None, true) {
                if self.existing {
                    self.open = false;
                    self.password.clear();
                } else {
                    self.step = Step::Address;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ready = !self.password.is_empty();
                let label = if self.existing {
                    t.s(Key::SaveAndTest)
                } else {
                    t.s(Key::AddAndTest)
                };
                if (ui::primary(ui, t, label, None, ready) || (submit && ready))
                    && let Some(request) = self.submit_password()
                {
                    queue.push(request);
                    self.error = None;
                    self.step = Step::Test;
                }
            });
        });
    }

    /// `set_password` for an existing account, else `add_account`; takes the password out of
    /// the field.
    fn submit_password(&mut self) -> Option<Request> {
        if self.existing {
            let id = self.account?;
            return Some(Request {
                purpose: Purpose::Background(Background::WizardPassword),
                cmd: Command::SetPassword {
                    id,
                    password: Secret::new(std::mem::take(&mut self.password)),
                },
            });
        }
        self.add_command().map(|cmd| Request {
            purpose: Purpose::Background(Background::WizardAdd),
            cmd,
        })
    }

    /// The `add_account` for the form; takes the password out of the field.
    fn add_command(&mut self) -> Option<Command> {
        let custom = self.preset == ProviderPreset::Custom;
        let endpoint = |text: &str| {
            if custom {
                Endpoint::parse(text.trim()).ok()
            } else {
                None
            }
        };
        let name = self.display_name.trim();
        Some(Command::AddAccount {
            preset: self.preset,
            email: self.email.trim().to_owned(),
            display_name: (!name.is_empty()).then(|| name.to_owned()),
            password: Some(Secret::new(std::mem::take(&mut self.password))),
            imap: endpoint(&self.imap),
            smtp: endpoint(&self.smtp),
        })
    }

    fn test_step(&mut self, ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
        let account = self.account.and_then(|id| state.accounts.get(&id));
        match account.map(|a| &a.status) {
            None | Some(AccountStatus::Unverified) => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t.f(Key::SigningIn, &[("email", self.email.trim())]))
                            .color(t.text_strong),
                    );
                });
            }
            Some(AccountStatus::Verified { .. }) => {
                ui::icon_label(
                    ui,
                    t,
                    app_theme::Icon::Check,
                    t.s(Key::ConnectedLogin),
                    t.text_strong,
                    t.s(Key::ConnectedLogin),
                );
                ui.add_space(t.m.space.md);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::primary(ui, t, t.s(Key::SyncNow), None, true)
                            && let Some(id) = self.account
                        {
                            queue.push(Request::user(Command::SyncNow { account: Some(id) }));
                            self.open = false;
                        }
                    });
                });
            }
            Some(AccountStatus::Failed { error, .. }) => {
                ui.colored_label(
                    t.danger,
                    t.f(
                        Key::LoginFailedError,
                        &[("error", &app_front::i18n::message(error))],
                    ),
                );
                ui.add_space(t.m.space.md);
                ui.horizontal(|ui| {
                    let keep = if self.existing {
                        t.s(Key::Close)
                    } else {
                        t.s(Key::KeepAnyway)
                    };
                    if ui::secondary(ui, t, keep, None, true) {
                        self.open = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::primary(ui, t, t.s(Key::TryAnotherPassword), None, true) {
                            if self.existing {
                                self.step = Step::Password;
                            } else if let Some(id) = self.account.take() {
                                queue.push(Request::user(Command::RemoveAccount {
                                    id,
                                    confirm: true,
                                }));
                                self.step = Step::Password;
                            }
                        }
                    });
                });
            }
        }
    }
}
