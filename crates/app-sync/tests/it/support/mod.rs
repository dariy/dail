//! A scripted in-memory IMAP server for `LiveBackend` tests. It speaks just enough IMAP4rev1
//! (greeting, LOGIN, CAPABILITY, LIST, CREATE, EXAMINE / SELECT, UID SEARCH (a few keys) / HEADER,
//! FETCH / UID FETCH, UID STORE / MOVE / COPY / EXPUNGE, EXPUNGE, APPEND, NOOP, IDLE, LOGOUT)
//! over `tokio::io::duplex`; no network is involved.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use app_core::{Endpoint, Secret};
use app_sync::imap::greet_and_login;
use app_sync::{BoxFuture, ImapConnector, ImapSession};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream};

/// One folder on the fake server.
#[derive(Debug, Clone)]
pub struct FakeFolder {
    /// Full path.
    pub path: String,
    /// LIST attributes, e.g. `\HasNoChildren \Sent`.
    pub attributes: String,
    pub uid_validity: u32,
    /// Messages in UID order.
    pub messages: Vec<FakeMail>,
}

/// One message in a [`FakeFolder`].
#[derive(Debug, Clone)]
pub struct FakeMail {
    pub uid: u32,
    /// Flags as written in a FETCH response, e.g. `\Seen \Flagged`.
    pub flags: String,
    pub raw: String,
    /// CONDSTORE mod-sequence of the last change (1 for messages as created).
    pub modseq: u64,
}

impl FakeFolder {
    pub fn new(path: &str, attributes: &str) -> Self {
        Self {
            path: path.to_owned(),
            attributes: attributes.to_owned(),
            uid_validity: 1,
            messages: Vec::new(),
        }
    }

    /// Adds a message with `Message-ID: <id>`, the given subject and flags.
    pub fn with(mut self, uid: u32, flags: &str, id: &str, subject: &str) -> Self {
        self.messages.push(FakeMail {
            uid,
            flags: flags.to_owned(),
            raw: format!(
                "Message-ID: <{id}>\r\nFrom: Bob <bob@example.org>\r\nSubject: {subject}\r\n\
                 Date: Tue, 14 Nov 2023 09:41:00 +0000\r\n\r\nBody of {id}\r\n"
            ),
            modseq: 1,
        });
        self
    }
}

/// Server content and counters, shared by all connections.
#[derive(Debug, Default)]
pub struct FakeServer {
    pub password: String,
    pub folders: Vec<FakeFolder>,
    /// Leave MOVE and UIDPLUS out of CAPABILITY (an old server).
    pub no_move: bool,
    /// Leave IDLE out of CAPABILITY.
    pub no_idle: bool,
    /// Announce CONDSTORE: `SELECT … (CONDSTORE)` reports HIGHESTMODSEQ, `UID FETCH … (CHANGEDSINCE n)`
    /// lists only messages changed since.
    pub condstore: bool,
    /// Commands received, without tags (passwords replaced by `***`).
    pub log: Vec<String>,
}

/// An `ImapConnector` whose connections talk to a [`FakeServer`].
#[derive(Debug, Clone, Default)]
pub struct FakeImap {
    pub server: Arc<Mutex<FakeServer>>,
    connects: Arc<AtomicUsize>,
    /// Bumping the epoch makes every open connection drop on its next command.
    epoch: Arc<AtomicUsize>,
    /// Wakes a connection in IDLE ([`FakeImap::deliver`]).
    changes: Arc<tokio::sync::Notify>,
}

impl FakeImap {
    pub fn new(password: &str, folders: Vec<FakeFolder>) -> Self {
        let imap = Self::default();
        {
            let mut server = imap.server.lock().unwrap();
            server.password = password.to_owned();
            server.folders = folders;
        }
        imap
    }

    /// Number of connections opened so far.
    pub fn connects(&self) -> usize {
        self.connects.load(Ordering::SeqCst)
    }

    /// Drops every open connection (as an idle timeout on the server would).
    pub fn drop_connections(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    pub fn log(&self) -> Vec<String> {
        self.server.lock().unwrap().log.clone()
    }

    /// New mail in `path`: appended with the next UID, and announced to a connection in IDLE.
    pub fn deliver(&self, path: &str, raw: &str) {
        {
            let mut server = self.server.lock().unwrap();
            let folder = server.folders.iter_mut().find(|f| f.path == path).unwrap();
            let uid = folder.messages.last().map_or(1, |m| m.uid + 1);
            folder.messages.push(FakeMail {
                uid,
                flags: String::new(),
                raw: raw.to_owned(),
                modseq: 1,
            });
        }
        // A permit, so that a connection about to IDLE does not miss it.
        self.changes.notify_one();
    }
}

impl ImapConnector for FakeImap {
    fn connect<'a>(
        &'a self,
        _server: &'a Endpoint,
        login: &'a str,
        password: &'a Secret,
    ) -> BoxFuture<'a, Result<ImapSession, String>> {
        Box::pin(async move {
            self.connects.fetch_add(1, Ordering::SeqCst);
            let (client, server) = tokio::io::duplex(1 << 16);
            tokio::spawn(serve(
                server,
                Arc::clone(&self.server),
                Arc::clone(&self.epoch),
                Arc::clone(&self.changes),
            ));
            greet_and_login(Box::new(client), login, password, "fake").await
        })
    }
}

/// Splits `"a" "b c"` style IMAP arguments (quoted strings and atoms).
fn arguments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c == ' ' {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut value = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => value.extend(chars.next()),
                    '"' => break,
                    c => value.push(c),
                }
            }
            out.push(value);
        } else {
            let mut value = String::new();
            while let Some(&c) = chars.peek() {
                if c == ' ' {
                    break;
                }
                value.push(c);
                chars.next();
            }
            out.push(value);
        }
    }
    out
}

async fn serve(
    stream: DuplexStream,
    server: Arc<Mutex<FakeServer>>,
    epoch: Arc<AtomicUsize>,
    changes: Arc<tokio::sync::Notify>,
) {
    let born = epoch.load(Ordering::SeqCst);
    let (read, mut write) = tokio::io::split(stream);
    let mut lines = BufReader::new(read).lines();
    let mut selected: Option<String> = None;
    if write.write_all(b"* OK fake IMAP ready\r\n").await.is_err() {
        return;
    }
    while let Ok(Some(line)) = lines.next_line().await {
        if epoch.load(Ordering::SeqCst) != born {
            return;
        }
        let (tag, rest) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let (verb, args) = rest.split_once(' ').unwrap_or((rest, ""));
        let verb = verb.to_ascii_uppercase();
        if verb == "IDLE" && !server.lock().unwrap().no_idle {
            server.lock().unwrap().log.push("IDLE".to_owned());
            if write.write_all(b"+ idling\r\n").await.is_err() {
                return;
            }
            loop {
                tokio::select! {
                    line = lines.next_line() => match line {
                        Ok(Some(line)) if line.trim().eq_ignore_ascii_case("DONE") => {
                            let done = format!("{tag} OK IDLE terminated\r\n");
                            if write.write_all(done.as_bytes()).await.is_err() {
                                return;
                            }
                            break;
                        }
                        Ok(Some(_)) => {}
                        _ => return,
                    },
                    () = changes.notified() => {
                        let exists = {
                            let server = server.lock().unwrap();
                            selected
                                .as_ref()
                                .and_then(|path| server.folders.iter().find(|f| &f.path == path))
                                .map_or(0, |f| f.messages.len())
                        };
                        let note = format!("* {exists} EXISTS\r\n");
                        if write.write_all(note.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                }
            }
            continue;
        }
        if verb == "APPEND" {
            // `APPEND "path" (flags) {n}`: the message follows as a literal.
            let size: usize = args
                .rsplit_once('{')
                .and_then(|(_, n)| n.trim_end_matches('}').parse().ok())
                .unwrap_or(0);
            if write
                .write_all(b"+ Ready for literal data\r\n")
                .await
                .is_err()
            {
                return;
            }
            let mut literal = vec![0_u8; size];
            if lines.get_mut().read_exact(&mut literal).await.is_err() {
                return;
            }
            // The CRLF that ends the command line.
            let _ = lines.next_line().await;
            let reply = {
                let mut server = server.lock().unwrap();
                server.log.push(format!("APPEND {args}"));
                append(&mut server, tag, args, &literal)
            };
            if write.write_all(reply.as_bytes()).await.is_err() {
                return;
            }
            continue;
        }
        let reply = {
            let mut server = server.lock().unwrap();
            server.log.push(if verb == "LOGIN" {
                "LOGIN ***".to_owned()
            } else {
                rest.to_owned()
            });
            respond(&mut server, &mut selected, tag, &verb, args)
        };
        if write.write_all(reply.as_bytes()).await.is_err() || verb == "LOGOUT" {
            return;
        }
    }
}

/// `APPEND "path" (flags) {n}` with the literal `raw`.
fn append(server: &mut FakeServer, tag: &str, args: &str, raw: &[u8]) -> String {
    let path = arguments(args).into_iter().next().unwrap_or_default();
    let flags = args
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(flags, _)| flags.to_owned())
        .unwrap_or_default();
    let Some(folder) = server.folders.iter_mut().find(|f| f.path == path) else {
        return format!("{tag} NO [TRYCREATE] no such mailbox\r\n");
    };
    let uid = folder.messages.last().map_or(1, |m| m.uid + 1);
    folder.messages.push(FakeMail {
        uid,
        flags,
        raw: String::from_utf8_lossy(raw).into_owned(),
        modseq: 1,
    });
    format!(
        "{tag} OK [APPENDUID {} {uid}] APPEND completed\r\n",
        folder.uid_validity
    )
}

/// Whether the header block of `raw` has `name: value` (name case-insensitive).
fn has_header(raw: &str, name: &str, value: &str) -> bool {
    let header_end = raw.find("\r\n\r\n").unwrap_or(raw.len());
    raw[..header_end].split("\r\n").any(|line| {
        line.split_once(':')
            .is_some_and(|(n, v)| n.trim().eq_ignore_ascii_case(name) && v.trim() == value)
    })
}

/// A tiny `SEARCH` evaluator: `ALL`, `SEEN`, `UNSEEN`, `FLAGGED`, `DELETED`, `TEXT` / `X-GM-RAW` (a
/// case-insensitive substring of the raw message), `FROM` / `SUBJECT` (of that header) and
/// `NOT`, all ANDed; `CHARSET x` is skipped.
fn search_matches(mail: &FakeMail, keys: &[String]) -> bool {
    let raw = mail.raw.to_lowercase();
    let header = |name: &str, value: &str| {
        raw.split("\r\n").take_while(|l| !l.is_empty()).any(|line| {
            line.split_once(':')
                .is_some_and(|(n, v)| n.trim() == name && v.contains(&value.to_lowercase()))
        })
    };
    let mut keys = keys.iter();
    let mut negate = false;
    while let Some(key) = keys.next() {
        let key = key.to_ascii_uppercase();
        let hit = match key.as_str() {
            "NOT" => {
                negate = !negate;
                continue;
            }
            "CHARSET" => {
                keys.next();
                continue;
            }
            "ALL" => true,
            "UID" => in_set(mail.uid, &uid_ranges(keys.next().unwrap())),
            "SEEN" => mail.flags.contains("\\Seen"),
            "UNSEEN" => !mail.flags.contains("\\Seen"),
            "FLAGGED" => mail.flags.contains("\\Flagged"),
            "DELETED" => mail.flags.contains("\\Deleted"),
            "TEXT" | "X-GM-RAW" => raw.contains(&keys.next().unwrap().to_lowercase()),
            "FROM" => header("from", keys.next().unwrap()),
            "SUBJECT" => header("subject", keys.next().unwrap()),
            other => panic!("fake SEARCH does not know {other}"),
        };
        if hit == negate {
            return false;
        }
        negate = false;
    }
    true
}

/// Parses `a:b` / `a:*` / `a` into an inclusive range (`*` = `u32::MAX`).
fn range(set: &str) -> (u32, u32) {
    let (a, b) = set.split_once(':').unwrap_or((set, set));
    let parse = |v: &str| {
        if v == "*" {
            u32::MAX
        } else {
            v.parse().unwrap()
        }
    };
    let (a, b) = (parse(a), parse(b));
    (a.min(b), a.max(b))
}

/// One FETCH response line: flags only, the header fields, or the full message (`BODY[]`).
fn fetch_line(seq: usize, mail: &FakeMail, query: &str) -> String {
    let query = query.to_ascii_uppercase();
    if query.contains("BODY.PEEK[]") {
        return format!(
            "* {seq} FETCH (UID {} BODY[] {{{}}}\r\n{})\r\n",
            mail.uid,
            mail.raw.len(),
            mail.raw
        );
    }
    if !query.contains("HEADER") {
        return format!(
            "* {seq} FETCH (UID {} FLAGS ({}))\r\n",
            mail.uid, mail.flags
        );
    }
    let header_end = mail.raw.find("\r\n\r\n").map_or(mail.raw.len(), |i| i + 4);
    let header = &mail.raw[..header_end];
    format!(
        "* {seq} FETCH (UID {} FLAGS ({}) RFC822.SIZE {} BODY[HEADER.FIELDS (MESSAGE-ID)] {{{}}}\r\n{header})\r\n",
        mail.uid,
        mail.flags,
        mail.raw.len(),
        header.len()
    )
}

/// Parses a UID set like `1:3,7` into its ranges.
fn uid_ranges(set: &str) -> Vec<(u32, u32)> {
    set.split(',').map(range).collect()
}

fn in_set(uid: u32, set: &[(u32, u32)]) -> bool {
    set.iter().any(|(lo, hi)| (*lo..=*hi).contains(&uid))
}

/// Applies `+FLAGS.SILENT (\X)` / `-FLAGS.SILENT (\X)` to a flag string.
fn apply_flags(flags: &mut String, query: &str) {
    let add = query.starts_with('+');
    let names: Vec<&str> = query
        .split_once('(')
        .map(|(_, rest)| rest.trim_end_matches(')').split_whitespace().collect())
        .unwrap_or_default();
    let mut current: Vec<String> = flags.split_whitespace().map(str::to_owned).collect();
    for name in names {
        current.retain(|f| !f.eq_ignore_ascii_case(name));
        if add {
            current.push(name.to_owned());
        }
    }
    *flags = current.join(" ");
}

/// Mutating UID commands on the selected folder: STORE, MOVE, COPY, EXPUNGE.
fn uid_change(server: &mut FakeServer, path: &str, tag: &str, sub: &str, rest: &str) -> String {
    let (set, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    let set = uid_ranges(set);
    let Some(index) = server.folders.iter().position(|f| f.path == path) else {
        return format!("{tag} NO no mailbox selected\r\n");
    };
    match sub {
        "STORE" => {
            let next = server.folders[index]
                .messages
                .iter()
                .map(|m| m.modseq)
                .max()
                .unwrap_or(1)
                + 1;
            for mail in server.folders[index]
                .messages
                .iter_mut()
                .filter(|m| in_set(m.uid, &set))
            {
                apply_flags(&mut mail.flags, tail);
                mail.modseq = next;
            }
            format!("{tag} OK STORE completed\r\n")
        }
        "MOVE" | "COPY" => {
            if sub == "MOVE" && server.no_move {
                return format!("{tag} BAD unknown command\r\n");
            }
            let to = arguments(tail).into_iter().next().unwrap_or_default();
            let Some(target) = server.folders.iter().position(|f| f.path == to) else {
                return format!("{tag} NO [TRYCREATE] no such mailbox\r\n");
            };
            let picked: Vec<FakeMail> = server.folders[index]
                .messages
                .iter()
                .filter(|m| in_set(m.uid, &set))
                .cloned()
                .collect();
            if sub == "MOVE" {
                server.folders[index]
                    .messages
                    .retain(|m| !in_set(m.uid, &set));
            }
            for mut mail in picked {
                mail.uid = server.folders[target]
                    .messages
                    .last()
                    .map_or(1, |m| m.uid + 1);
                server.folders[target].messages.push(mail);
            }
            format!("{tag} OK {sub} completed\r\n")
        }
        "EXPUNGE" => {
            server.folders[index]
                .messages
                .retain(|m| !(in_set(m.uid, &set) && m.flags.contains("\\Deleted")));
            format!("{tag} OK EXPUNGE completed\r\n")
        }
        _ => format!("{tag} BAD unsupported UID command\r\n"),
    }
}

fn respond(
    server: &mut FakeServer,
    selected: &mut Option<String>,
    tag: &str,
    verb: &str,
    args: &str,
) -> String {
    if verb == "UID" {
        let (sub, rest) = args.split_once(' ').unwrap_or((args, ""));
        let sub = sub.to_ascii_uppercase();
        if matches!(sub.as_str(), "STORE" | "MOVE" | "COPY" | "EXPUNGE") {
            let Some(path) = selected.clone() else {
                return format!("{tag} BAD no mailbox selected\r\n");
            };
            return uid_change(server, &path, tag, &sub, rest);
        }
    }
    match verb {
        "CAPABILITY" => {
            let extra = if server.no_move { "" } else { " MOVE UIDPLUS" };
            let idle = if server.no_idle { "" } else { " IDLE" };
            let condstore = if server.condstore { " CONDSTORE" } else { "" };
            return format!(
                "* CAPABILITY IMAP4rev1{extra}{idle}{condstore}\r\n{tag} OK CAPABILITY completed\r\n"
            );
        }
        "CREATE" => {
            let path = arguments(args).into_iter().next().unwrap_or_default();
            if server.folders.iter().any(|f| f.path == path) {
                return format!("{tag} NO [ALREADYEXISTS] mailbox exists\r\n");
            }
            server.folders.push(FakeFolder::new(&path, ""));
            return format!("{tag} OK CREATE completed\r\n");
        }
        "EXPUNGE" => {
            if let Some(folder) = selected
                .as_ref()
                .and_then(|path| server.folders.iter_mut().find(|f| &f.path == path))
            {
                folder.messages.retain(|m| !m.flags.contains("\\Deleted"));
            }
            return format!("{tag} OK EXPUNGE completed\r\n");
        }
        _ => {}
    }
    let folder = selected
        .as_ref()
        .and_then(|path| server.folders.iter().find(|f| &f.path == path));
    match verb {
        "EXAMINE" | "SELECT" => {
            let path = arguments(args).into_iter().next().unwrap_or_default();
            let Some(folder) = server.folders.iter().find(|f| f.path == path) else {
                return format!("{tag} NO [NONEXISTENT] no such mailbox\r\n");
            };
            *selected = Some(path);
            let next = folder.messages.last().map_or(1, |m| m.uid + 1);
            let modseq = if server.condstore {
                let highest = folder.messages.iter().map(|m| m.modseq).max().unwrap_or(1);
                format!("* OK [HIGHESTMODSEQ {highest}] highest\r\n")
            } else {
                String::new()
            };
            format!(
                "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n* {} EXISTS\r\n\
                 * 0 RECENT\r\n* OK [UIDVALIDITY {}] UIDs valid\r\n* OK [UIDNEXT {next}] next\r\n\
                 {modseq}{tag} OK [READ-ONLY] EXAMINE completed\r\n",
                folder.messages.len(),
                folder.uid_validity
            )
        }
        "UID" => {
            let Some(folder) = folder else {
                return format!("{tag} BAD no mailbox selected\r\n");
            };
            let (sub, rest) = args.split_once(' ').unwrap_or((args, ""));
            match sub.to_ascii_uppercase().as_str() {
                "SEARCH" if rest.to_ascii_uppercase().starts_with("HEADER") => {
                    let query = arguments(rest);
                    let (name, value) = (
                        query.get(1).cloned().unwrap_or_default(),
                        query.get(2).cloned().unwrap_or_default(),
                    );
                    let hits: Vec<String> = folder
                        .messages
                        .iter()
                        .filter(|m| has_header(&m.raw, &name, &value))
                        .map(|m| m.uid.to_string())
                        .collect();
                    format!(
                        "* SEARCH {}\r\n{tag} OK SEARCH completed\r\n",
                        hits.join(" ")
                    )
                }
                "SEARCH" => {
                    let keys = arguments(rest);
                    let hits: Vec<String> = folder
                        .messages
                        .iter()
                        .filter(|m| search_matches(m, &keys))
                        .map(|m| m.uid.to_string())
                        .collect();
                    format!(
                        "* SEARCH {}\r\n{tag} OK SEARCH completed\r\n",
                        hits.join(" ")
                    )
                }
                "FETCH" => {
                    let set = uid_ranges(rest.split(' ').next().unwrap_or_default());
                    let since: Option<u64> = rest
                        .to_ascii_uppercase()
                        .split_once("CHANGEDSINCE ")
                        .and_then(|(_, n)| n.trim_end_matches(')').trim().parse().ok());
                    let hi = set.iter().map(|(_, hi)| *hi).max().unwrap_or(0);
                    let mut hits: Vec<(usize, &FakeMail)> = folder
                        .messages
                        .iter()
                        .enumerate()
                        .filter(|(_, m)| in_set(m.uid, &set))
                        .filter(|(_, m)| since.is_none_or(|since| m.modseq > since))
                        .map(|(i, m)| (i + 1, m))
                        .collect();
                    if hits.is_empty()
                        && hi == u32::MAX
                        && !folder.messages.is_empty()
                        && since.is_none()
                    {
                        // `n:*` always matches the last message.
                        let last = folder.messages.len();
                        hits.push((last, &folder.messages[last - 1]));
                    }
                    let mut out: String = hits
                        .iter()
                        .map(|(seq, m)| fetch_line(*seq, m, rest))
                        .collect();
                    out.push_str(&format!("{tag} OK FETCH completed\r\n"));
                    out
                }
                _ => format!("{tag} BAD unsupported UID command\r\n"),
            }
        }
        "FETCH" => {
            let Some(folder) = folder else {
                return format!("{tag} BAD no mailbox selected\r\n");
            };
            let set = args.split(' ').next().unwrap_or_default();
            let (lo, hi) = range(set);
            let len = folder.messages.len();
            let lo = usize::try_from(lo).unwrap().min(len).max(1);
            let hi = usize::try_from(hi.min(u32::try_from(len).unwrap())).unwrap();
            let mut out: String = (lo..=hi)
                .map(|seq| fetch_line(seq, &folder.messages[seq - 1], args))
                .collect();
            out.push_str(&format!("{tag} OK FETCH completed\r\n"));
            out
        }
        "LOGIN" => {
            let args = arguments(args);
            if args.get(1).map(String::as_str) == Some(server.password.as_str()) {
                format!("{tag} OK LOGIN completed\r\n")
            } else {
                format!("{tag} NO [AUTHENTICATIONFAILED] Invalid credentials\r\n")
            }
        }
        "LIST" => {
            let mut out = String::new();
            for folder in &server.folders {
                out.push_str(&format!(
                    "* LIST ({}) \"/\" \"{}\"\r\n",
                    folder.attributes, folder.path
                ));
            }
            out.push_str(&format!("{tag} OK LIST completed\r\n"));
            out
        }
        "NOOP" => format!("{tag} OK NOOP completed\r\n"),
        "LOGOUT" => format!("* BYE logging out\r\n{tag} OK LOGOUT completed\r\n"),
        _ => format!("{tag} BAD unsupported command\r\n"),
    }
}
