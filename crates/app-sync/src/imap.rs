//! IMAP client operations used by `LiveBackend` (`ARCHITECTURE.md §5.2`).
//!
//! Connections are opened through an [`ImapConnector`]: [`NetImap`] in production (TCP + TLS or
//! STARTTLS, then LOGIN), scripted in-memory servers in tests. Protocol operations work on an
//! [`ImapSession`] over any boxed stream, so they are tested without a network.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use app_core::{
    Endpoint, MailboxRole, RemoteEnvelope, RemoteMailbox, RemoteStep, Secret, Security,
};
use async_imap::imap_proto::{Response, Status};
use async_imap::types::{Fetch, Flag, NameAttribute};
use futures_util::TryStreamExt as _;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::backend::BoxFuture;
use crate::error::SyncError;
use crate::tls;

/// A byte stream an IMAP session can run over.
pub trait ImapStream: AsyncRead + AsyncWrite + Unpin + Send + fmt::Debug {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + fmt::Debug> ImapStream for T {}

/// An authenticated IMAP session over a boxed stream.
pub type ImapSession = async_imap::Session<Box<dyn ImapStream>>;

/// Default limit for connecting and logging in.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Opens authenticated IMAP sessions.
pub trait ImapConnector: Send + Sync + 'static {
    /// Connects to `server` and logs in. Errors never contain the password.
    fn connect<'a>(
        &'a self,
        server: &'a Endpoint,
        login: &'a str,
        password: &'a Secret,
    ) -> BoxFuture<'a, Result<ImapSession, String>>;
}

/// Connects to real servers over TLS (or STARTTLS) with the Mozilla roots.
#[derive(Clone)]
pub struct NetImap {
    tls: Arc<ClientConfig>,
    timeout: Duration,
}

impl fmt::Debug for NetImap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetImap")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl NetImap {
    /// Uses the Mozilla root certificates and [`CONNECT_TIMEOUT`].
    pub fn new() -> Result<Self, SyncError> {
        Ok(Self::with_tls(tls::client_config()?))
    }

    /// Uses the given TLS configuration.
    pub fn with_tls(tls: Arc<ClientConfig>) -> Self {
        Self {
            tls,
            timeout: CONNECT_TIMEOUT,
        }
    }

    /// Changes the connect + login timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl ImapConnector for NetImap {
    fn connect<'a>(
        &'a self,
        server: &'a Endpoint,
        login: &'a str,
        password: &'a Secret,
    ) -> BoxFuture<'a, Result<ImapSession, String>> {
        Box::pin(async move {
            let open = connect_login(&self.tls, server, login, password);
            match tokio::time::timeout(self.timeout, open).await {
                Ok(result) => result,
                Err(_) => Err(format!("IMAP {}: timed out", address(server))),
            }
        })
    }
}

/// `host:port`, for diagnostics.
pub fn address(endpoint: &Endpoint) -> String {
    format!("{}:{}", endpoint.host, endpoint.port)
}

/// TCP connect, TLS (or STARTTLS), greeting, LOGIN.
pub async fn connect_login(
    tls: &Arc<ClientConfig>,
    endpoint: &Endpoint,
    login: &str,
    password: &Secret,
) -> Result<ImapSession, String> {
    let at = address(endpoint);
    let server_name = ServerName::try_from(endpoint.host.clone())
        .map_err(|err| format!("IMAP {at}: invalid host name: {err}"))?;
    let connector = TlsConnector::from(Arc::clone(tls));
    let tcp = TcpStream::connect((endpoint.host.as_str(), endpoint.port))
        .await
        .map_err(|err| format!("IMAP {at}: connect failed: {err}"))?;
    match endpoint.security {
        Security::Tls => {
            let stream = connector.connect(server_name, tcp).await.map_err(|err| {
                tls::with_hint(format!("IMAP {at}: TLS handshake failed: {err}"), &err)
            })?;
            greet_and_login(Box::new(stream), login, password, &at).await
        }
        Security::StartTls => {
            let mut client = async_imap::Client::new(tcp);
            greeting(&mut client, &at).await?;
            client
                .run_command_and_check_ok("STARTTLS", None)
                .await
                .map_err(|err| format!("IMAP {at}: STARTTLS refused: {err}"))?;
            let stream = connector
                .connect(server_name, client.into_inner())
                .await
                .map_err(|err| {
                    tls::with_hint(format!("IMAP {at}: TLS handshake failed: {err}"), &err)
                })?;
            // No greeting after STARTTLS: log in right away.
            login_with(
                async_imap::Client::new(Box::new(stream) as Box<dyn ImapStream>),
                login,
                password,
            )
            .await
        }
    }
}

/// Reads the server greeting on an established (already secure) stream, then logs in.
pub async fn greet_and_login(
    stream: Box<dyn ImapStream>,
    login: &str,
    password: &Secret,
    at: &str,
) -> Result<ImapSession, String> {
    let mut client = async_imap::Client::new(stream);
    greeting(&mut client, at).await?;
    login_with(client, login, password).await
}

async fn login_with(
    client: async_imap::Client<Box<dyn ImapStream>>,
    login: &str,
    password: &Secret,
) -> Result<ImapSession, String> {
    client
        .login(login, password.expose())
        .await
        .map_err(|(err, _client)| format!("IMAP LOGIN failed: {err}"))
}

/// Reads the server greeting; it must be `* OK` (or `* PREAUTH`).
async fn greeting<T>(client: &mut async_imap::Client<T>, at: &str) -> Result<(), String>
where
    T: AsyncRead + AsyncWrite + Unpin + fmt::Debug + Send,
{
    let response = client
        .read_response()
        .await
        .map_err(|err| format!("IMAP {at}: reading greeting failed: {err}"))?
        .ok_or_else(|| format!("IMAP {at}: server closed the connection"))?;
    match response.parsed() {
        Response::Data {
            status: Status::Ok | Status::PreAuth,
            ..
        } => Ok(()),
        _ => Err(format!("IMAP {at}: unexpected greeting")),
    }
}

/// `LIST "" "*"`: every mailbox with its SPECIAL-USE attribute, in server order.
pub async fn list_mailboxes(session: &mut ImapSession) -> Result<Vec<RemoteMailbox>, String> {
    let names: Vec<_> = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|err| format!("IMAP LIST failed: {err}"))?
        .try_collect()
        .await
        .map_err(|err| format!("IMAP LIST failed: {err}"))?;
    // Every IMAP account has an INBOX (RFC 3501 §5.1), so a listing without one was cut short.
    // That holds for IMAP only: a backend whose accounts can lack an INBOX needs its own check.
    if !names
        .iter()
        .any(|name| name.name().eq_ignore_ascii_case("INBOX"))
    {
        return Err("IMAP LIST returned no INBOX (connection lost?)".to_owned());
    }
    Ok(names
        .iter()
        .map(|name| {
            let attributes = name.attributes();
            RemoteMailbox {
                path: name.name().to_owned(),
                delimiter: name.delimiter().map(str::to_owned),
                special_use: attributes.iter().find_map(special_use),
                selectable: !attributes.iter().any(|attribute| match attribute {
                    NameAttribute::NoSelect => true,
                    NameAttribute::Extension(ext) => ext.eq_ignore_ascii_case("\\NonExistent"),
                    _ => false,
                }),
            }
        })
        .collect())
}

fn special_use(attribute: &NameAttribute<'_>) -> Option<MailboxRole> {
    match attribute {
        NameAttribute::All => Some(MailboxRole::All),
        NameAttribute::Archive => Some(MailboxRole::Archive),
        NameAttribute::Drafts => Some(MailboxRole::Drafts),
        NameAttribute::Flagged => Some(MailboxRole::Flagged),
        NameAttribute::Junk => Some(MailboxRole::Junk),
        NameAttribute::Sent => Some(MailboxRole::Sent),
        NameAttribute::Trash => Some(MailboxRole::Trash),
        // Pre-RFC 6154 Gmail-style attributes such as `\Spam` arrive as extensions.
        NameAttribute::Extension(ext) => match ext.to_ascii_lowercase().as_str() {
            "\\spam" => Some(MailboxRole::Junk),
            "\\allmail" => Some(MailboxRole::All),
            "\\starred" => Some(MailboxRole::Flagged),
            _ => None,
        },
        _ => None,
    }
}

/// Header fields fetched for envelopes.
const ENVELOPE_QUERY: &str = "(UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (MESSAGE-ID \
                              IN-REPLY-TO REFERENCES FROM TO CC SUBJECT DATE CONTENT-TYPE)])";

/// What `fetch_envelopes` found in a mailbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedMailbox {
    /// UIDVALIDITY on the server.
    pub uid_validity: u32,
    /// UIDNEXT on the server, if reported.
    pub uid_next: Option<u32>,
    /// Messages in the mailbox.
    pub total: u32,
    /// Unread messages in the mailbox.
    pub unread: u32,
    /// Fetched envelopes, ascending by UID.
    pub envelopes: Vec<RemoteEnvelope>,
}

/// `EXAMINE` the mailbox, count unread messages, then fetch envelopes: every UID from
/// `since_uid` on if `uid_validity` still matches, else the newest `limit` messages. At most the
/// newest `limit` envelopes are returned either way.
pub async fn fetch_envelopes(
    session: &mut ImapSession,
    path: &str,
    uid_validity: Option<u32>,
    since_uid: Option<u32>,
    limit: u32,
) -> Result<FetchedMailbox, String> {
    let selected = session
        .examine(path)
        .await
        .map_err(|err| format!("IMAP EXAMINE {path} failed: {err}"))?;
    let server_validity = selected
        .uid_validity
        .ok_or_else(|| format!("IMAP EXAMINE {path}: server reported no UIDVALIDITY"))?;
    let unread = session
        .uid_search("UNSEEN")
        .await
        .map_err(|err| format!("IMAP SEARCH UNSEEN in {path} failed: {err}"))?
        .len();
    let mut fetched = FetchedMailbox {
        uid_validity: server_validity,
        uid_next: selected.uid_next,
        total: selected.exists,
        unread: u32::try_from(unread).unwrap_or(u32::MAX),
        envelopes: Vec::new(),
    };
    if selected.exists == 0 || limit == 0 {
        return Ok(fetched);
    }
    let incremental = match since_uid {
        Some(since) if uid_validity == Some(server_validity) => Some(since.max(1)),
        _ => None,
    };
    let responses: Vec<Fetch> = match incremental {
        Some(since) => {
            if selected.uid_next.is_some_and(|next| next <= since) {
                // Nothing new; skip the round trip.
                return Ok(fetched);
            }
            session
                .uid_fetch(format!("{since}:*"), ENVELOPE_QUERY)
                .await
                .map_err(|err| format!("IMAP UID FETCH in {path} failed: {err}"))?
                .try_collect()
                .await
        }
        None => {
            let start = selected.exists.saturating_sub(limit).saturating_add(1);
            session
                .fetch(format!("{start}:*"), ENVELOPE_QUERY)
                .await
                .map_err(|err| format!("IMAP FETCH in {path} failed: {err}"))?
                .try_collect()
                .await
        }
    }
    .map_err(|err| format!("IMAP FETCH in {path} failed: {err}"))?;

    let mut envelopes: Vec<RemoteEnvelope> = responses
        .iter()
        .filter_map(|fetch| {
            let uid = fetch.uid?;
            // `n:*` returns the last message even if its UID is below `n`.
            if incremental.is_some_and(|since| uid < since) {
                return None;
            }
            Some(envelope_of(uid, fetch))
        })
        .collect();
    envelopes.sort_by_key(|envelope| envelope.uid);
    envelopes.dedup_by_key(|envelope| envelope.uid);
    let keep = usize::try_from(limit).unwrap_or(usize::MAX);
    if envelopes.len() > keep {
        envelopes.drain(..envelopes.len() - keep);
    }
    fetched.envelopes = envelopes;
    Ok(fetched)
}

/// Envelopes of up to `limit` messages just older than `before_uid`: the messages at sequence
/// numbers below `before_uid`'s (or, if it was expunged, the highest UIDs below it). Returns the
/// server's UIDVALIDITY, the envelopes ascending by UID and whether the oldest message is among
/// them. Nothing is fetched if UIDVALIDITY changed.
pub async fn fetch_older(
    session: &mut ImapSession,
    path: &str,
    uid_validity: u32,
    before_uid: u32,
    limit: u32,
) -> Result<(u32, Vec<RemoteEnvelope>, bool), String> {
    let failed =
        |what: &str, err: async_imap::error::Error| format!("IMAP {what} in {path} failed: {err}");
    let selected = session
        .examine(path)
        .await
        .map_err(|err| format!("IMAP EXAMINE {path} failed: {err}"))?;
    let server_validity = selected
        .uid_validity
        .ok_or_else(|| format!("IMAP EXAMINE {path}: server reported no UIDVALIDITY"))?;
    if server_validity != uid_validity || before_uid <= 1 || limit == 0 {
        return Ok((server_validity, Vec::new(), server_validity == uid_validity));
    }
    // The sequence number of the oldest cached message: everything below it is older.
    let probe: Vec<Fetch> = session
        .uid_fetch(before_uid.to_string(), "(UID)")
        .await
        .map_err(|err| failed("UID FETCH", err))?
        .try_collect()
        .await
        .map_err(|err| failed("UID FETCH", err))?;
    let seq = probe
        .iter()
        .find(|f| f.uid == Some(before_uid))
        .map(|f| f.message);
    let responses: Vec<Fetch> = match seq {
        Some(seq) if seq <= 1 => return Ok((server_validity, Vec::new(), true)),
        Some(seq) => {
            let low = seq.saturating_sub(limit).max(1);
            session
                .fetch(format!("{low}:{}", seq - 1), ENVELOPE_QUERY)
                .await
                .map_err(|err| failed("FETCH", err))?
                .try_collect()
                .await
                .map_err(|err| failed("FETCH", err))?
        }
        None => {
            // Expunged meanwhile: the highest UIDs below it.
            let mut older: Vec<u32> = session
                .uid_search(format!("UID 1:{}", before_uid - 1))
                .await
                .map_err(|err| failed("UID SEARCH", err))?
                .into_iter()
                .collect();
            older.sort_unstable();
            let keep = usize::try_from(limit).unwrap_or(usize::MAX);
            let start = older.len().saturating_sub(keep);
            let set: Vec<String> = older[start..].iter().map(u32::to_string).collect();
            if set.is_empty() {
                return Ok((server_validity, Vec::new(), true));
            }
            session
                .uid_fetch(set.join(","), ENVELOPE_QUERY)
                .await
                .map_err(|err| failed("UID FETCH", err))?
                .try_collect()
                .await
                .map_err(|err| failed("UID FETCH", err))?
        }
    };
    let low_seq = responses.iter().map(|f| f.message).min();
    let mut envelopes: Vec<RemoteEnvelope> = responses
        .iter()
        .filter_map(|fetch| {
            let uid = fetch.uid.filter(|uid| *uid < before_uid)?;
            Some(envelope_of(uid, fetch))
        })
        .collect();
    envelopes.sort_by_key(|envelope| envelope.uid);
    envelopes.dedup_by_key(|envelope| envelope.uid);
    let complete = envelopes.is_empty() || low_seq == Some(1);
    Ok((server_validity, envelopes, complete))
}

/// The envelope in a FETCH response for [`ENVELOPE_QUERY`].
fn envelope_of(uid: u32, fetch: &Fetch) -> RemoteEnvelope {
    let flags: Vec<String> = fetch.flags().filter_map(|flag| flag_name(&flag)).collect();
    app_io::mime::remote_envelope(
        uid,
        flags.iter().map(String::as_str),
        fetch.size.unwrap_or(0),
        fetch.header().unwrap_or_default(),
    )
}

/// `EXAMINE` the mailbox, `UID SEARCH` it with `arguments` (see
/// [`crate::search::search_arguments`]) and fetch the envelopes of the newest `limit` hits.
/// Returns the server's UIDVALIDITY and the envelopes, ascending by UID.
pub async fn search_mailbox(
    session: &mut ImapSession,
    path: &str,
    arguments: &str,
    limit: u32,
) -> Result<(u32, Vec<RemoteEnvelope>), String> {
    let selected = session
        .examine(path)
        .await
        .map_err(|err| format!("IMAP EXAMINE {path} failed: {err}"))?;
    let uid_validity = selected
        .uid_validity
        .ok_or_else(|| format!("IMAP EXAMINE {path}: server reported no UIDVALIDITY"))?;
    let mut uids: Vec<u32> = session
        .uid_search(arguments)
        .await
        .map_err(|err| format!("IMAP UID SEARCH in {path} failed: {err}"))?
        .into_iter()
        .collect();
    uids.sort_unstable();
    let keep = usize::try_from(limit).unwrap_or(usize::MAX);
    if uids.len() > keep {
        uids.drain(..uids.len() - keep);
    }
    if uids.is_empty() {
        return Ok((uid_validity, Vec::new()));
    }
    let responses: Vec<Fetch> = session
        .uid_fetch(uid_set(&uids), ENVELOPE_QUERY)
        .await
        .map_err(|err| format!("IMAP UID FETCH in {path} failed: {err}"))?
        .try_collect()
        .await
        .map_err(|err| format!("IMAP UID FETCH in {path} failed: {err}"))?;
    let mut envelopes: Vec<RemoteEnvelope> = responses
        .iter()
        .filter_map(|fetch| {
            let uid = fetch.uid.filter(|uid| uids.binary_search(uid).is_ok())?;
            Some(envelope_of(uid, fetch))
        })
        .collect();
    envelopes.sort_by_key(|envelope| envelope.uid);
    envelopes.dedup_by_key(|envelope| envelope.uid);
    Ok((uid_validity, envelopes))
}

fn flag_name(flag: &Flag<'_>) -> Option<String> {
    match flag {
        Flag::Seen => Some("\\Seen".to_owned()),
        Flag::Answered => Some("\\Answered".to_owned()),
        Flag::Flagged => Some("\\Flagged".to_owned()),
        Flag::Draft => Some("\\Draft".to_owned()),
        Flag::Custom(name) => Some(name.to_string()),
        _ => None,
    }
}

/// What a flag resync found ([`fetch_flags`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResyncedFlags {
    /// The server's UIDVALIDITY.
    pub uid_validity: u32,
    /// `(uid, flags)`: of every existing UID, or only of the changed ones with `existing`.
    pub flags: Vec<(u32, Vec<String>)>,
    /// With CONDSTORE and a mod-sequence to start from: every UID of the range that exists.
    pub existing: Option<Vec<u32>>,
    /// The server's HIGHESTMODSEQ, if it has CONDSTORE.
    pub highest_modseq: Option<u64>,
}

/// Flags of the UIDs `from..=to` that still exist, after checking UIDVALIDITY. A CONDSTORE
/// server is opened with `SELECT … (CONDSTORE)`; given `changed_since` (its HIGHESTMODSEQ at
/// the last resync) only the flags changed since are fetched (`CHANGEDSINCE`), and
/// `UID SEARCH UID from:to` tells which UIDs still exist.
pub async fn fetch_flags(
    session: &mut ImapSession,
    path: &str,
    from: u32,
    to: u32,
    changed_since: Option<u64>,
) -> Result<ResyncedFlags, String> {
    let capabilities = session
        .capabilities()
        .await
        .map_err(|err| format!("IMAP CAPABILITY failed: {err}"))?;
    let condstore = capabilities.has_str("CONDSTORE") || capabilities.has_str("QRESYNC");
    let selected = if condstore {
        session
            .select_condstore(path)
            .await
            .map_err(|err| format!("IMAP SELECT {path} (CONDSTORE) failed: {err}"))?
    } else {
        session
            .examine(path)
            .await
            .map_err(|err| format!("IMAP EXAMINE {path} failed: {err}"))?
    };
    let uid_validity = selected
        .uid_validity
        .ok_or_else(|| format!("IMAP EXAMINE {path}: server reported no UIDVALIDITY"))?;
    let highest_modseq = selected.highest_modseq.filter(|_| condstore);
    let mut out = ResyncedFlags {
        uid_validity,
        flags: Vec::new(),
        existing: None,
        highest_modseq,
    };
    if selected.exists == 0 || from > to {
        out.existing = highest_modseq.map(|_| Vec::new());
        return Ok(out);
    }
    let since = changed_since.filter(|_| highest_modseq.is_some());
    let query = match since {
        Some(modseq) => format!("(UID FLAGS) (CHANGEDSINCE {modseq})"),
        None => "(UID FLAGS)".to_owned(),
    };
    if since.is_some() {
        let existing = session
            .uid_search(format!("UID {from}:{to}"))
            .await
            .map_err(|err| format!("IMAP UID SEARCH in {path} failed: {err}"))?;
        let mut existing: Vec<u32> = existing
            .into_iter()
            .filter(|uid| (from..=to).contains(uid))
            .collect();
        existing.sort_unstable();
        out.existing = Some(existing);
    }
    let responses: Vec<Fetch> = session
        .uid_fetch(format!("{from}:{to}"), query)
        .await
        .map_err(|err| format!("IMAP UID FETCH FLAGS in {path} failed: {err}"))?
        .try_collect()
        .await
        .map_err(|err| format!("IMAP UID FETCH FLAGS in {path} failed: {err}"))?;
    let mut flags: Vec<(u32, Vec<String>)> = responses
        .iter()
        .filter_map(|fetch| {
            let uid = fetch.uid.filter(|uid| (from..=to).contains(uid))?;
            Some((uid, fetch.flags().filter_map(|f| flag_name(&f)).collect()))
        })
        .collect();
    flags.sort_by_key(|(uid, _)| *uid);
    flags.dedup_by_key(|(uid, _)| *uid);
    out.flags = flags;
    Ok(out)
}

/// The raw RFC 822 message with `uid`, without setting `\Seen`. Fails if UIDVALIDITY changed
/// or the message is gone.
pub async fn fetch_body(
    session: &mut ImapSession,
    path: &str,
    uid_validity: u32,
    uid: u32,
) -> Result<Vec<u8>, String> {
    let selected = session
        .examine(path)
        .await
        .map_err(|err| format!("IMAP EXAMINE {path} failed: {err}"))?;
    if selected.uid_validity != Some(uid_validity) {
        return Err(format!(
            "{path}: UIDVALIDITY changed; the message must be re-synced"
        ));
    }
    let responses: Vec<Fetch> = session
        .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
        .await
        .map_err(|err| format!("IMAP UID FETCH BODY in {path} failed: {err}"))?
        .try_collect()
        .await
        .map_err(|err| format!("IMAP UID FETCH BODY in {path} failed: {err}"))?;
    responses
        .iter()
        .find(|fetch| fetch.uid == Some(uid))
        .and_then(Fetch::body)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| format!("{path}: message UID {uid} is no longer on the server"))
}

/// A UID set in IMAP syntax, with consecutive runs collapsed (`1:3,7,9:10`). `uids` must be
/// ascending.
pub fn uid_set(uids: &[u32]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < uids.len() {
        let start = uids[i];
        let mut end = start;
        while i + 1 < uids.len() && uids[i + 1] == end.saturating_add(1) {
            i += 1;
            end = uids[i];
        }
        if !out.is_empty() {
            out.push(',');
        }
        if start == end {
            out.push_str(&start.to_string());
        } else {
            out.push_str(&format!("{start}:{end}"));
        }
        i += 1;
    }
    out
}

/// Why a remote step failed.
#[derive(Debug)]
enum StepError {
    /// The connection broke (or the server could not be understood): retrying may work.
    Lost(String),
    /// The server refused (`NO`/`BAD`, changed UIDVALIDITY).
    Rejected(String),
}

impl StepError {
    fn into_message(self) -> String {
        match self {
            Self::Lost(message) | Self::Rejected(message) => message,
        }
    }

    fn imap(context: String, err: &async_imap::error::Error) -> Self {
        use async_imap::error::Error;
        let message = format!("{context} failed: {err}");
        match err {
            Error::No(_) | Error::Bad(_) | Error::Validate(_) | Error::Append => {
                Self::Rejected(message)
            }
            // I/O, a lost connection, an unparsable answer (and future variants): try again.
            _ => Self::Lost(message),
        }
    }
}

/// Runs the steps of a pending op in order (`ARCHITECTURE.md §5.2`). Each step `SELECT`s its
/// mailbox read-write and fails if UIDVALIDITY changed. Moves use `UID MOVE` if the server
/// offers MOVE, else `UID COPY` + `UID STORE +FLAGS.SILENT (\Deleted)` + [`expunge`]. Every step
/// is safe to repeat.
///
/// The outer error means the connection broke (worth retrying, also on a fresh session); the
/// inner one that the server rejected a step.
pub async fn run_steps(
    session: &mut ImapSession,
    steps: &[RemoteStep],
) -> Result<Result<(), String>, String> {
    match try_steps(session, steps).await {
        Ok(()) => Ok(Ok(())),
        Err(StepError::Rejected(error)) => Ok(Err(error)),
        Err(StepError::Lost(error)) => Err(error),
    }
}

async fn try_steps(session: &mut ImapSession, steps: &[RemoteStep]) -> Result<(), StepError> {
    let capabilities = session
        .capabilities()
        .await
        .map_err(|err| StepError::imap("IMAP CAPABILITY".to_owned(), &err))?;
    let has_move = capabilities.has_str("MOVE");
    let has_uidplus = capabilities.has_str("UIDPLUS");
    for step in steps {
        match step {
            RemoteStep::Store {
                path,
                uid_validity,
                uids,
                flag,
                value,
            } => {
                select(session, path, *uid_validity).await?;
                let sign = if *value { '+' } else { '-' };
                let query = format!("{sign}FLAGS.SILENT ({})", flag.imap());
                store(session, path, uids, &query).await?;
            }
            RemoteStep::Move {
                path,
                uid_validity,
                uids,
                to,
                create,
            } => {
                if *create {
                    create_mailbox(session, to).await;
                }
                select(session, path, *uid_validity).await?;
                let set = uid_set(uids);
                if has_move {
                    session.uid_mv(&set, to).await.map_err(|err| {
                        StepError::imap(format!("IMAP UID MOVE {path} -> {to}"), &err)
                    })?;
                } else {
                    session.uid_copy(&set, to).await.map_err(|err| {
                        StepError::imap(format!("IMAP UID COPY {path} -> {to}"), &err)
                    })?;
                    store(session, path, uids, "+FLAGS.SILENT (\\Deleted)").await?;
                    expunge(session, path, uids, has_uidplus).await?;
                }
            }
            RemoteStep::Copy {
                path,
                uid_validity,
                uids,
                to,
                create,
            } => {
                if *create {
                    create_mailbox(session, to).await;
                }
                select(session, path, *uid_validity).await?;
                session.uid_copy(uid_set(uids), to).await.map_err(|err| {
                    StepError::imap(format!("IMAP UID COPY {path} -> {to}"), &err)
                })?;
            }
        }
    }
    Ok(())
}

/// `SELECT path` (read-write) and check UIDVALIDITY.
async fn select(session: &mut ImapSession, path: &str, uid_validity: u32) -> Result<(), StepError> {
    let selected = session
        .select(path)
        .await
        .map_err(|err| StepError::imap(format!("IMAP SELECT {path}"), &err))?;
    if selected.uid_validity != Some(uid_validity) {
        return Err(StepError::Rejected(format!(
            "{path}: UIDVALIDITY changed; the messages must be re-synced"
        )));
    }
    Ok(())
}

async fn store(
    session: &mut ImapSession,
    path: &str,
    uids: &[u32],
    query: &str,
) -> Result<(), StepError> {
    let context = format!("IMAP UID STORE in {path}");
    let _: Vec<Fetch> = session
        .uid_store(uid_set(uids), query)
        .await
        .map_err(|err| StepError::imap(context.clone(), &err))?
        .try_collect()
        .await
        .map_err(|err| StepError::imap(context, &err))?;
    Ok(())
}

/// Expunges `uids` of the selected mailbox `path`, which the caller flagged `\Deleted`: `UID
/// EXPUNGE` with UIDPLUS. A plain `EXPUNGE` would remove every message flagged `\Deleted`, also
/// ones another client flagged and has not expunged yet, so those lose the flag for the moment
/// of the `EXPUNGE` and get it back afterwards. A connection lost in between leaves them
/// undeleted, never gone.
async fn expunge(
    session: &mut ImapSession,
    path: &str,
    uids: &[u32],
    has_uidplus: bool,
) -> Result<(), StepError> {
    let context = format!("IMAP EXPUNGE in {path}");
    if has_uidplus {
        let _: Vec<_> = session
            .uid_expunge(uid_set(uids))
            .await
            .map_err(|err| StepError::imap(context.clone(), &err))?
            .try_collect()
            .await
            .map_err(|err| StepError::imap(context, &err))?;
        return Ok(());
    }
    let mut others: Vec<u32> = session
        .uid_search("DELETED")
        .await
        .map_err(|err| StepError::imap(format!("IMAP UID SEARCH DELETED in {path}"), &err))?
        .into_iter()
        .filter(|uid| !uids.contains(uid))
        .collect();
    others.sort_unstable();
    if !others.is_empty() {
        store(session, path, &others, "-FLAGS.SILENT (\\Deleted)").await?;
    }
    let expunged: Result<Vec<_>, _> = match session.expunge().await {
        Ok(stream) => stream.try_collect().await,
        Err(err) => Err(err),
    };
    if !others.is_empty() {
        store(session, path, &others, "+FLAGS.SILENT (\\Deleted)").await?;
    }
    expunged.map_err(|err| StepError::imap(context, &err))?;
    Ok(())
}

/// `CREATE name`. A failure is not an error: the mailbox usually exists already, and if it does
/// not, the following MOVE/COPY reports it.
async fn create_mailbox(session: &mut ImapSession, name: &str) {
    if let Err(err) = session.create(name).await {
        tracing::debug!(mailbox = name, error = %err, "IMAP CREATE failed (probably exists)");
    }
}

/// An IMAP quoted string: `\` and `"` escaped, control characters (CR, LF, …) replaced by
/// spaces. async-imap sends search criteria as they are, so every string that goes into one
/// (queries, Message-IDs) is quoted with this: it can never end the command line and inject
/// another IMAP command.
pub fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
            out.push(c);
        } else if c.is_control() {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

/// `SELECT path` (read-write) and the UIDs of the messages with `Message-ID: <message_id>`.
async fn select_by_message_id(
    session: &mut ImapSession,
    path: &str,
    message_id: &str,
) -> Result<Vec<u32>, String> {
    session
        .select(path)
        .await
        .map_err(|err| format!("IMAP SELECT {path} failed: {err}"))?;
    let query = format!("HEADER Message-ID {}", quoted(&format!("<{message_id}>")));
    let mut uids: Vec<u32> = session
        .uid_search(query)
        .await
        .map_err(|err| format!("IMAP UID SEARCH in {path} failed: {err}"))?
        .into_iter()
        .collect();
    uids.sort_unstable();
    Ok(uids)
}

/// `UID STORE +FLAGS.SILENT (\Deleted)` and [`expunge`] `uids` of the selected mailbox `path`.
async fn delete_uids(session: &mut ImapSession, path: &str, uids: &[u32]) -> Result<(), String> {
    if uids.is_empty() {
        return Ok(());
    }
    let has_uidplus = session
        .capabilities()
        .await
        .map_err(|err| format!("IMAP CAPABILITY failed: {err}"))?
        .has_str("UIDPLUS");
    store(session, path, uids, "+FLAGS.SILENT (\\Deleted)")
        .await
        .map_err(StepError::into_message)?;
    expunge(session, path, uids, has_uidplus)
        .await
        .map_err(StepError::into_message)
}

/// Stores a draft in `path`: `APPEND` it with `\Seen \Draft`, then delete the copies saved
/// before (same `Message-ID`). The new copy is appended first, so a failure never loses the
/// draft; at worst an old copy stays.
pub async fn save_draft(
    session: &mut ImapSession,
    path: &str,
    message_id: &str,
    raw: &[u8],
) -> Result<(), String> {
    let old = select_by_message_id(session, path, message_id).await?;
    session
        .append(path, Some("(\\Seen \\Draft)"), None, raw)
        .await
        .map_err(|err| format!("IMAP APPEND to {path} failed: {err}"))?;
    delete_uids(session, path, &old).await
}

/// `APPEND`s a message to `path` with `flags` (e.g. `(\\Seen)`).
pub async fn append_message(
    session: &mut ImapSession,
    path: &str,
    flags: &str,
    raw: &[u8],
) -> Result<(), String> {
    session
        .append(path, Some(flags), None, raw)
        .await
        .map_err(|err| format!("IMAP APPEND to {path} failed: {err}"))
}

/// Deletes every copy of a draft (by `Message-ID`) from `path`.
pub async fn delete_draft(
    session: &mut ImapSession,
    path: &str,
    message_id: &str,
) -> Result<(), String> {
    let old = select_by_message_id(session, path, message_id).await?;
    delete_uids(session, path, &old).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_strings_are_escaped_and_stay_on_one_line() {
        assert_eq!(quoted(r#"say "hi" \o/"#), r#""say \"hi\" \\o/""#);
        let hostile = quoted("<a@b\r\nZ1 DELETE INBOX\r\nZ2 NOOP>");
        assert_eq!(hostile, "\"<a@b  Z1 DELETE INBOX  Z2 NOOP>\"");
        assert!(!hostile.contains(['\r', '\n']));
        assert_eq!(quoted("a\tb\0c"), "\"a b c\"");
    }
}

/// How long one IDLE waits before it ends quietly (servers drop IDLE after 29 minutes).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(25 * 60);

/// Why an IDLE could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleError {
    /// The server does not announce `IDLE`.
    Unsupported,
    /// The connection or the server failed.
    Failed(String),
}

/// `EXAMINE path`, then `IDLE` until the server reports a change (`true`) or `timeout` passes
/// quietly (`false`), then `DONE`. Returns the session for the next IDLE.
pub async fn idle_once(
    mut session: ImapSession,
    path: &str,
    timeout: Duration,
) -> Result<(ImapSession, bool), IdleError> {
    let failed = |what: &str, err: async_imap::error::Error| {
        IdleError::Failed(format!("IMAP {what} {path} failed: {err}"))
    };
    let capabilities = session
        .capabilities()
        .await
        .map_err(|err| failed("CAPABILITY for", err))?;
    if !capabilities.has_str("IDLE") {
        return Err(IdleError::Unsupported);
    }
    session
        .examine(path)
        .await
        .map_err(|err| failed("EXAMINE", err))?;
    let mut idle = session.idle();
    idle.init().await.map_err(|err| failed("IDLE in", err))?;
    let response = {
        // Dropping the stop source would end the wait at once: keep it until the wait ends.
        let (wait, _stop) = idle.wait_with_timeout(timeout);
        wait.await.map_err(|err| failed("IDLE in", err))?
    };
    let changed = matches!(
        response,
        async_imap::extensions::idle::IdleResponse::NewData(_)
    );
    let session = idle.done().await.map_err(|err| failed("DONE in", err))?;
    Ok((session, changed))
}
