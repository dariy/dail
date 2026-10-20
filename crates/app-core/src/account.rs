//! Accounts: provider presets, server endpoints and secret references (`ARCHITECTURE.md §4.2,
//! §5.3`). Secrets themselves never live in state; see [`Secret`] and [`SecretRef`].

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize, Serializer};

use crate::ids::AccountId;

/// A configured mail account. Holds no secret: the password lives in the OS keyring under
/// [`Account::secret`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Account {
    /// Account ID.
    pub id: AccountId,
    /// Name shown in the UI.
    pub display_name: String,
    /// Email address; also the IMAP and SMTP login.
    pub email: String,
    /// Provider preset the account was created from.
    pub preset: ProviderPreset,
    /// Incoming (IMAP) server.
    pub imap: Endpoint,
    /// Outgoing (SMTP submission) server.
    pub smtp: Endpoint,
    /// Where the password is stored in the OS keyring.
    pub secret: SecretRef,
    /// Result of the last connection test.
    pub status: AccountStatus,
    /// The addresses mail can be sent from (`set_identity`): aliases, and the account's own
    /// address when it has a name or a signature of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identities: Vec<Identity>,
}

/// An address an account sends from, with its name and signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Identity {
    /// The address (`From`).
    pub email: String,
    /// The display name; the account's name if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Added below the text of new drafts from this address, after a `-- ` line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl Account {
    /// The identity for `email` (any case): a configured one, the account's own address, or
    /// `None` if the account does not send from it.
    pub fn identity(&self, email: &str) -> Option<Identity> {
        let configured = self
            .identities
            .iter()
            .find(|i| i.email.eq_ignore_ascii_case(email.trim()))
            .cloned();
        configured.or_else(|| {
            self.email
                .eq_ignore_ascii_case(email.trim())
                .then(|| Identity {
                    email: self.email.clone(),
                    name: None,
                    signature: None,
                })
        })
    }

    /// The `From` address of `identity`: its name, else the account's.
    pub fn from_addr(&self, identity: &Identity) -> crate::message::Addr {
        crate::message::Addr {
            name: identity
                .name
                .clone()
                .or_else(|| Some(self.display_name.clone()))
                .filter(|n| !n.trim().is_empty()),
            email: identity.email.clone(),
        }
    }

    /// What the runtime needs to log in to this account's IMAP server.
    pub fn imap_login(&self) -> ImapLogin {
        ImapLogin {
            login: self.email.clone(),
            server: self.imap.clone(),
            secret: self.secret.clone(),
        }
    }
}

impl Account {
    /// What the runtime needs to authenticate to this account's SMTP server.
    pub fn smtp_login(&self) -> SmtpLogin {
        SmtpLogin {
            login: self.email.clone(),
            server: self.smtp.clone(),
            secret: self.secret.clone(),
        }
    }
}

/// SMTP submission parameters carried by `SmtpSend`. Holds no secret.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SmtpLogin {
    /// SMTP login (the email address).
    pub login: String,
    /// SMTP submission server.
    pub server: Endpoint,
    /// Keyring entry of the password.
    pub secret: SecretRef,
}

/// IMAP connection parameters carried by mail effects. Holds no secret: the runtime reads the
/// password from the keyring entry [`ImapLogin::secret`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ImapLogin {
    /// IMAP login (the email address).
    pub login: String,
    /// IMAP server.
    pub server: Endpoint,
    /// Keyring entry of the password.
    pub secret: SecretRef,
}

/// Result of the last `test_account` run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AccountStatus {
    /// Not tested since it was added or its servers changed.
    #[default]
    Unverified,
    /// IMAP LOGIN and SMTP AUTH both succeeded.
    Verified {
        /// When the test finished, in Unix milliseconds.
        at_ms: i64,
    },
    /// The last test, or storing the password, failed.
    Failed {
        /// Diagnostic, e.g. `IMAP LOGIN failed: …`.
        error: String,
        /// When the failure was reported, in Unix milliseconds.
        at_ms: i64,
    },
}

/// A mail server address. Plaintext connections are not supported.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Endpoint {
    /// Host name, e.g. `imap.gmail.com`.
    pub host: String,
    /// TCP port, e.g. 993 (IMAP over TLS) or 465 (SMTP over TLS).
    pub port: u16,
    /// How the connection is secured.
    pub security: Security,
}

impl Endpoint {
    /// Parses `host:port[:tls|starttls]`. Without a suffix, ports 143 and 587 use STARTTLS and
    /// every other port uses TLS.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (rest, security) = match text.rsplit_once(':') {
            Some((rest, "tls")) => (rest, Some(Security::Tls)),
            Some((rest, "starttls")) => (rest, Some(Security::StartTls)),
            _ => (text, None),
        };
        let (host, port) = rest
            .rsplit_once(':')
            .filter(|(host, _)| !host.is_empty())
            .ok_or_else(|| format!("`{text}`: expected host:port[:tls|starttls]"))?;
        let port: u16 = port
            .parse()
            .map_err(|_| format!("`{text}`: invalid port `{port}`"))?;
        let security = security.unwrap_or(match port {
            143 | 587 => Security::StartTls,
            _ => Security::Tls,
        });
        Ok(Self {
            host: host.to_owned(),
            port,
            security,
        })
    }
}

/// `host:port:tls` or `host:port:starttls`, the form [`Endpoint::parse`] reads back.
impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let security = match self.security {
            Security::Tls => "tls",
            Security::StartTls => "starttls",
        };
        write!(f, "{}:{}:{security}", self.host, self.port)
    }
}

/// Transport security of an [`Endpoint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Security {
    /// TLS from the first byte (IMAP 993, SMTP 465).
    Tls,
    /// Plain connection upgraded with STARTTLS before login (IMAP 143, SMTP 587).
    #[serde(rename = "starttls")]
    StartTls,
}

/// Provider preset. Provider differences are data ([`PresetProfile`]), not code branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPreset {
    /// Hostinger Mail (imap/smtp.hostinger.com).
    Hostinger,
    /// Gmail with an App Password (requires 2-step verification).
    Gmail,
    /// Any other IMAP/SMTP provider; servers must be given explicitly.
    Custom,
}

/// A server default of a preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ServerDefault {
    /// Host name.
    pub host: &'static str,
    /// TCP port.
    pub port: u16,
    /// Transport security.
    pub security: Security,
}

impl ServerDefault {
    /// The default as an [`Endpoint`].
    pub fn to_endpoint(self) -> Endpoint {
        Endpoint {
            host: self.host.to_owned(),
            port: self.port,
            security: self.security,
        }
    }
}

/// What a preset knows about its provider (`ARCHITECTURE.md §5.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PresetProfile {
    /// Default IMAP server (`None`: must be supplied).
    pub imap: Option<ServerDefault>,
    /// Default SMTP server (`None`: must be supplied).
    pub smtp: Option<ServerDefault>,
    /// The SMTP server stores a copy of sent mail itself, so no APPEND to Sent is needed.
    pub smtp_saves_sent: bool,
    /// Mailbox that `archive` moves to if no mailbox has the `archive` role.
    pub archive_mailbox: Option<&'static str>,
    /// Archiving moves to the mailbox with the `all` role (Gmail: archive = remove from INBOX).
    pub archive_to_all: bool,
    /// Create the archive mailbox if it is missing.
    pub create_archive: bool,
    /// Mailbox that `trash` moves to (`None`: SPECIAL-USE `\Trash`).
    pub trash_mailbox: Option<&'static str>,
    /// Server searches use Gmail's own query language (`X-GM-RAW`) instead of IMAP `SEARCH`
    /// criteria.
    pub raw_search: bool,
}

const HOSTINGER: PresetProfile = PresetProfile {
    imap: Some(ServerDefault {
        host: "imap.hostinger.com",
        port: 993,
        security: Security::Tls,
    }),
    smtp: Some(ServerDefault {
        host: "smtp.hostinger.com",
        port: 465,
        security: Security::Tls,
    }),
    smtp_saves_sent: false,
    archive_mailbox: Some("Archive"),
    archive_to_all: false,
    create_archive: true,
    trash_mailbox: None,
    raw_search: false,
};

const GMAIL: PresetProfile = PresetProfile {
    imap: Some(ServerDefault {
        host: "imap.gmail.com",
        port: 993,
        security: Security::Tls,
    }),
    smtp: Some(ServerDefault {
        host: "smtp.gmail.com",
        port: 465,
        security: Security::Tls,
    }),
    smtp_saves_sent: true,
    archive_mailbox: Some("[Gmail]/All Mail"),
    archive_to_all: true,
    create_archive: false,
    trash_mailbox: Some("[Gmail]/Trash"),
    raw_search: true,
};

const CUSTOM: PresetProfile = PresetProfile {
    imap: None,
    smtp: None,
    smtp_saves_sent: false,
    archive_mailbox: None,
    archive_to_all: false,
    create_archive: false,
    trash_mailbox: None,
    raw_search: false,
};

impl ProviderPreset {
    /// Every preset.
    pub const ALL: &'static [Self] = &[Self::Hostinger, Self::Gmail, Self::Custom];

    /// The preset's `snake_case` name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hostinger => "hostinger",
            Self::Gmail => "gmail",
            Self::Custom => "custom",
        }
    }

    /// The provider data of this preset.
    pub const fn profile(self) -> &'static PresetProfile {
        match self {
            Self::Hostinger => &HOSTINGER,
            Self::Gmail => &GMAIL,
            Self::Custom => &CUSTOM,
        }
    }
}

/// Where a secret lives in the OS keyring. Safe to store and log; the secret itself is not here.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct SecretRef {
    /// Keyring service name (always `dail`).
    pub service: String,
    /// Keyring user name, e.g. `account/<account id>`.
    pub user: String,
}

impl SecretRef {
    /// The keyring entry of an account's password.
    pub fn for_account(id: AccountId) -> Self {
        Self {
            service: "dail".to_owned(),
            user: format!("account/{id}"),
        }
    }
}

/// A password in transit from a front-end to the keyring.
///
/// Deserializes from a plain string, but `Debug` and `Serialize` print `<redacted>`, so a secret
/// can't leak through logs, receipts, replay logs or snapshots. Use [`Secret::expose`] to read it.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// What `Debug` and `Serialize` print instead of the secret.
    pub const REDACTED: &'static str = "<redacted>";

    /// Wraps a secret value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The secret value. Pass it straight to the keyring or the server; never log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({})", Self::REDACTED)
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::REDACTED)
    }
}

impl JsonSchema for Secret {
    fn schema_name() -> Cow<'static, str> {
        "Secret".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "writeOnly": true,
        })
    }
}
