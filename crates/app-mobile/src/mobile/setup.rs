//! Account setup on the phone: provider → address (and servers for a custom one, *Find
//! servers*) → password → `add_account` → `test_account` → *Sync now*, like the desktop's
//! wizard and the TUI's.
//!
//! The password arrives as `text` events of the `password` field and goes straight into the
//! write-only `Secret` of `add_account` (or `set_password`), and on to the keyring; the field is
//! empty afterwards and no frame ever carries it. A failed test can be retried with another
//! password: the half-added account is removed first (`remove_account` with `confirm`; this
//! setup created it a moment ago). [`Setup::reenter`] opens the password step for an existing
//! account (a problem's *re-enter password* fix); another try keeps the account.

use app_core::{
    AccountId, AccountStatus, AppState, Command, CommandName, Endpoint, ProviderPreset, Secret,
};
use app_front::text::clean;

use super::{Act, Mobile};
use crate::event::Field;
use crate::frame::{Button, InputKind, PresetChoice, SetupField, SetupSheet, TextValue};
use crate::request::{Purpose, Reply, Request};

const PRESETS: [(ProviderPreset, &str, &str); 3] = [
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
        "Enter the servers yourself, or find them.",
    ),
];

/// Where the setup is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// Pick a provider.
    Preset,
    /// Address, name and (custom) servers.
    Address,
    /// The password.
    Password,
    /// `add_account` (or `set_password`) sent; waiting for the account's status.
    Test,
}

/// What a setup button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SetupAct {
    /// Open the setup at its first step.
    Open,
    /// Choose a provider.
    Preset(ProviderPreset),
    /// On to the next step.
    Next,
    /// Back a step, else close.
    Back,
    /// `discover_servers` for the typed address.
    Find,
    /// `add_account` or `set_password` with the typed password.
    Submit,
    /// Another password after a failed test.
    Retry,
    /// `sync_now` for the new account, and close.
    Sync,
}

/// The setup's state.
pub(super) struct Setup {
    step: Step,
    preset: ProviderPreset,
    email: String,
    name: String,
    imap: String,
    smtp: String,
    password: String,
    /// Changes whenever the presenter replaces a field's text (found servers, a cleared
    /// password).
    version: u64,
    account: Option<AccountId>,
    /// Changing the password of `account` (which exists) instead of adding one.
    existing: bool,
    error: Option<String>,
    /// The address whose servers were asked for and not yet filled in.
    looking: Option<String>,
    /// Why the last `discover_servers` was refused.
    lookup_error: Option<String>,
}

impl std::fmt::Debug for Setup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the password.
        f.debug_struct("Setup")
            .field("step", &self.step)
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

impl Setup {
    /// The first step.
    pub(super) fn new() -> Self {
        Self {
            step: Step::Preset,
            preset: ProviderPreset::Gmail,
            email: String::new(),
            name: String::new(),
            imap: String::new(),
            smtp: String::new(),
            password: String::new(),
            version: 1,
            account: None,
            existing: false,
            error: None,
            looking: None,
            lookup_error: None,
        }
    }

    /// The password step for `account`; `None` if there is no such account.
    pub(super) fn reenter(state: &AppState, account: AccountId) -> Option<Self> {
        let found = state.accounts.get(&account)?;
        Some(Self {
            step: Step::Password,
            preset: found.preset,
            email: found.email.clone(),
            account: Some(account),
            existing: true,
            ..Self::new()
        })
    }

    /// A field's typed text.
    pub(super) fn text(&mut self, field: Field, text: String) {
        match field {
            Field::Email => self.email = text,
            Field::Name => self.name = text,
            Field::Imap => self.imap = text,
            Field::Smtp => self.smtp = text,
            Field::Password => self.password = text,
            Field::To | Field::Cc | Field::Bcc | Field::Subject | Field::Body | Field::Query => {}
        }
    }

    fn address_problem(&self) -> Option<String> {
        if app_front::compose::parse_addr(self.email.trim()).is_none_or(|a| a.name.is_some()) {
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

    /// The account's status: 0 waiting, 1 verified, 2 failed (with the error).
    fn status(&self, state: &AppState) -> (u8, Option<String>) {
        match self
            .account
            .and_then(|id| state.accounts.get(&id))
            .map(|a| &a.status)
        {
            None | Some(AccountStatus::Unverified) => (0, None),
            Some(AccountStatus::Verified { .. }) => (1, None),
            Some(AccountStatus::Failed { error, .. }) => (2, Some(error.clone())),
        }
    }

    /// Takes the found servers once the lookup of `looking` is answered.
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
            self.version += 1;
        }
    }

    /// The password, moved out of the field (which the host is told to clear).
    fn take_password(&mut self) -> Secret {
        self.version += 1;
        Secret::new(std::mem::take(&mut self.password))
    }

    /// `set_password` for an existing account, else `add_account`.
    fn submit(&mut self) -> Option<Command> {
        if self.existing {
            let id = self.account?;
            return Some(Command::SetPassword {
                id,
                password: self.take_password(),
            });
        }
        let custom = self.preset == ProviderPreset::Custom;
        let imap = custom
            .then(|| Endpoint::parse(self.imap.trim()).ok())
            .flatten();
        let smtp = custom
            .then(|| Endpoint::parse(self.smtp.trim()).ok())
            .flatten();
        let name = self.name.trim();
        let display_name = (!name.is_empty()).then(|| name.to_owned());
        Some(Command::AddAccount {
            preset: self.preset,
            email: self.email.trim().to_owned(),
            display_name,
            password: Some(self.take_password()),
            imap,
            smtp,
        })
    }

    /// Takes the reply to one of the setup's commands; returns the login test after
    /// `add_account`.
    pub(super) fn accept(&mut self, reply: &Reply) -> Option<Request> {
        let error = match &reply.result {
            Ok(Ok(_)) => None,
            Ok(Err(err)) => Some(err.to_string()),
            Err(err) => Some(err.clone()),
        };
        match reply.name {
            CommandName::DiscoverServers => {
                // A refused lookup ends the wait; an answer otherwise arrives in the state.
                if let Some(error) = error {
                    self.looking = None;
                    self.lookup_error = Some(error);
                }
                None
            }
            CommandName::AddAccount => {
                let receipt = match &reply.result {
                    Ok(Ok(receipt)) => receipt,
                    _ => {
                        self.fail(error.unwrap_or_default());
                        return None;
                    }
                };
                match serde_json::from_value::<AccountId>(receipt.data["id"].clone()) {
                    Ok(id) => {
                        self.account = Some(id);
                        Some(Request::user(Command::TestAccount { id }))
                    }
                    Err(err) => {
                        self.fail(format!("unexpected receipt: {err}"));
                        None
                    }
                }
            }
            _ => {
                if let Some(error) = error {
                    self.fail(error);
                }
                None
            }
        }
    }

    fn fail(&mut self, error: String) {
        self.error = Some(error);
        self.step = Step::Password;
    }

    /// Carries out a button; `false` closes the setup.
    fn act(&mut self, act: SetupAct, out: &mut Vec<Request>) -> bool {
        match act {
            SetupAct::Open => {}
            SetupAct::Preset(preset) => self.preset = preset,
            SetupAct::Next => match self.step {
                Step::Preset => self.step = Step::Address,
                Step::Address if self.address_problem().is_none() => self.step = Step::Password,
                Step::Address | Step::Password | Step::Test => {}
            },
            SetupAct::Back => return self.back(),
            SetupAct::Find => {
                let email = self.email.trim().to_owned();
                if self.looking.is_none() {
                    out.push(Request {
                        purpose: Purpose::Setup,
                        cmd: Command::DiscoverServers {
                            email: email.clone(),
                        },
                    });
                    self.looking = Some(email);
                    self.lookup_error = None;
                }
            }
            SetupAct::Submit => {
                if self.step == Step::Password
                    && !self.password.is_empty()
                    && let Some(cmd) = self.submit()
                {
                    out.push(Request {
                        purpose: Purpose::Setup,
                        cmd,
                    });
                    self.error = None;
                    self.step = Step::Test;
                }
            }
            SetupAct::Retry => {
                if !self.existing
                    && let Some(id) = self.account.take()
                {
                    out.push(Request::user(Command::RemoveAccount { id, confirm: true }));
                }
                self.step = Step::Password;
            }
            SetupAct::Sync => {
                if let Some(id) = self.account {
                    out.push(Request::user(Command::SyncNow { account: Some(id) }));
                }
                return false;
            }
        }
        true
    }

    /// Back a step; `false` closes the setup.
    pub(super) fn back(&mut self) -> bool {
        match self.step {
            Step::Address => self.step = Step::Preset,
            Step::Password if !self.existing => self.step = Step::Address,
            Step::Preset | Step::Password | Step::Test => return false,
        }
        true
    }
}

impl Mobile {
    /// Opens the setup at its first step.
    pub(super) fn open_setup(&mut self) {
        self.setup = Some(Setup::new());
    }

    /// Opens the password form of `account`.
    pub(super) fn open_reenter(&mut self, state: &AppState, account: AccountId) {
        if let Some(setup) = Setup::reenter(state, account) {
            self.setup = Some(setup);
        }
    }

    /// A setup button.
    pub(super) fn setup_act(&mut self, act: SetupAct) -> Vec<Request> {
        let mut out = Vec::new();
        if act == SetupAct::Open {
            self.open_setup();
            return out;
        }
        if let Some(setup) = self.setup.as_mut()
            && !setup.act(act, &mut out)
        {
            self.setup = None;
        }
        out
    }

    fn setup_button(&mut self, label: &str, act: SetupAct) -> Button {
        Button {
            icon: None,
            label: label.to_owned(),
            action: self.add(Act::Setup(act)),
        }
    }

    /// The setup sheet, while open.
    pub(super) fn setup_sheet(&mut self, state: &AppState) -> Option<SetupSheet> {
        let mut setup = self.setup.take()?;
        setup.take_discovery(state);
        let sheet = self.draw_setup(&mut setup, state);
        self.setup = Some(setup);
        Some(sheet)
    }

    fn draw_setup(&mut self, setup: &mut Setup, state: &AppState) -> SetupSheet {
        let title = if setup.existing {
            "Change the password"
        } else {
            "Add an account"
        };
        let step = match (setup.existing, setup.step) {
            (true, _) => clean(&setup.email),
            (false, Step::Preset) => "1 / 4  Provider".to_owned(),
            (false, Step::Address) => "2 / 4  Address".to_owned(),
            (false, Step::Password) => "3 / 4  Password".to_owned(),
            (false, Step::Test) => "4 / 4  Connection".to_owned(),
        };
        let field = |field, label: &str, hint: &str, input, text: &str| SetupField {
            field,
            label: label.to_owned(),
            hint: hint.to_owned(),
            input,
            value: TextValue {
                text: text.to_owned(),
                version: setup.version,
            },
        };
        let mut presets = Vec::new();
        let mut fields = Vec::new();
        let mut note = None;
        let mut error = None;
        let mut busy = false;
        let mut actions = Vec::new();
        let back_label = match setup.step {
            Step::Preset => "Cancel",
            Step::Password if setup.existing => "Cancel",
            Step::Test => "Close",
            Step::Address | Step::Password => "Back",
        };
        match setup.step {
            Step::Preset => {
                for (preset, label, hint) in PRESETS {
                    presets.push(PresetChoice {
                        label: label.to_owned(),
                        hint: hint.to_owned(),
                        selected: setup.preset == preset,
                        action: self.add(Act::Setup(SetupAct::Preset(preset))),
                    });
                }
                actions.push(self.setup_button("Next", SetupAct::Next));
            }
            Step::Address => {
                fields.push(field(
                    Field::Email,
                    "Email address",
                    "you@example.com",
                    InputKind::Email,
                    &setup.email,
                ));
                fields.push(field(
                    Field::Name,
                    "Name (optional)",
                    "shown in the drawer",
                    InputKind::Name,
                    &setup.name,
                ));
                if setup.preset == ProviderPreset::Custom {
                    fields.push(field(
                        Field::Imap,
                        "IMAP server",
                        "imap.example.com:993",
                        InputKind::Server,
                        &setup.imap,
                    ));
                    fields.push(field(
                        Field::Smtp,
                        "SMTP server",
                        "smtp.example.com:465",
                        InputKind::Server,
                        &setup.smtp,
                    ));
                    let email = setup.email.trim();
                    note = Some(match (&state.status.discovery, &setup.lookup_error) {
                        _ if setup.looking.is_some() => "Looking…".to_owned(),
                        (_, Some(error)) => clean(error),
                        (Some(d), None) if d.email == email => match &d.found {
                            Ok(found) => clean(&format!("Found at {}", found.source)),
                            Err(error) => {
                                clean(&format!("Not found ({error}); enter them yourself."))
                            }
                        },
                        (None | Some(_), None) => "host:port, optionally :tls or :starttls. \
                                                  Find servers asks the provider, then \
                                                  Mozilla's list."
                            .to_owned(),
                    });
                    busy = setup.looking.is_some();
                    let valid =
                        app_front::compose::parse_addr(email).is_some_and(|a| a.name.is_none());
                    if valid && setup.looking.is_none() {
                        actions.push(self.setup_button("Find servers", SetupAct::Find));
                    }
                }
                let problem = setup.address_problem();
                if problem.is_none() {
                    actions.push(self.setup_button("Next", SetupAct::Next));
                } else if !setup.email.trim().is_empty() {
                    error = problem;
                }
            }
            Step::Password => {
                let label = if setup.preset == ProviderPreset::Gmail {
                    "App Password"
                } else {
                    "Password"
                };
                fields.push(field(Field::Password, label, "", InputKind::Password, ""));
                note = Some(
                    "Stored in Keystore-protected storage only; dail never writes it anywhere \
                     else."
                        .to_owned(),
                );
                error = setup.error.as_deref().map(clean);
                if !setup.password.is_empty() {
                    let label = if setup.existing {
                        "Save and test"
                    } else {
                        "Add and test"
                    };
                    actions.push(self.setup_button(label, SetupAct::Submit));
                }
            }
            Step::Test => match setup.status(state) {
                (1, _) => {
                    note = Some("Connected. IMAP and SMTP both accepted the login.".to_owned());
                    actions.push(self.setup_button("Sync now", SetupAct::Sync));
                }
                (2, failure) => {
                    error = Some(clean(&format!(
                        "The login failed: {}",
                        failure.unwrap_or_default()
                    )));
                    actions.push(self.setup_button("Try another password", SetupAct::Retry));
                }
                _ => {
                    busy = true;
                    note = Some(clean(&format!("Signing in to {}…", setup.email.trim())));
                }
            },
        }
        let mut back = self.setup_button(back_label, SetupAct::Back);
        if back_label == "Back" {
            // A left arrow, as every other Back (Cancel and Close stay words).
            back.icon = Some("arrow_back".to_owned());
        }
        SetupSheet {
            title: title.to_owned(),
            step,
            presets,
            fields,
            note,
            error,
            busy,
            actions,
            back,
        }
    }
}
