//! Pairing a device with a dail server (ADR 0017, SRV-QR1): the `dail://pair` URI that the QR
//! code of *Pair a device…* holds, its strict parser, and the dialog's state machine that the
//! GUI and the TUI draw.
//!
//! # The URI
//!
//! `dail://pair?v=1&h=<host>&p=<port>&c=<code>&fp=<sha256 hex>&n=<name>`, written by
//! [`PairingUri::to_uri`] in that order. A phone scans it from any camera, so [`parse_uri`]
//! treats it as hostile input and accepts only what `to_uri` could have written:
//! - at most [`MAX_URI_BYTES`] bytes, checked before anything else;
//! - the prefix exactly `dail://pair?` (lower case), no fragment, only the characters of an
//!   RFC 3986 query (anything else, a space or a non-ASCII letter, must be percent-encoded);
//! - parameters `v`, `h`, `p`, `c`, `fp` and `n`, each exactly once, in any order, no other
//!   parameter and no empty one; `v` exactly `1`; values percent-decoded as UTF-8 (a bad escape
//!   or invalid UTF-8 is refused), without a raw `=`;
//! - `h`: a DNS name (letters, digits and `-`, labels of 1–63, at most 253 bytes, the last
//!   label not all digits; lower-cased), an IPv4 address, or an IPv6 address **without
//!   brackets** (raw or as `%3A`, which `to_uri` writes; zone IDs are refused). IP addresses are normalised
//!   to their canonical text;
//! - `p`: 1–65535 in decimal, without a leading zero;
//! - `c`: exactly 8 RFC 4648 base32 characters, case-insensitive, normalised to upper case;
//! - `fp`: exactly 64 hex digits, normalised to lower case;
//! - `n`: 1–[`MAX_NAME_CHARS`] characters after decoding, no control or bidirectional
//!   formatting character, no line separator, no leading or trailing white space.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use app_core::{AppState, Command, CommandReceipt, DeviceRole, ServeInfo};

use crate::qr::QrMatrix;

/// The longest URI [`parse_uri`] reads. The longest valid one (a 253-byte host and a name of 64
/// four-byte characters, each byte percent-encoded) is about 1130 bytes.
pub const MAX_URI_BYTES: usize = 1280;

/// The longest device name, in characters (as `start_pairing`).
pub const MAX_NAME_CHARS: usize = 64;

/// The URI's fixed start.
const PREFIX: &str = "dail://pair?";

/// Why a text is not a pairing URI. The messages never repeat the text: it is untrusted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairingError {
    /// Longer than [`MAX_URI_BYTES`].
    #[error("ERR462: the pairing link is too long")]
    TooLong,
    /// Does not start with `dail://pair?`.
    #[error("ERR463: not a dail pairing link (dail://pair?…)")]
    NotPairing,
    /// A character, an escape or a parameter that is not well formed.
    #[error("ERR464: the pairing link is malformed: {0}")]
    Malformed(&'static str),
    /// A parameter is missing.
    #[error("ERR465: the pairing link has no `{0}`")]
    Missing(&'static str),
    /// A parameter appears twice.
    #[error("ERR466: the pairing link has `{0}` twice")]
    Duplicate(&'static str),
    /// A parameter dail does not know.
    #[error("ERR467: the pairing link has an unknown parameter")]
    Unknown,
    /// `v` is not `1`.
    #[error("ERR468: the pairing link is of a version this dail does not read")]
    Version,
    /// `h` is not a DNS name or an IP address.
    #[error("ERR469: the pairing link's host is not a DNS name or an IP address")]
    Host,
    /// `p` is not 1–65535.
    #[error("ERR470: the pairing link's port is not 1 to 65535")]
    Port,
    /// `c` is not 8 base32 characters.
    #[error("ERR471: the pairing link's code is not 8 characters A-Z, 2-7")]
    Code,
    /// `fp` is not 64 hex digits.
    #[error("ERR472: the pairing link's fingerprint is not 64 hex digits")]
    Fingerprint,
    /// `n` is empty, too long or has a control character.
    #[error("ERR473: the pairing link's device name is not 1 to 64 printable characters")]
    Name,
    /// The receipt of `start_pairing` holds no code (a server of another version).
    #[error("ERR474: the server's answer has no pairing code")]
    NoCode,
}

/// What a device needs to pair with a dail server: where it is, the single-use code, the
/// certificate to pin, and the name it was given. Always valid: built by [`PairingUri::new`] or
/// [`parse_uri`].
#[derive(Clone, PartialEq, Eq)]
pub struct PairingUri {
    host: String,
    port: u16,
    code: String,
    fingerprint: String,
    name: String,
}

impl std::fmt::Debug for PairingUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingUri")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("code", &"<redacted>")
            .field("fingerprint", &self.fingerprint)
            .field("name", &self.name)
            .finish()
    }
}

impl PairingUri {
    /// Checks and normalises the parts as [`parse_uri`] does; `host` may also be an IPv6 address
    /// in brackets (as `state.link.server.host` can be), and `fingerprint` may have colons.
    pub fn new(
        host: &str,
        port: u16,
        code: &str,
        fingerprint: &str,
        name: &str,
    ) -> Result<Self, PairingError> {
        let bare = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .filter(|h| h.contains(':'))
            .unwrap_or(host);
        let fingerprint: String = fingerprint.chars().filter(|c| *c != ':').collect();
        Ok(Self {
            host: check_host(bare)?,
            port: check_port_number(port)?,
            code: check_code(code)?,
            fingerprint: check_fingerprint(&fingerprint)?,
            name: check_name(name)?,
        })
    }

    /// The server's host: a lower-case DNS name, or an IP address (IPv6 without brackets).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The server's port.
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// The pairing code, upper case. Shown to the user by design; never log it.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The SHA-256 fingerprint of the server's certificate, lower-case hex.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The name the device was given in `start_pairing`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The URI: `dail://pair?v=1&h=…&p=…&c=…&fp=…&n=…`, every byte of a value but the RFC 3986
    /// unreserved ones percent-encoded.
    pub fn to_uri(&self) -> String {
        format!(
            "{PREFIX}v=1&h={}&p={}&c={}&fp={}&n={}",
            encode(&self.host),
            self.port,
            self.code,
            self.fingerprint,
            encode(&self.name)
        )
    }
}

/// Whether `b` may stand unencoded in a value written by [`PairingUri::to_uri`] (RFC 3986
/// unreserved).
const fn unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

/// Whether `b` may stand raw in the query of a pairing URI (RFC 3986 `query` without `=` and
/// `&`, which separate the parameters, and with `%` for escapes).
const fn query_byte(b: u8) -> bool {
    unreserved(b)
        || matches!(
            b,
            b'!' | b'$'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b':'
                | b'@'
                | b'/'
                | b'?'
                | b'%'
        )
}

fn encode(text: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if unreserved(b) {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Strict percent-decoding: `%` needs two hex digits, and the bytes must be UTF-8.
fn decode(value: &str) -> Result<String, PairingError> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b == b'%' {
            let hex = |at: usize| {
                bytes
                    .get(at)
                    .and_then(|d| char::from(*d).to_digit(16))
                    .and_then(|d| u8::try_from(d).ok())
            };
            let (Some(high), Some(low)) = (hex(i + 1), hex(i + 2)) else {
                return Err(PairingError::Malformed("a % escape needs two hex digits"));
            };
            out.push(high << 4 | low);
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| PairingError::Malformed("an escape is not UTF-8"))
}

/// Parses a scanned `dail://pair` URI strictly (the module's rules). The text is untrusted.
pub fn parse_uri(text: &str) -> Result<PairingUri, PairingError> {
    if text.len() > MAX_URI_BYTES {
        return Err(PairingError::TooLong);
    }
    let query = text.strip_prefix(PREFIX).ok_or(PairingError::NotPairing)?;
    if let Some(b) = query
        .bytes()
        .find(|b| !query_byte(*b) && !matches!(b, b'=' | b'&'))
    {
        return Err(PairingError::Malformed(if b == b'#' {
            "a fragment (#) is not allowed"
        } else {
            "a character must be percent-encoded"
        }));
    }
    const NAMES: [&str; 6] = ["v", "h", "p", "c", "fp", "n"];
    let mut values: [Option<String>; 6] = Default::default();
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(PairingError::Malformed("a parameter has no value"));
        };
        if value.contains('=') {
            return Err(PairingError::Malformed("a value has a raw ="));
        }
        let Some(index) = NAMES.iter().position(|n| *n == key) else {
            return Err(PairingError::Unknown);
        };
        let (Some(slot), Some(name)) = (values.get_mut(index), NAMES.get(index)) else {
            return Err(PairingError::Unknown);
        };
        if slot.is_some() {
            return Err(PairingError::Duplicate(name));
        }
        *slot = Some(decode(value)?);
    }
    let mut take = |index: usize| -> Result<String, PairingError> {
        let name = NAMES.get(index).copied().unwrap_or("?");
        values
            .get_mut(index)
            .and_then(Option::take)
            .ok_or(PairingError::Missing(name))
    };
    let (v, h, p, c, fp, n) = (take(0)?, take(1)?, take(2)?, take(3)?, take(4)?, take(5)?);
    if v != "1" {
        return Err(PairingError::Version);
    }
    Ok(PairingUri {
        host: check_host(&h)?,
        port: check_port(&p)?,
        code: check_code(&c)?,
        fingerprint: check_fingerprint(&fp)?,
        name: check_name(&n)?,
    })
}

/// A host a pairing URI may name (the module's rules, IPv6 in brackets too), normalised.
pub fn valid_host(host: &str) -> Result<String, PairingError> {
    check_host(&bare_host(host))
}

/// A DNS name (lower-cased), an IPv4 address or a bare IPv6 address, in canonical form.
fn check_host(host: &str) -> Result<String, PairingError> {
    if host.contains(':') {
        return host
            .parse::<Ipv6Addr>()
            .map(|ip| ip.to_string())
            .map_err(|_| PairingError::Host);
    }
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Ok(ip.to_string());
    }
    let labels: Vec<&str> = host.split('.').collect();
    let label_ok = |label: &&str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let numeric_tld = labels
        .last()
        .is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()));
    if host.is_empty() || host.len() > 253 || numeric_tld || !labels.iter().all(label_ok) {
        return Err(PairingError::Host);
    }
    Ok(host.to_ascii_lowercase())
}

fn check_port(port: &str) -> Result<u16, PairingError> {
    if port.is_empty() || port.len() > 5 || port.starts_with('0') {
        return Err(PairingError::Port);
    }
    if !port.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PairingError::Port);
    }
    port.parse::<u16>()
        .map_err(|_| PairingError::Port)
        .and_then(check_port_number)
}

const fn check_port_number(port: u16) -> Result<u16, PairingError> {
    if port == 0 {
        Err(PairingError::Port)
    } else {
        Ok(port)
    }
}

fn check_code(code: &str) -> Result<String, PairingError> {
    let upper = code.to_ascii_uppercase();
    if app_core::link::is_pairing_code(&upper) {
        Ok(upper)
    } else {
        Err(PairingError::Code)
    }
}

fn check_fingerprint(fingerprint: &str) -> Result<String, PairingError> {
    if fingerprint.len() == 64 && fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(fingerprint.to_ascii_lowercase())
    } else {
        Err(PairingError::Fingerprint)
    }
}

/// Characters a device name may not have: controls, bidirectional and zero-width formatting
/// (including the Arabic letter mark and the Mongolian vowel separator), line and paragraph
/// separators (they could make a name read as something else). The front ends drop them as
/// they are typed or pasted.
pub fn forbidden_in_name(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

fn check_name(name: &str) -> Result<String, PairingError> {
    let count = name.chars().count();
    if count == 0
        || count > MAX_NAME_CHARS
        || name.trim() != name
        || name.chars().any(forbidden_in_name)
    {
        return Err(PairingError::Name);
    }
    Ok(name.to_owned())
}

/// Where a device reaches the dail server, for the QR code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Host (IPv6 without brackets).
    pub host: String,
    /// Port.
    pub port: u16,
    /// SHA-256 fingerprint of the server's certificate, hex.
    pub fingerprint: String,
}

/// Whether this front end can open a pairing code: it is attached to a running `dail serve`
/// (`status.serve`), or its profile mirrors a server whose certificate it pinned (the code is
/// then opened on the server, `start_pairing` being a server command).
pub fn can_pair(state: &AppState) -> bool {
    pinned_server(state).is_some() || state.status.serve.is_some()
}

fn pinned_server(state: &AppState) -> Option<&app_core::ServerEndpoint> {
    state
        .link
        .as_ref()
        .and_then(|link| link.server.as_ref())
        .filter(|server| server.is_pinned())
}

/// Where a new device should dial, for the QR code:
/// - on a device that mirrors a server: the host and port it reaches the server by
///   (`state.link.server`) and the fingerprint it pinned, so the phone dials what demonstrably
///   works and pins what this device pinned;
/// - on the server itself (`status.serve`): the first advertised host that is a valid host
///   (with the first listener's port), else the first bound address that is neither loopback
///   nor unspecified, else the first loopback one (an emulator or `adb reverse` reaches it);
///   `0.0.0.0` and `::` are never dialable.
///
/// `None` when neither is known.
pub fn endpoint(state: &AppState) -> Option<Endpoint> {
    if let Some(server) = pinned_server(state) {
        return Some(Endpoint {
            host: bare_host(&server.host),
            port: server.port,
            fingerprint: server.fingerprint.clone()?,
        });
    }
    serve_endpoint(state.status.serve.as_ref()?)
}

fn bare_host(host: &str) -> String {
    host.strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host)
        .to_owned()
}

/// The endpoint of a running `dail serve` by the rules of [`endpoint`] (also `dail serve pair`'s).
pub fn serve_endpoint(serve: &ServeInfo) -> Option<Endpoint> {
    let bound: Vec<std::net::SocketAddr> = serve
        .addresses
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect();
    let endpoint = |host: String, port: u16| Endpoint {
        host,
        port,
        fingerprint: serve.fingerprint.clone(),
    };
    if let Some(first) = bound.first()
        && let Some(host) = serve
            .advertise
            .iter()
            .find_map(|h| check_host(&bare_host(h)).ok())
    {
        return Some(endpoint(host, first.port()));
    }
    let dialable = |ip: &IpAddr| !ip.is_unspecified();
    bound
        .iter()
        .find(|a| dialable(&a.ip()) && !a.ip().is_loopback())
        .or_else(|| bound.iter().find(|a| a.ip().is_loopback()))
        .map(|a| endpoint(a.ip().to_string(), a.port()))
}

/// A pairing code the server opened, as the dialog shows it. It lives only in the dialog (the
/// receipt of `start_pairing`), never in `AppState`, and is never logged.
#[derive(Clone, PartialEq, Eq)]
pub struct Opened {
    /// The code, upper case.
    pub code: String,
    /// The role it pairs.
    pub role: DeviceRole,
    /// The device's name.
    pub name: String,
    /// When the code expires, Unix ms.
    pub expires_at_ms: i64,
    /// Where the device dials, if known.
    pub endpoint: Option<Endpoint>,
    /// The `dail://pair` URI, when the endpoint is known.
    pub uri: Option<String>,
    /// The URI's QR code.
    pub qr: Option<QrMatrix>,
    /// Why there is no link or QR code although the endpoint is known (already in words).
    pub link_error: Option<String>,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("role", &self.role)
            .field("name", &self.name)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl Opened {
    /// Milliseconds until the code expires (0 once it has).
    pub fn remaining_ms(&self, now_ms: i64) -> i64 {
        self.expires_at_ms.saturating_sub(now_ms).max(0)
    }
}

/// The time left as `m:ss`.
pub fn countdown(remaining_ms: i64) -> String {
    let secs = remaining_ms.max(0).saturating_add(999) / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Where the dialog is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// Choosing the role and typing the name.
    Form,
    /// `start_pairing` was submitted; waiting for its receipt.
    Waiting,
    /// The server opened a code.
    Opened(Box<Opened>),
    /// The command was refused, or the server could not open the code.
    Failed(String),
}

/// The *Pair a device…* dialog (GUI and TUI): role and name, then `start_pairing`, then the
/// code, its expiry, the fingerprint and the QR code. The front ends draw it and feed it the
/// reply; the rules are here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairDialog {
    /// The role chosen.
    pub role: DeviceRole,
    /// The name typed.
    pub name: String,
    stage: Stage,
    /// `status.serve_error` when the command was submitted: a new one is this pairing's failure.
    serve_error: Option<String>,
}

impl Default for PairDialog {
    fn default() -> Self {
        Self {
            role: DeviceRole::User,
            name: String::new(),
            stage: Stage::Form,
            serve_error: None,
        }
    }
}

impl PairDialog {
    /// The dialog for a `start_pairing` submitted elsewhere (the palette's questions, a key):
    /// it waits for that command's reply.
    pub fn submitted(role: DeviceRole, name: &str, state: &AppState) -> Self {
        Self {
            role,
            name: name.to_owned(),
            stage: Stage::Waiting,
            serve_error: state.status.serve_error.clone(),
        }
    }

    /// Where the dialog is.
    pub const fn stage(&self) -> &Stage {
        &self.stage
    }

    /// Whether *Pair* may be pressed: on the form, with a name of 1–64 characters.
    pub fn can_submit(&self) -> bool {
        self.stage == Stage::Form && check_name(self.name.trim()).is_ok()
    }

    /// Submits the form: the command to send (with the reply routed back to [`Self::accept`]),
    /// or `None` if it cannot be submitted.
    pub fn submit(&mut self, state: &AppState) -> Option<Command> {
        if !self.can_submit() {
            return None;
        }
        self.serve_error.clone_from(&state.status.serve_error);
        self.stage = Stage::Waiting;
        Some(Command::StartPairing {
            role: self.role,
            name: self.name.trim().to_owned(),
        })
    }

    /// The reply to the submitted command: its receipt, or why it failed (already in words).
    pub fn accept(&mut self, result: Result<&CommandReceipt, String>, state: &AppState) {
        if self.stage != Stage::Waiting {
            return;
        }
        self.stage = match result {
            Ok(receipt) => opened(receipt, state).map_or_else(
                || Stage::Failed(PairingError::NoCode.to_string()),
                |opened| Stage::Opened(Box::new(opened)),
            ),
            Err(error) => Stage::Failed(error),
        };
    }

    /// Follows the state: a new `status.serve_error` after the code was asked for means the
    /// server could not open it.
    pub fn sync(&mut self, state: &AppState) {
        if matches!(self.stage, Stage::Waiting | Stage::Opened(_))
            && let Some(error) = &state.status.serve_error
            && self.serve_error.as_ref() != Some(error)
        {
            self.stage = Stage::Failed(error.clone());
        }
    }

    /// Back to the form, for another device (the name is cleared, the role kept).
    pub fn again(&mut self) {
        self.name.clear();
        self.stage = Stage::Form;
    }
}

/// The opened code from the receipt of `start_pairing`, with the URI and its QR code when the
/// endpoint is known.
fn opened(receipt: &CommandReceipt, state: &AppState) -> Option<Opened> {
    let data = &receipt.data;
    let code = check_code(data["code"].as_str()?).ok()?;
    let role: DeviceRole = serde_json::from_value(data["role"].clone()).ok()?;
    let name = data["name"].as_str()?.to_owned();
    let expires_at_ms = data["expires_at_ms"].as_i64()?;
    let endpoint = endpoint(state);
    let (uri, qr, link_error) = match &endpoint {
        None => (None, None, None),
        Some(e) => match PairingUri::new(&e.host, e.port, &code, &e.fingerprint, &name) {
            Err(err) => (None, None, Some(err.to_string())),
            Ok(uri) => {
                let text = uri.to_uri();
                match QrMatrix::encode(&text) {
                    Ok(qr) => (Some(text), Some(qr), None),
                    Err(err) => (Some(text), None, Some(err.to_string())),
                }
            }
        },
    };
    Some(Opened {
        code,
        role,
        name,
        expires_at_ms,
        endpoint,
        uri,
        qr,
        link_error,
    })
}

/// `fingerprint` in groups of four hex digits, `per_line` groups a line (for narrow dialogs).
pub fn fingerprint_lines(fingerprint: &str, per_line: usize) -> Vec<String> {
    let groups: Vec<&str> = fingerprint
        .as_bytes()
        .chunks(4)
        .filter_map(|c| std::str::from_utf8(c).ok())
        .collect();
    groups
        .chunks(per_line.max(1))
        .map(|line| line.join(" "))
        .collect()
}
