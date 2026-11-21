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
//!
//! *Connect to a dail server instead* (ADR 0017, SRV-6; [`Wizard::connect`]) replaces the mail
//! account with a server this profile mirrors: it draws `app_front::connect::ConnectFlow`
//! (address or pasted `dail://pair` link → `connect_server` → the offered fingerprint to compare
//! and the code → `trust_server` → paired). Offered only to a profile without accounts.

use app_core::{AccountId, AccountStatus, AppState, Command, Endpoint, ProviderPreset, Secret};
use app_front::connect::{Availability, ConnectFlow, Retry, Stage, availability};
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
    /// *Connect to a dail server* instead of an account (SRV-6).
    Server,
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
    /// *Connect to a dail server* (the code it holds is never shown in `Debug`).
    flow: ConnectFlow,
    /// The clock of the last frame, for the flow's replies.
    now_ms: i64,
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
            flow: ConnectFlow::default(),
            now_ms: 0,
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

    /// Opens the wizard at *Connect to a dail server* (the settings' button).
    pub fn connect(&mut self) {
        *self = Self {
            open: true,
            offered: true,
            step: Step::Server,
            ..Self::default()
        };
    }

    /// The *Connect to a dail server* flow.
    pub const fn connect_flow(&self) -> &ConnectFlow {
        &self.flow
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
        if reply.purpose == Purpose::Background(Background::Connect) {
            let result = match &reply.result {
                Ok(Ok(receipt)) => Ok(receipt),
                Ok(Err(err)) => Err(app_front::i18n::message(&err.to_string())),
                Err(err) => Err(format!("dail: {err}")),
            };
            self.flow.accept(result, self.now_ms);
            return;
        }
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
        now_ms: i64,
        queue: &mut Vec<Request>,
    ) {
        self.now_ms = now_ms;
        // A device of a dail server has its accounts once the server's state arrives.
        if !self.offered && state.accounts.is_empty() && state.link.is_none() {
            self.open();
        }
        if !self.open {
            return;
        }
        if self.step == Step::Server {
            self.flow.sync(state, now_ms);
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
                } else if self.step == Step::Server {
                    t.s(Key::ConnectToServer)
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
                    (_, Step::Server) => self.flow.server(),
                };
                ui::caption(ui, t, &step);
                ui.add_space(t.m.space.md);
                match self.step {
                    Step::Preset => self.preset_step(ui, t, state),
                    Step::Address => self.address_step(ui, t, state, queue),
                    Step::Password => self.password_step(ui, t, queue),
                    Step::Test => self.test_step(ui, t, state, queue),
                    Step::Server => self.server_step(ui, t, state, queue),
                }
            });
        if modal.should_close() && self.step != Step::Test {
            if self.step == Step::Server {
                self.give_up(state, queue);
            }
            self.open = false;
            self.password.clear();
        }
    }

    /// Closes *Connect to a dail server*: a link it left unpinned is removed.
    fn give_up(&mut self, state: &AppState, queue: &mut Vec<Request>) {
        if let Some(cmd) = self.flow.cancel(state) {
            queue.push(Request {
                purpose: Purpose::Background(Background::Connect),
                cmd,
            });
        }
        self.flow = ConnectFlow::default();
        self.open = false;
    }

    /// *Connect to a dail server* (SRV-6): the stages of `app_front::connect::ConnectFlow`.
    fn server_step(&mut self, ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
        let muted = |ui: &mut Ui, text: &str| {
            ui.label(
                RichText::new(text)
                    .font(ui::font(&t.type_scale.body_sm))
                    .color(t.text_muted),
            );
        };
        let fingerprint = |ui: &mut Ui, label: &str, fp: &str| {
            ui.label(ui::text(&t.type_scale.label_md, label, t.text_secondary));
            for line in app_front::connect::fingerprint_lines(fp) {
                ui.add(
                    egui::Label::new(
                        RichText::new(line)
                            .font(ui::mono(t.type_scale.body_md.size))
                            .color(t.text_strong),
                    )
                    .selectable(true),
                );
            }
        };
        let server = self.flow.server();
        let mut submit = None;
        let mut cancel = false;
        match self.flow.stage().clone() {
            Stage::Address => {
                let label = ui.label(ui::text(
                    &t.type_scale.label_md,
                    t.s(Key::ConnectServerLabel),
                    t.text_secondary,
                ));
                let before = self.flow.address.chars().count();
                let response = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.flow.address)
                            .hint_text("home.example.org:7443")
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(label.id);
                if response.changed() {
                    self.flow.address_edited();
                    // A paste (more than one character at once) is read at once if it is a
                    // pairing link; a link typed by hand is read on *Connect*.
                    if self.flow.address.chars().count() > before + 1 {
                        self.flow.read_address();
                    }
                }
                muted(ui, t.s(Key::ConnectAddressHint));
                if let Some(note) = self.flow.note() {
                    muted(ui, note);
                }
                let problem = self.flow.address_problem();
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(t.m.space.md);
                ui.horizontal(|ui| {
                    if ui::secondary(ui, t, t.s(Key::Back), None, true) {
                        self.step = Step::Preset;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ready = problem.is_none();
                        if ui::primary(ui, t, t.s(Key::Connect), None, ready) || (enter && ready) {
                            submit = self.flow.submit_address(state);
                        }
                        if let Some(problem) = &problem
                            && !self.flow.address.trim().is_empty()
                        {
                            muted(ui, problem);
                        }
                    });
                });
            }
            Stage::Probing | Stage::Pairing => {
                let text = if self.flow.stage() == &Stage::Probing {
                    t.f(Key::ConnectProbing, &[("server", &server)])
                } else {
                    t.f(Key::ConnectPairing, &[("server", &server)])
                };
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(text);
                });
                ui.add_space(t.m.space.md);
                if ui::secondary(ui, t, t.s(Key::Cancel), Some("Esc"), true) {
                    cancel = true;
                }
            }
            Stage::Compare {
                fingerprint: offered,
                matches_link,
            } => {
                ui.label(ui::text(
                    &t.type_scale.body_md,
                    t.f(Key::ConnectCompare, &[("server", &server)]),
                    t.text,
                ));
                ui.add_space(t.m.space.xs);
                fingerprint(ui, t.s(Key::PairingFingerprint), &offered);
                if matches_link {
                    ui::icon_label(
                        ui,
                        t,
                        app_theme::Icon::Check,
                        t.s(Key::ConnectMatchesLink),
                        t.text_strong,
                        t.s(Key::ConnectMatchesLink),
                    );
                }
                ui.add_space(t.m.space.sm);
                let label = ui.label(ui::text(
                    &t.type_scale.label_md,
                    t.s(Key::PairingCode),
                    t.text_secondary,
                ));
                let response = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.flow.code)
                            .hint_text("ABCD2345")
                            .font(ui::mono(t.type_scale.body_md.size))
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(label.id);
                muted(ui, t.s(Key::ConnectCodeHint));
                let problem = self.flow.code_problem();
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(t.m.space.md);
                ui.horizontal(|ui| {
                    if ui::secondary(ui, t, t.s(Key::Back), None, true) {
                        self.flow.back();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ready = problem.is_none();
                        if ui::primary(ui, t, t.s(Key::Pair), None, ready) || (enter && ready) {
                            submit = self.flow.submit_code(state);
                        }
                        if let Some(problem) = &problem
                            && !self.flow.code.trim().is_empty()
                        {
                            muted(ui, problem);
                        }
                    });
                });
            }
            Stage::Mismatch { offered, expected } => {
                ui.colored_label(t.danger, t.f(Key::ConnectMismatch, &[("server", &server)]));
                ui.add_space(t.m.space.xs);
                fingerprint(ui, t.s(Key::PairingFingerprint), &offered);
                fingerprint(ui, t.s(Key::ConnectLinkFingerprint), &expected);
                ui.add_space(t.m.space.md);
                ui.horizontal(|ui| {
                    if ui::secondary(ui, t, t.s(Key::Cancel), Some("Esc"), true) {
                        cancel = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::primary(ui, t, t.s(Key::Back), None, true) {
                            self.flow.retry();
                        }
                    });
                });
            }
            Stage::Paired => {
                ui.label(ui::text(
                    &t.type_scale.body_md,
                    t.f(Key::ConnectPaired, &[("server", &server)]),
                    t.text_strong,
                ));
                if let Some(summary) = app_front::server::summary(state, t.lang) {
                    muted(ui, &summary.text);
                }
                ui.add_space(t.m.space.md);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::primary(ui, t, t.s(Key::Close), None, true) {
                            self.flow = ConnectFlow::default();
                            self.open = false;
                        }
                    });
                });
            }
            Stage::Failed { error, retry } => {
                ui.colored_label(t.danger, app_front::text::clean(&error));
                ui.add_space(t.m.space.md);
                ui.horizontal(|ui| {
                    if ui::secondary(ui, t, t.s(Key::Cancel), Some("Esc"), true) {
                        cancel = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let again = match retry {
                            Retry::Address => t.s(Key::TryAgain),
                            Retry::Code => t.s(Key::AnotherCode),
                        };
                        if ui::primary(ui, t, again, None, true) {
                            self.flow.retry();
                        }
                    });
                });
            }
        }
        if let Some(cmd) = submit {
            queue.push(Request {
                purpose: Purpose::Background(Background::Connect),
                cmd,
            });
        }
        if cancel {
            self.give_up(state, queue);
        }
    }

    fn preset_step(&mut self, ui: &mut Ui, t: &Tokens, state: &AppState) {
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
        // SRV-6: a profile without accounts can mirror a dail server instead.
        if availability(state) == Availability::Open {
            ui.add_space(t.m.space.xs);
            if ui::secondary(ui, t, t.s(Key::ConnectToServerInstead), None, true) {
                self.flow = ConnectFlow::default();
                self.step = Step::Server;
            }
            ui.label(
                RichText::new(t.s(Key::ConnectToServerDetail))
                    .font(ui::font(&t.type_scale.body_sm))
                    .color(t.text_muted),
            );
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
