//! Account setup in the TUI (stretch): preset → address (and servers for a custom one) →
//! masked password → `add_account` → `test_account` → `sync_now`, like the GUI's wizard and
//! `dail account add`.
//!
//! The password goes straight into the write-only `Secret` of `add_account` (moved out of the
//! field, which is empty afterwards) and on to the keyring; it is never drawn. A failed test can
//! be retried with another password: the half-added account is removed first (`remove_account`
//! with `confirm`; this wizard created it a moment ago).
//!
//! [`Setup::reenter`] opens the same password step for an existing account (`set_password`:
//! keyring, then a login test); another try keeps the account.
//!
//! *Connect to a dail server instead* (ADR 0017, SRV-6), the last choice of the first step on a
//! profile without accounts (or [`Setup::connect`]), draws `app_front::connect::ConnectFlow`: the
//! server or a pasted `dail://pair` link, `connect_server`, the offered fingerprint to compare
//! with the code, `trust_server`, paired. Its commands go out as [`Purpose::Setup`].

use crate::text::{f, s};
use app_core::{
    AccountId, AccountStatus, AppState, Command, CommandName, Endpoint, ProviderPreset, Secret,
};
use app_front::connect::{Availability, ConnectFlow, Retry, Stage as Connect, availability};
use app_front::i18n::Key;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::request::{Purpose, Reply, Request};
use crate::text::{clean, wrap};
use crate::view::{bold, dim, reverse};

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

/// What a key in the wizard led to.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Submit these.
    pub requests: Vec<Request>,
    /// Close the wizard.
    pub close: bool,
}

/// The wizard's state.
pub struct Setup {
    step: Step,
    preset: usize,
    /// Email, name, IMAP, SMTP.
    fields: [String; 4],
    field: usize,
    password: String,
    account: Option<AccountId>,
    /// Changing the password of `account` (which exists) instead of adding one.
    existing: bool,
    error: Option<String>,
    /// *Connect to a dail server* (its code is never shown in `Debug`).
    flow: ConnectFlow,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            step: Step::Preset,
            preset: 0,
            fields: Default::default(),
            field: 0,
            password: String::new(),
            account: None,
            existing: false,
            error: None,
            flow: ConnectFlow::default(),
        }
    }
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
    /// The current step.
    pub const fn step(&self) -> Step {
        self.step
    }

    /// The setup at *Connect to a dail server* (the settings' row).
    pub fn connect() -> Self {
        Self {
            step: Step::Server,
            ..Self::default()
        }
    }

    /// The *Connect to a dail server* flow.
    pub const fn connect_flow(&self) -> &ConnectFlow {
        &self.flow
    }

    /// Whether it waits for the server (the runner then ticks).
    pub const fn is_waiting(&self) -> bool {
        matches!(self.step, Step::Server) && self.flow.is_waiting()
    }

    /// Follows the state (the connect flow's probe and pairing). The TUI calls it before each
    /// frame.
    pub fn sync(&mut self, state: &AppState, now_ms: i64) {
        if self.step == Step::Server {
            self.flow.sync(state, now_ms);
        }
    }

    /// Given up (Esc on the first step of the flow, or a cancel): the request that removes the
    /// link the flow left unpinned.
    fn give_up(&mut self, state: &AppState, out: &mut Outcome) {
        if let Some(cmd) = self.flow.cancel(state) {
            out.requests.push(Request {
                purpose: Purpose::Setup,
                cmd,
            });
        }
        out.close = true;
    }

    /// How many choices the first step has: the presets, and *Connect to a dail server
    /// instead* on a profile that can.
    fn choices(state: &AppState) -> usize {
        PRESETS.len() + usize::from(availability(state) == Availability::Open)
    }

    fn preset(&self) -> ProviderPreset {
        PRESETS
            .get(self.preset)
            .map_or(ProviderPreset::Gmail, |p| p.0)
    }

    /// How many address fields the preset has: email and name, plus the servers for custom.
    fn field_count(&self) -> usize {
        if self.preset() == ProviderPreset::Custom {
            4
        } else {
            2
        }
    }

    fn address_problem(&self) -> Option<String> {
        let [email, _, imap, smtp] = &self.fields;
        if app_front::compose::parse_addr(email.trim()).is_none_or(|a| a.name.is_some()) {
            return Some(s(Key::EnterEmail).to_owned());
        }
        if self.preset() == ProviderPreset::Custom {
            for (name, text) in [("IMAP", imap), ("SMTP", smtp)] {
                if let Err(err) = Endpoint::parse(text.trim()) {
                    return Some(format!("{name}: {err}"));
                }
            }
        }
        None
    }

    /// The password step for `account`, to replace its password; `None` if there is no such
    /// account.
    pub fn reenter(state: &AppState, account: AccountId) -> Option<Self> {
        let found = state.accounts.get(&account)?;
        let mut fields: [String; 4] = Default::default();
        fields[0].clone_from(&found.email);
        Some(Self {
            step: Step::Password,
            preset: PRESETS
                .iter()
                .position(|p| p.0 == found.preset)
                .unwrap_or(PRESETS.len() - 1),
            fields,
            account: Some(account),
            existing: true,
            ..Self::default()
        })
    }

    /// `set_password` for an existing account, else `add_account`.
    fn submit_password(&mut self) -> Request {
        match (self.existing, self.account) {
            (true, Some(id)) => Request {
                purpose: Purpose::Setup,
                cmd: Command::SetPassword {
                    id,
                    password: Secret::new(std::mem::take(&mut self.password)),
                },
            },
            _ => self.add_account(),
        }
    }

    fn add_account(&mut self) -> Request {
        let [email, name, imap, smtp] = &self.fields;
        let custom = self.preset() == ProviderPreset::Custom;
        let endpoint = |text: &str| custom.then(|| Endpoint::parse(text.trim()).ok()).flatten();
        // The password moves into the `Secret`; the field is empty from now on.
        let password = Secret::new(std::mem::take(&mut self.password));
        Request {
            purpose: Purpose::Setup,
            cmd: Command::AddAccount {
                preset: self.preset(),
                email: email.trim().to_owned(),
                display_name: (!name.trim().is_empty()).then(|| name.trim().to_owned()),
                password: Some(password),
                imap: endpoint(imap),
                smtp: endpoint(smtp),
            },
        }
    }

    /// Takes the reply to the wizard's `add_account`: then tests the login (the test's own reply
    /// is only its start; the result arrives in the account's status).
    pub fn accept(&mut self, reply: &Reply, now_ms: i64) -> Option<Request> {
        if reply.tag == Purpose::Setup
            && matches!(
                reply.name,
                CommandName::ConnectServer | CommandName::TrustServer
            )
        {
            let result = match &reply.result {
                Ok(Ok(receipt)) => Ok(receipt),
                Ok(Err(err)) => Err(app_front::i18n::message(&err.to_string())),
                Err(err) => Err(format!("dail: {err}")),
            };
            self.flow.accept(result, now_ms);
            return None;
        }
        if reply.tag == Purpose::Setup
            && matches!(
                reply.name,
                CommandName::DiscoverServers | CommandName::SetPassword
            )
        {
            // A refused lookup shows; its answer otherwise arrives in the state.
            match &reply.result {
                Ok(Ok(_)) => {}
                Ok(Err(err)) => self.error = Some(app_front::i18n::message(&err.to_string())),
                Err(err) => self.error = Some(app_front::i18n::message(&err.to_string())),
            }
            return None;
        }
        if reply.tag != Purpose::Setup || reply.name != CommandName::AddAccount {
            return None;
        }
        match &reply.result {
            Ok(Ok(receipt)) => {
                match serde_json::from_value::<AccountId>(receipt.data["id"].clone()) {
                    Ok(id) => {
                        self.account = Some(id);
                        return Some(Request {
                            purpose: Purpose::Setup,
                            cmd: Command::TestAccount { id },
                        });
                    }
                    Err(err) => {
                        self.fail(f(Key::UnexpectedReceipt, &[("error", &err.to_string())]))
                    }
                }
            }
            Ok(Err(err)) => self.fail(app_front::i18n::message(&err.to_string())),
            Err(err) => self.fail(app_front::i18n::message(&err.to_string())),
        }
        None
    }

    fn fail(&mut self, error: String) {
        self.error = Some(error);
        self.step = Step::Password;
    }

    /// Pasted text goes into the field under the cursor (the password too).
    pub fn paste(&mut self, text: &str) {
        let line = text.lines().next().unwrap_or("");
        match self.step {
            Step::Address => {
                if let Some(field) = self.fields.get_mut(self.field) {
                    field.push_str(line);
                }
            }
            Step::Password => self.password.push_str(line),
            Step::Server => self.flow.paste(text),
            Step::Preset | Step::Test => {}
        }
    }

    /// Handles a key while the wizard is open.
    pub fn key(&mut self, key: KeyEvent, state: &AppState) -> Outcome {
        let mut out = Outcome::default();
        let typed = match key.code {
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Some(c)
            }
            _ => None,
        };
        match self.step {
            Step::Preset => match key.code {
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                    self.preset = (self.preset + 1).min(Self::choices(state) - 1);
                }
                KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                    self.preset = self.preset.saturating_sub(1);
                }
                KeyCode::Enter if self.preset >= PRESETS.len() => {
                    self.preset = 0;
                    self.flow = ConnectFlow::default();
                    self.step = Step::Server;
                }
                KeyCode::Enter => {
                    self.step = Step::Address;
                    self.field = 0;
                }
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => out.close = true,
                _ => {}
            },
            Step::Address => match key.code {
                KeyCode::Char('f')
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && self.preset() == ProviderPreset::Custom =>
                {
                    // The servers found for this address fill the fields; otherwise look them up.
                    let email = self.fields[0].trim().to_owned();
                    match &state.status.discovery {
                        Some(d) if d.email == email && d.found.is_ok() => {
                            if let Ok(found) = &d.found {
                                self.fields[2] = found.imap.to_string();
                                self.fields[3] = found.smtp.to_string();
                            }
                        }
                        _ => {
                            self.error = None;
                            out.requests.push(Request {
                                purpose: Purpose::Setup,
                                cmd: Command::DiscoverServers { email },
                            });
                        }
                    }
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.field = (self.field + 1) % self.field_count();
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.field = (self.field + self.field_count() - 1) % self.field_count();
                }
                KeyCode::Enter if self.field + 1 < self.field_count() => self.field += 1,
                KeyCode::Enter => {
                    self.error = self.address_problem();
                    if self.error.is_none() {
                        self.step = Step::Password;
                    }
                }
                KeyCode::Esc => self.step = Step::Preset,
                KeyCode::Backspace => {
                    if let Some(field) = self.fields.get_mut(self.field) {
                        field.pop();
                    }
                }
                _ => {
                    if let (Some(c), Some(field)) = (typed, self.fields.get_mut(self.field)) {
                        field.push(c);
                    }
                }
            },
            Step::Password => match key.code {
                KeyCode::Enter if !self.password.is_empty() => {
                    self.error = None;
                    self.step = Step::Test;
                    out.requests.push(self.submit_password());
                }
                KeyCode::Esc => {
                    self.password.clear();
                    if self.existing {
                        out.close = true;
                    } else {
                        self.step = Step::Address;
                    }
                }
                KeyCode::Backspace => {
                    self.password.pop();
                }
                _ => {
                    if let Some(c) = typed {
                        self.password.push(c);
                    }
                }
            },
            Step::Server => self.server_key(key, typed, state, &mut out),
            Step::Test => {
                let status = self
                    .account
                    .and_then(|id| state.accounts.get(&id))
                    .map(|a| &a.status);
                match (status, key.code, self.account) {
                    (Some(AccountStatus::Verified { .. }), KeyCode::Enter, Some(id)) => {
                        out.requests
                            .push(Request::user(Command::SyncNow { account: Some(id) }));
                        out.close = true;
                    }
                    (Some(AccountStatus::Failed { .. }), KeyCode::Char('p'), Some(_))
                        if self.existing =>
                    {
                        self.step = Step::Password;
                    }
                    (Some(AccountStatus::Failed { .. }), KeyCode::Char('p'), Some(id)) => {
                        // Try another password: this wizard's account goes first.
                        out.requests
                            .push(Request::user(Command::RemoveAccount { id, confirm: true }));
                        self.account = None;
                        self.step = Step::Password;
                    }
                    (Some(AccountStatus::Failed { .. }), KeyCode::Char('k') | KeyCode::Esc, _) => {
                        out.close = true;
                    }
                    (Some(AccountStatus::Verified { .. }), KeyCode::Esc, _) => out.close = true,
                    _ => {}
                }
            }
        }
        out
    }

    /// A key at *Connect to a dail server*.
    fn server_key(
        &mut self,
        key: KeyEvent,
        typed: Option<char>,
        state: &AppState,
        out: &mut Outcome,
    ) {
        let mut submit = None;
        match self.flow.stage().clone() {
            Connect::Address => match key.code {
                KeyCode::Enter => submit = self.flow.submit_address(state),
                KeyCode::Esc => {
                    self.give_up(state, out);
                    out.close = false;
                    self.flow = ConnectFlow::default();
                    self.step = Step::Preset;
                }
                KeyCode::Backspace => {
                    self.flow.address.pop();
                    self.flow.address_edited();
                }
                _ => {
                    if let Some(c) = typed {
                        self.flow.address.push(c);
                        self.flow.address_edited();
                    }
                }
            },
            Connect::Compare { .. } => match key.code {
                KeyCode::Enter => submit = self.flow.submit_code(state),
                KeyCode::Esc => self.flow.back(),
                KeyCode::Backspace => {
                    self.flow.code.pop();
                }
                _ => {
                    if let Some(c) = typed.filter(char::is_ascii_alphanumeric) {
                        self.flow.code.push(c.to_ascii_uppercase());
                    }
                }
            },
            Connect::Probing | Connect::Pairing => {
                if key.code == KeyCode::Esc {
                    self.give_up(state, out);
                }
            }
            Connect::Mismatch { .. } => match key.code {
                KeyCode::Enter => self.flow.retry(),
                KeyCode::Esc => self.give_up(state, out),
                _ => {}
            },
            Connect::Failed { .. } => match key.code {
                KeyCode::Enter => self.flow.retry(),
                KeyCode::Esc => self.give_up(state, out),
                _ => {}
            },
            Connect::Paired => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                    out.close = true;
                }
            }
        }
        if let Some(cmd) = submit {
            out.requests.push(Request {
                purpose: Purpose::Setup,
                cmd,
            });
        }
    }

    /// The lines and footer of *Connect to a dail server*.
    fn server_lines(&self, state: &AppState, lines: &mut Vec<(String, Style)>) -> &'static str {
        let server = self.flow.server();
        let fingerprint = |lines: &mut Vec<(String, Style)>, label: &str, fp: &str| {
            lines.push((label.to_owned(), dim()));
            for line in app_front::connect::fingerprint_lines(fp) {
                lines.push((format!("  {line}"), bold()));
            }
        };
        match self.flow.stage() {
            Connect::Address => {
                let value = clean(&self.flow.address);
                lines.push((format!("{}: {value}▏", s(Key::ConnectServerLabel)), bold()));
                lines.push((s(Key::ConnectAddressHint).to_owned(), dim()));
                if let Some(note) = self.flow.note() {
                    lines.push((clean(note), Style::default()));
                }
                if let Some(problem) = self
                    .flow
                    .address_problem()
                    .filter(|_| !self.flow.address.trim().is_empty())
                {
                    lines.push((problem, dim()));
                }
                s(Key::ConnectAddressFooter)
            }
            Connect::Probing => {
                lines.push((
                    f(Key::ConnectProbing, &[("server", &clean(&server))]),
                    dim(),
                ));
                s(Key::ConnectWaitFooter)
            }
            Connect::Compare {
                fingerprint: offered,
                matches_link,
            } => {
                lines.push((
                    f(Key::ConnectCompare, &[("server", &clean(&server))]),
                    Style::default(),
                ));
                fingerprint(lines, s(Key::PairingFingerprint), offered);
                if *matches_link {
                    lines.push((format!("✓ {}", s(Key::ConnectMatchesLink)), bold()));
                }
                lines.push((String::new(), Style::default()));
                lines.push((
                    format!("{}: {}▏", s(Key::PairingCode), clean(&self.flow.code)),
                    bold(),
                ));
                lines.push((s(Key::ConnectCodeHint).to_owned(), dim()));
                if let Some(problem) = self
                    .flow
                    .code_problem()
                    .filter(|_| !self.flow.code.is_empty())
                {
                    lines.push((problem, dim()));
                }
                s(Key::ConnectCodeFooter)
            }
            Connect::Mismatch { offered, expected } => {
                lines.push((
                    f(Key::ConnectMismatch, &[("server", &clean(&server))]),
                    bold(),
                ));
                fingerprint(lines, s(Key::PairingFingerprint), offered);
                fingerprint(lines, s(Key::ConnectLinkFingerprint), expected);
                s(Key::ConnectRetryFooter)
            }
            Connect::Pairing => {
                lines.push((
                    f(Key::ConnectPairing, &[("server", &clean(&server))]),
                    dim(),
                ));
                s(Key::ConnectWaitFooter)
            }
            Connect::Paired => {
                lines.push((
                    f(Key::ConnectPaired, &[("server", &clean(&server))]),
                    bold(),
                ));
                if let Some(summary) = app_front::server::summary(state, app_front::i18n::current())
                {
                    lines.push((summary.text, dim()));
                }
                s(Key::ConnectDoneFooter)
            }
            Connect::Failed { error, retry } => {
                lines.push((clean(error), bold()));
                lines.push((
                    match retry {
                        Retry::Address => s(Key::TryAgain),
                        Retry::Code => s(Key::AnotherCode),
                    }
                    .to_owned(),
                    dim(),
                ));
                s(Key::ConnectRetryFooter)
            }
        }
    }

    /// Draws the wizard over `area`.
    pub fn draw(&self, frame: &mut Frame, area: Rect, state: &AppState, borders: bool) {
        frame.render_widget(Clear, area);
        let title = if self.step == Step::Server {
            s(Key::ConnectToServer)
        } else {
            s(Key::AddAccountTitle)
        };
        let block = if borders {
            Block::bordered().border_style(bold()).title(title)
        } else {
            Block::new()
        };
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let columns = usize::from(inner.width);
        let mut lines: Vec<(String, Style)> = Vec::new();
        let step = match self.step {
            Step::Preset => s(Key::Step1),
            Step::Address => s(Key::Step2),
            Step::Password => s(Key::Step3),
            Step::Test => s(Key::Step4),
            Step::Server => "",
        };
        if self.step == Step::Server {
            lines.push((s(Key::ConnectToServer).to_owned(), bold()));
        } else if self.existing {
            lines.push((
                f(Key::ChangePasswordOf, &[("email", &clean(&self.fields[0]))]),
                bold(),
            ));
        } else {
            lines.push((f(Key::AddAccountStep, &[("step", step)]), bold()));
        }
        lines.push((String::new(), Style::default()));
        let footer = match self.step {
            Step::Preset => {
                let connect = (availability(state) == Availability::Open)
                    .then_some((Key::ConnectToServerInstead, Key::ConnectToServerDetail));
                let choices = PRESETS.iter().map(|(_, title, hint)| (*title, *hint));
                for (i, (title, hint)) in choices.chain(connect).enumerate() {
                    let here = i == self.preset;
                    lines.push((
                        format!("{} {}", if here { "›" } else { " " }, s(title)),
                        if here { reverse() } else { Style::default() },
                    ));
                    lines.push((format!("    {}", s(hint)), dim()));
                }
                s(Key::ChooseFooter)
            }
            Step::Address => {
                let labels = [
                    s(Key::Email),
                    s(Key::NameOptional),
                    s(Key::ImapHostPort),
                    s(Key::SmtpHostPort),
                ];
                for (i, label) in labels.iter().enumerate().take(self.field_count()) {
                    let here = i == self.field;
                    let value = self.fields.get(i).map(|v| clean(v)).unwrap_or_default();
                    let cursor = if here { "▏" } else { "" };
                    lines.push((
                        format!("{label}: {value}{cursor}"),
                        if here { bold() } else { Style::default() },
                    ));
                }
                if self.preset() == ProviderPreset::Custom {
                    lines.push((s(Key::PortsHint).to_owned(), dim()));
                    let email = self.fields[0].trim();
                    let note = match &state.status.discovery {
                        Some(d) if d.email == email => match &d.found {
                            Ok(found) => f(
                                Key::FoundFill,
                                &[
                                    ("source", &clean(&found.source)),
                                    ("imap", &found.imap.to_string()),
                                    ("smtp", &found.smtp.to_string()),
                                ],
                            ),
                            Err(error) => f(Key::NotFound, &[("error", &clean(error))]),
                        },
                        _ => s(Key::CtrlFHint).to_owned(),
                    };
                    lines.push((note, dim()));
                }
                s(Key::FieldFooter)
            }
            Step::Password => {
                let label = if self.preset() == ProviderPreset::Gmail {
                    s(Key::AppPassword)
                } else {
                    s(Key::Password)
                };
                let masked = "•".repeat(self.password.chars().count());
                lines.push((format!("{label}: {masked}▏"), bold()));
                lines.push((s(Key::KeyringTui).to_owned(), dim()));
                if self.existing {
                    s(Key::SaveTestFooter)
                } else {
                    s(Key::AddTestFooter)
                }
            }
            Step::Test => {
                let status = self
                    .account
                    .and_then(|id| state.accounts.get(&id))
                    .map(|a| &a.status);
                match status {
                    Some(AccountStatus::Verified { .. }) => {
                        lines.push((s(Key::ConnectedCap).to_owned(), bold()));
                        s(Key::SyncCloseFooter)
                    }
                    Some(AccountStatus::Failed { error, .. }) => {
                        lines.push((s(Key::LoginFailedCap).to_owned(), bold()));
                        lines.push((clean(&app_front::i18n::message(error)), Style::default()));
                        if self.existing {
                            s(Key::PasswordClose)
                        } else {
                            s(Key::PasswordKeep)
                        }
                    }
                    _ => {
                        lines.push((s(Key::TestingLogin).to_owned(), dim()));
                        ""
                    }
                }
            }
            Step::Server => self.server_lines(state, &mut lines),
        };
        if let Some(error) = &self.error {
            lines.push((String::new(), Style::default()));
            lines.push((clean(error), bold()));
        }
        let mut out: Vec<Line<'static>> = lines
            .into_iter()
            .flat_map(|(text, style)| {
                wrap(&text, columns)
                    .into_iter()
                    .map(move |piece| Line::from(Span::styled(piece, style)))
            })
            .collect();
        while out.len() + 1 < usize::from(inner.height) {
            out.push(Line::from(""));
        }
        out.push(Line::from(Span::styled(footer, dim())));
        frame.render_widget(Paragraph::new(out), inner);
    }
}
