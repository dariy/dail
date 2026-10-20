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

use app_core::{
    AccountId, AccountStatus, AppState, Command, CommandName, Endpoint, ProviderPreset, Secret,
};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::request::{Purpose, Reply, Request};
use crate::text::{clean, wrap};
use crate::view::{bold, dim, reverse};

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
            return Some("Enter an email address like you@example.com.".to_owned());
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
    pub fn accept(&mut self, reply: &Reply) -> Option<Request> {
        if reply.tag == Purpose::Setup
            && matches!(
                reply.name,
                CommandName::DiscoverServers | CommandName::SetPassword
            )
        {
            // A refused lookup shows; its answer otherwise arrives in the state.
            match &reply.result {
                Ok(Ok(_)) => {}
                Ok(Err(err)) => self.error = Some(err.to_string()),
                Err(err) => self.error = Some(err.to_string()),
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
                    Err(err) => self.fail(format!("unexpected receipt: {err}")),
                }
            }
            Ok(Err(err)) => self.fail(err.to_string()),
            Err(err) => self.fail(err.to_string()),
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
                    self.preset = (self.preset + 1).min(PRESETS.len() - 1);
                }
                KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                    self.preset = self.preset.saturating_sub(1);
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

    /// Draws the wizard over `area`.
    pub fn draw(&self, frame: &mut Frame, area: Rect, state: &AppState, borders: bool) {
        frame.render_widget(Clear, area);
        let block = if borders {
            Block::bordered()
                .border_style(bold())
                .title(" Add account ")
        } else {
            Block::new()
        };
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let columns = usize::from(inner.width);
        let mut lines: Vec<(String, Style)> = Vec::new();
        let step = match self.step {
            Step::Preset => "1/4 Provider",
            Step::Address => "2/4 Address",
            Step::Password => "3/4 Password",
            Step::Test => "4/4 Connection",
        };
        if self.existing {
            lines.push((
                format!("Change the password · {}", clean(&self.fields[0])),
                bold(),
            ));
        } else {
            lines.push((format!("Add account · {step}"), bold()));
        }
        lines.push((String::new(), Style::default()));
        let footer = match self.step {
            Step::Preset => {
                for (i, (_, title, hint)) in PRESETS.iter().enumerate() {
                    let here = i == self.preset;
                    lines.push((
                        format!("{} {title}", if here { "›" } else { " " }),
                        if here { reverse() } else { Style::default() },
                    ));
                    lines.push((format!("    {hint}"), dim()));
                }
                "↑↓ choose · Enter next · Esc cancel"
            }
            Step::Address => {
                let labels = [
                    "Email",
                    "Name (optional)",
                    "IMAP host:port",
                    "SMTP host:port",
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
                    lines.push((
                        "Ports 143 and 587 use STARTTLS, others TLS; add :tls or :starttls to choose."
                            .to_owned(),
                        dim(),
                    ));
                    let email = self.fields[0].trim();
                    let note = match &state.status.discovery {
                        Some(d) if d.email == email => match &d.found {
                            Ok(found) => format!(
                                "Found at {}: {} and {}. Ctrl+F fills them in.",
                                clean(&found.source),
                                found.imap,
                                found.smtp
                            ),
                            Err(error) => {
                                format!("Not found ({}); enter them yourself.", clean(error))
                            }
                        },
                        _ => "Ctrl+F finds the servers (asks the provider's web server, then Mozilla's list)."
                            .to_owned(),
                    };
                    lines.push((note, dim()));
                }
                "Tab field · Enter next · Esc back"
            }
            Step::Password => {
                let label = if self.preset() == ProviderPreset::Gmail {
                    "App Password"
                } else {
                    "Password"
                };
                let masked = "•".repeat(self.password.chars().count());
                lines.push((format!("{label}: {masked}▏"), bold()));
                lines.push((
                    "It goes to the system keyring, never into dail's files.".to_owned(),
                    dim(),
                ));
                if self.existing {
                    "Enter save and test · Esc cancel"
                } else {
                    "Enter add and test · Esc back"
                }
            }
            Step::Test => {
                let status = self
                    .account
                    .and_then(|id| state.accounts.get(&id))
                    .map(|a| &a.status);
                match status {
                    Some(AccountStatus::Verified { .. }) => {
                        lines.push(("Connected.".to_owned(), bold()));
                        "Enter syncs now · Esc closes"
                    }
                    Some(AccountStatus::Failed { error, .. }) => {
                        lines.push(("The login failed:".to_owned(), bold()));
                        lines.push((clean(error), Style::default()));
                        if self.existing {
                            "p another password · k close"
                        } else {
                            "p another password · k keep it anyway"
                        }
                    }
                    _ => {
                        lines.push(("Testing the login…".to_owned(), dim()));
                        ""
                    }
                }
            }
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
