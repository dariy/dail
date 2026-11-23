//! *Connect to a dail server* (ADR 0017, SRV-6): the account wizards' other first step, which
//! makes this profile a device of a dail server instead of adding a mail account. The GUI's
//! wizard and the TUI's setup draw [`ConnectFlow`] and feed it the replies and the state; the
//! rules are here, in the order of `dail remote add`:
//!
//! 1. [`Stage::Address`]: `host[:port]` (port [`DEFAULT_PORT`] if none, checked like `dail remote
//!    add`, the host a DNS name or an IP address), or a pasted `dail://pair` link, parsed by
//!    [`crate::pairing::parse_uri`] as untrusted input: it fills the server and the code and sets
//!    the fingerprint the server must show.
//! 2. `connect_server` → [`Stage::Probing`] until `link.server.offered` (or `link.error`).
//! 3. [`Stage::Compare`]: the offered fingerprint, in groups of four as `dail serve` and *Pair a
//!    device…* show it, for the user to compare; the 8-character code. With a pasted link, an
//!    offered fingerprint other than the link's is [`Stage::Mismatch`]: nothing is trusted.
//! 4. `trust_server` with the offered fingerprint and the code (a write-only `Secret`; the field
//!    is empty afterwards) → [`Stage::Pairing`] until the fingerprint is pinned
//!    ([`Stage::Paired`]) or `link.error` says why not ([`Stage::Failed`], another code may be
//!    tried).
//!
//! Only a profile without mail accounts can become a device ([`availability`]; the engine
//! refuses others with `ERR013`). A flow given up before pairing removes its unpinned link
//! ([`ConnectFlow::cancel`]), so the profile is a plain one again.
//!
//! The state that shows a probe's or a pairing's failure is the same before and after a retry
//! with the same error text, and a front end may see a state older than a reply. So a failure
//! counts only after the command's reply came back and the state then showed the attempt in
//! progress, a different error, or [`GRACE_MS`] passed.

use app_core::{AppState, Command, CommandReceipt, Secret, ServerEndpoint};

use crate::i18n::{self, Key};
use crate::pairing;

/// The port `dail serve` listens on and `dail remote add` dials when none is given.
pub const DEFAULT_PORT: u16 = 7443;

/// How long after a reply an unchanged error in the state counts as the attempt's failure.
pub const GRACE_MS: i64 = 1500;

/// Whether this profile can connect to a dail server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// No mail accounts and no pinned server: it can.
    Open,
    /// It has mail accounts (the engine refuses with `ERR013`).
    HasAccounts(usize),
    /// It is a device of a server already.
    Linked,
}

/// Whether `state`'s profile can connect to a dail server.
pub fn availability(state: &AppState) -> Availability {
    if state
        .link
        .as_ref()
        .and_then(|link| link.server.as_ref())
        .is_some_and(ServerEndpoint::is_pinned)
    {
        Availability::Linked
    } else if !state.accounts.is_empty() {
        Availability::HasAccounts(state.accounts.len())
    } else {
        Availability::Open
    }
}

/// `host[:port]` as `connect_server` takes it, by the rules of `dail remote add`: the port is
/// [`DEFAULT_PORT`] when none is given, an IPv6 address keeps its brackets (`[::1]:7443`) and a
/// bare one has no port. The host must also be a DNS name or an IP address. The error is the
/// text to show.
pub fn parse_server(text: &str) -> Result<(String, u16), Key> {
    let text = text.trim();
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        match rest.split_once(']') {
            Some((_, "")) => (text, None),
            Some((inner, port)) => match port.strip_prefix(':') {
                Some(port) => (text.get(..inner.len() + 2).unwrap_or(text), Some(port)),
                None => return Err(Key::ConnectExpectedHostPort),
            },
            None => return Err(Key::ConnectExpectedHostPort),
        }
    } else {
        match text.matches(':').count() {
            0 => (text, None),
            1 => text
                .split_once(':')
                .map_or((text, None), |(host, port)| (host, Some(port))),
            // A bare IPv6 address.
            _ => (text, None),
        }
    };
    if host.is_empty() {
        return Err(Key::ConnectExpectedHostPort);
    }
    let port = match port {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(Key::ConnectBadPort)?,
        None => DEFAULT_PORT,
    };
    pairing::valid_host(host).map_err(|_| Key::ConnectBadHost)?;
    Ok((host.to_owned(), port))
}

/// `host:port` for the texts (an IPv6 host in brackets).
fn server_text(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// What a failed step goes back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// The address: the server could not be reached, or `connect_server` was refused.
    Address,
    /// The code: the server refused the pairing.
    Code,
}

/// Where the flow is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// Typing the server, or pasting a pairing link.
    Address,
    /// `connect_server` was submitted; waiting for the server's certificate.
    Probing,
    /// The server offered `fingerprint`: compare it, type the code.
    Compare {
        /// The offered fingerprint, 64 lower-case hex digits.
        fingerprint: String,
        /// It equals the fingerprint of the pasted pairing link.
        matches_link: bool,
    },
    /// The server offered another fingerprint than the pasted pairing link: never trusted.
    Mismatch {
        /// What the server offered.
        offered: String,
        /// What the link said.
        expected: String,
    },
    /// `trust_server` was submitted; waiting for the server to accept the code.
    Pairing,
    /// The certificate is pinned: this profile mirrors the server.
    Paired,
    /// A step failed (already in words).
    Failed {
        /// Why.
        error: String,
        /// What [`ConnectFlow::retry`] goes back to.
        retry: Retry,
    },
}

/// What the flow knows about the command it waits for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Wait {
    /// When the command's reply came back (Unix ms); `None` until then.
    acked_at: Option<i64>,
    /// The state showed the attempt in progress after the reply.
    seen: bool,
    /// `link.error` when the command was submitted.
    error_before: Option<String>,
}

impl Wait {
    fn new(state: &AppState) -> Self {
        Self {
            acked_at: None,
            seen: false,
            error_before: state.link.as_ref().and_then(|link| link.error.clone()),
        }
    }

    /// Whether `error` (the state's `link.error`) is this attempt's failure.
    fn failed(&self, error: Option<&String>, now_ms: i64) -> bool {
        let Some(acked_at) = self.acked_at else {
            return false;
        };
        error.is_some()
            && (self.seen
                || error != self.error_before.as_ref()
                || now_ms.saturating_sub(acked_at) >= GRACE_MS)
    }
}

/// *Connect to a dail server*: the module's flow. The code typed or read from a link stays only
/// here until it goes into `trust_server`; `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectFlow {
    /// The server as typed (`host[:port]`).
    pub address: String,
    /// The pairing code as typed.
    pub code: String,
    stage: Stage,
    /// The server of the last `connect_server`.
    target: Option<(String, u16)>,
    /// The fingerprint of a pasted pairing link: the server must offer exactly this one.
    expected: Option<String>,
    /// The fingerprint the server offered, kept for another code after a refused pairing.
    offered: Option<String>,
    /// A note about the address field (a link that was read, or why it could not be).
    note: Option<String>,
    wait: Wait,
}

impl std::fmt::Debug for ConnectFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectFlow")
            .field("address", &self.address)
            .field("code", &"<redacted>")
            .field("stage", &self.stage)
            .field("target", &self.target)
            .field("expected", &self.expected)
            .finish_non_exhaustive()
    }
}

impl Default for ConnectFlow {
    fn default() -> Self {
        Self {
            address: String::new(),
            code: String::new(),
            stage: Stage::Address,
            target: None,
            expected: None,
            offered: None,
            note: None,
            wait: Wait::default(),
        }
    }
}

impl ConnectFlow {
    /// Where the flow is.
    pub const fn stage(&self) -> &Stage {
        &self.stage
    }

    /// Whether it waits for the server (front ends keep redrawing: a spinner, a tick).
    pub const fn is_waiting(&self) -> bool {
        matches!(self.stage, Stage::Probing | Stage::Pairing)
    }

    /// The server being connected to, as `host:port`.
    pub fn server(&self) -> String {
        match &self.target {
            Some((host, port)) => server_text(host, *port),
            None => parse_server(&self.address)
                .map(|(host, port)| server_text(&host, port))
                .unwrap_or_else(|_| self.address.trim().to_owned()),
        }
    }

    /// The fingerprint of the pasted pairing link, if one was read.
    pub fn link_fingerprint(&self) -> Option<&str> {
        self.expected.as_deref()
    }

    /// A note under the address field: the pairing link that was read, or why it could not be.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Whether the address field holds a `dail://` link rather than a server.
    fn is_link(&self) -> bool {
        self.address
            .trim()
            .get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case("dail://"))
    }

    /// Reads a `dail://pair` link pasted into the address field (also done on *Connect*): a
    /// valid one fills the server and the code and sets the fingerprint the
    /// server must show; an invalid one empties the field and says why (never repeating the
    /// untrusted text). Any other text is left as it is.
    pub fn read_address(&mut self) {
        if !self.is_link() {
            return;
        }
        match pairing::parse_uri(self.address.trim()) {
            Ok(uri) => {
                let host = if uri.host().contains(':') {
                    format!("[{}]", uri.host())
                } else {
                    uri.host().to_owned()
                };
                self.address = server_text(&host, uri.port());
                self.code = uri.code().to_owned();
                self.expected = Some(uri.fingerprint().to_owned());
                self.note = Some(i18n::f(Key::ConnectLinkRead, &[("server", &self.address)]));
            }
            Err(err) => {
                self.address.clear();
                self.code.clear();
                self.expected = None;
                self.note = Some(i18n::f(
                    Key::ConnectLinkInvalid,
                    &[("error", &i18n::message(&err.to_string()))],
                ));
            }
        }
    }

    /// Text pasted into the field the stage shows (the first line): the address (a pairing link
    /// is read at once) or the code.
    pub fn paste(&mut self, text: &str) {
        let line = text.lines().next().unwrap_or("").trim();
        match self.stage {
            Stage::Address => {
                self.address.push_str(line);
                self.read_address();
            }
            Stage::Compare { .. } => self.code.push_str(line),
            _ => {}
        }
    }

    /// The user edited the address by hand: a fingerprint read from a link no longer applies.
    pub fn address_edited(&mut self) {
        self.expected = None;
        self.note = None;
    }

    /// Why the address cannot be submitted, in words; `None` when it can.
    pub fn address_problem(&self) -> Option<String> {
        if self.is_link() {
            // Read on *Connect*.
            return None;
        }
        parse_server(&self.address)
            .err()
            .map(|key| i18n::s(key).to_owned())
    }

    /// Why the code cannot be submitted, in words; `None` when it can.
    pub fn code_problem(&self) -> Option<String> {
        let code = self.code.trim().to_ascii_uppercase();
        (!app_core::link::is_pairing_code(&code)).then(|| i18n::s(Key::ConnectBadCode).to_owned())
    }

    /// *Connect*: the `connect_server` to submit (its reply goes to [`Self::accept`]), or
    /// `None` when the address is not valid (a pairing link in the field is read first), the profile cannot connect, or the flow is not at
    /// the address.
    pub fn submit_address(&mut self, state: &AppState) -> Option<Command> {
        let at_address = matches!(
            self.stage,
            Stage::Address
                | Stage::Failed {
                    retry: Retry::Address,
                    ..
                }
        );
        if !at_address || availability(state) != Availability::Open {
            return None;
        }
        // A link typed or pasted into the field: its server, code and fingerprint (or why not).
        self.read_address();
        let (host, port) = parse_server(&self.address).ok()?;
        self.target = Some((host.clone(), port));
        self.offered = None;
        self.wait = Wait::new(state);
        self.stage = Stage::Probing;
        Some(Command::ConnectServer { host, port })
    }

    /// *Pair*: the `trust_server` with the offered fingerprint and the code, which moves out of
    /// the field into the command's `Secret`; `None` when the code is not valid or the flow is
    /// not comparing.
    pub fn submit_code(&mut self, state: &AppState) -> Option<Command> {
        let Stage::Compare { fingerprint, .. } = &self.stage else {
            return None;
        };
        if self.expected.as_ref().is_some_and(|e| e != fingerprint) || self.code_problem().is_some()
        {
            return None;
        }
        let fingerprint = fingerprint.clone();
        let code = std::mem::take(&mut self.code).trim().to_ascii_uppercase();
        self.wait = Wait::new(state);
        self.stage = Stage::Pairing;
        Some(Command::TrustServer {
            fingerprint,
            code: Secret::new(code),
        })
    }

    /// The reply to the submitted command: its receipt, or why it was refused (in words).
    pub fn accept(&mut self, result: Result<&CommandReceipt, String>, now_ms: i64) {
        let retry = match self.stage {
            Stage::Probing => Retry::Address,
            Stage::Pairing => Retry::Code,
            _ => return,
        };
        match result {
            Ok(_) => self.wait.acked_at = Some(now_ms),
            Err(error) => {
                let error = if error.starts_with("ERR013") {
                    format!("{error}\n{}", i18n::s(Key::ConnectUseAnotherProfile))
                } else {
                    error
                };
                self.stage = Stage::Failed { error, retry };
            }
        }
    }

    /// Follows the state: the server's certificate, the pinned fingerprint, or the failure.
    pub fn sync(&mut self, state: &AppState, now_ms: i64) {
        let link = state.link.as_ref();
        let error = link.and_then(|link| link.error.as_ref());
        let server = link.and_then(|link| link.server.as_ref()).filter(|server| {
            self.target.as_ref().is_some_and(|(host, port)| {
                server.port == *port && server.host.eq_ignore_ascii_case(host)
            })
        });
        let acked = self.wait.acked_at.is_some();
        match &self.stage {
            Stage::Probing => {
                if let Some(offered) = server.and_then(|s| s.offered.clone()) {
                    self.offered = Some(offered.clone());
                    self.stage = match &self.expected {
                        Some(expected) if *expected != offered => Stage::Mismatch {
                            offered,
                            expected: expected.clone(),
                        },
                        _ => Stage::Compare {
                            matches_link: self.expected.is_some(),
                            fingerprint: offered,
                        },
                    };
                } else if self.wait.failed(error, now_ms) {
                    self.stage = Stage::Failed {
                        error: i18n::f(
                            Key::ConnectFailed,
                            &[("error", &i18n::message(error.map_or("", String::as_str)))],
                        ),
                        retry: Retry::Address,
                    };
                } else if acked && server.is_some() && error.is_none() {
                    self.wait.seen = true;
                }
            }
            Stage::Pairing => {
                if server.is_some_and(ServerEndpoint::is_pinned) {
                    self.stage = Stage::Paired;
                } else if acked && server.is_some_and(|s| s.pairing.is_some()) {
                    self.wait.seen = true;
                } else if self.wait.failed(error, now_ms) {
                    self.stage = Stage::Failed {
                        error: i18n::f(
                            Key::PairingFailed,
                            &[("error", &i18n::message(error.map_or("", String::as_str)))],
                        ),
                        retry: Retry::Code,
                    };
                }
            }
            _ => {}
        }
    }

    /// *Try again* after a failure (or *Back* from a mismatch): the address (kept, but a
    /// mismatched link's code and fingerprint are dropped) or the code (empty) again.
    pub fn retry(&mut self) {
        match &self.stage {
            Stage::Failed {
                retry: Retry::Code, ..
            } => match self.offered.clone() {
                Some(fingerprint) => {
                    self.code.clear();
                    self.stage = Stage::Compare {
                        matches_link: self.expected.is_some(),
                        fingerprint,
                    };
                }
                None => self.stage = Stage::Address,
            },
            Stage::Failed { .. } => self.stage = Stage::Address,
            Stage::Mismatch { .. } => {
                self.code.clear();
                self.expected = None;
                self.note = None;
                self.stage = Stage::Address;
            }
            _ => {}
        }
    }

    /// *Back* from comparing: the address again (the code is dropped).
    pub fn back(&mut self) {
        if matches!(self.stage, Stage::Compare { .. }) {
            self.code.clear();
            self.stage = Stage::Address;
        }
    }

    /// The flow is given up: the `disconnect_server` that removes the link it left unpinned
    /// (nothing queued, no mirrored mail), so the profile is a plain one again; `None` when
    /// there is nothing to undo. The code is dropped.
    pub fn cancel(&mut self, state: &AppState) -> Option<Command> {
        self.code.clear();
        let link = state.link.as_ref()?;
        let unpinned = link
            .server
            .as_ref()
            .is_none_or(|server| !server.is_pinned());
        (self.target.is_some() && unpinned && link.queued.is_empty())
            .then_some(Command::DisconnectServer { confirm: true })
    }
}

/// `fingerprint` as the pairing dialog and `dail serve` show it: groups of four hex digits,
/// eight a line.
pub fn fingerprint_lines(fingerprint: &str) -> Vec<String> {
    pairing::fingerprint_lines(fingerprint, 8)
}
