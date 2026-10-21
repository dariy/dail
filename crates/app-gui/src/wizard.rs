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

const PRESETS: &[(ProviderPreset, &str, &str)] = &[
    (
        ProviderPreset::Gmail,
        "Gmail",
        "Needs an App Password (2-Step Verification on).",
    ),
    (
        ProviderPreset::Hostinger,
        "Hostinger Mail",
        "Your mailbox password.",
    ),
    (
        ProviderPreset::Custom,
        "Other IMAP / SMTP",
        "Enter the servers yourself.",
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
                Ok(Err(err)) => self.fail(err.to_string()),
                Err(err) => self.fail(err.to_string()),
            }
            return;
        }
        if reply.purpose == Purpose::Background(Background::WizardLookup) {
            // A refused lookup ends the wait; its answer otherwise arrives in the state.
            let refused = match &reply.result {
                Ok(Ok(_)) => None,
                Ok(Err(err)) => Some(err.to_string()),
                Err(err) => Some(err.to_string()),
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
                    Err(err) => self.fail(format!("unexpected receipt: {err}")),
                }
            }
            Ok(Err(err)) => self.fail(err.to_string()),
            Err(err) => self.fail(err.to_string()),
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
            .frame(
                egui::Frame::new()
                    .fill(t.base)
                    .stroke(egui::Stroke::new(1.0, t.border_strong))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::same(20)),
            )
            .backdrop_color(egui::Color32::from_black_alpha(90))
            .show(ctx, |ui| {
                ui.set_width(440.0);
                let title = if self.existing {
                    "Change the password"
                } else {
                    "Add an account"
                };
                ui.label(
                    RichText::new(title)
                        .size(20.0)
                        .strong()
                        .family(crate::theme::semibold())
                        .color(t.text_strong),
                );
                let step = match (self.existing, self.step) {
                    (true, Step::Test) => format!("{}  Connection", self.email),
                    (true, _) => self.email.clone(),
                    (false, Step::Preset) => "1 / 4  Provider".to_owned(),
                    (false, Step::Address) => "2 / 4  Address".to_owned(),
                    (false, Step::Password) => "3 / 4  Password".to_owned(),
                    (false, Step::Test) => "4 / 4  Connection".to_owned(),
                };
                ui::caption(ui, t, &step);
                ui.add_space(12.0);
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
                egui::Button::new(
                    RichText::new(*title)
                        .strong()
                        .family(crate::theme::semibold())
                        .color(t.text_strong),
                )
                .selected(chosen)
                .stroke(egui::Stroke::new(
                    1.0,
                    if chosen {
                        t.text_strong
                    } else {
                        t.border_subtle
                    },
                ))
                .min_size(egui::vec2(ui.available_width(), 30.0)),
            );
            if response.clicked() {
                self.preset = *preset;
            }
            ui.label(RichText::new(*hint).size(11.5).color(t.text_muted));
            ui.add_space(6.0);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui::secondary(ui, t, "Cancel", Some("Esc"), true) {
                self.open = false;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::primary(ui, t, "Next", None, true) {
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
        let label = ui.label(RichText::new(label).color(t.text_secondary));
        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id)
    }

    fn address_problem(&self) -> Option<String> {
        let email = self.email.trim();
        if app_front::compose::parse_addr(email).is_none_or(|a| a.name.is_some()) {
            return Some("Enter an email address like you@example.com.".to_owned());
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
        Self::field(ui, t, "Email address", &mut self.email, "you@example.com");
        ui.add_space(6.0);
        Self::field(
            ui,
            t,
            "Name (optional)",
            &mut self.display_name,
            "shown in the sidebar",
        );
        if self.preset == ProviderPreset::Custom {
            ui.add_space(6.0);
            Self::field(ui, t, "IMAP server", &mut self.imap, "imap.example.com:993");
            ui.add_space(6.0);
            Self::field(ui, t, "SMTP server", &mut self.smtp, "smtp.example.com:465");
            ui.label(
                RichText::new("host:port, optionally :tls or :starttls (143 and 587 use STARTTLS)")
                    .size(11.0)
                    .color(t.text_muted),
            );
            ui.add_space(6.0);
            self.take_discovery(state);
            let email = self.email.trim().to_owned();
            let valid = app_front::compose::parse_addr(&email).is_some_and(|a| a.name.is_none());
            ui.horizontal(|ui| {
                if ui::secondary(ui, t, "Find servers", None, valid && self.looking.is_none()) {
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
                    _ if self.looking.is_some() => "Looking…".to_owned(),
                    (_, Some(error)) => error.clone(),
                    (Some(d), None) if d.email == email => match &d.found {
                        Ok(found) => format!("Found at {}", found.source),
                        Err(error) => format!("Not found ({error}); enter them yourself."),
                    },
                    (None, None) | (Some(_), None) => {
                        "Asks the provider's web server, then Mozilla's list.".to_owned()
                    }
                };
                ui.label(RichText::new(note).size(11.0).color(t.text_muted));
            });
        }
        let problem = self.address_problem();
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui::secondary(ui, t, "Back", None, true) {
                self.step = Step::Preset;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::primary(ui, t, "Next", None, problem.is_none()) {
                    self.step = Step::Password;
                }
                if let Some(problem) = &problem
                    && !self.email.is_empty()
                {
                    ui.label(RichText::new(problem).size(11.0).color(t.text_muted));
                }
            });
        });
    }

    fn password_step(&mut self, ui: &mut Ui, t: &Tokens, queue: &mut Vec<Request>) {
        let label = if self.preset == ProviderPreset::Gmail {
            "App Password"
        } else {
            "Password"
        };
        let label = ui.label(RichText::new(label).color(t.text_secondary));
        let response = ui
            .add(
                egui::TextEdit::singleline(&mut self.password)
                    .password(true)
                    .desired_width(f32::INFINITY),
            )
            .labelled_by(label.id);
        ui.label(
            RichText::new("Stored in the system keyring only; dail never writes it anywhere else.")
                .size(11.0)
                .color(t.text_muted),
        );
        if let Some(error) = &self.error {
            ui.add_space(6.0);
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.add_space(12.0);
        let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            let back = if self.existing { "Cancel" } else { "Back" };
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
                    "Save and test"
                } else {
                    "Add and test"
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
                        RichText::new(format!("Signing in to {}…", self.email.trim()))
                            .color(t.text_strong),
                    );
                });
            }
            Some(AccountStatus::Verified { .. }) => {
                ui::icon_label(
                    ui,
                    t,
                    app_theme::Icon::Check,
                    "Connected. IMAP and SMTP both accepted the login.",
                    t.text_strong,
                    "Connected. IMAP and SMTP both accepted the login.",
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::primary(ui, t, "Sync now", None, true)
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
                    ui.visuals().error_fg_color,
                    format!("The login failed: {error}"),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let keep = if self.existing {
                        "Close"
                    } else {
                        "Keep it anyway"
                    };
                    if ui::secondary(ui, t, keep, None, true) {
                        self.open = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::primary(ui, t, "Try another password", None, true) {
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
