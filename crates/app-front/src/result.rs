//! A query's result as a document people read (MISC-22): sections of rows and labelled fields,
//! built from the command's name, the receipt's data and the state. The GUI's result window, the
//! TUI's result view and the mobile result sheet draw the same document; the raw JSON stays one
//! tap away (*Raw data*).
//!
//! Email content in a result is untrusted: every string here goes through [`clean`], nothing is
//! interpreted, and a row's tap is one of dail's own `View` commands.

use app_core::{
    AccountId, AccountStatus, AppState, Command, CommandKind, CommandName, MailboxId, ThreadId,
};
use serde_json::{Map, Value};

use crate::format;
use crate::mailbox::mailbox_name;
use crate::text::clean;

/// A query's result as a document.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultDoc {
    /// The command's title.
    pub title: String,
    /// What it shows, top to bottom.
    pub sections: Vec<Section>,
}

/// A group of rows and fields under an optional caption.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Section {
    /// The caption.
    pub title: Option<String>,
    /// Labelled values.
    pub fields: Vec<Field>,
    /// Rows, drawn as the list draws threads.
    pub rows: Vec<ResultRow>,
}

/// A labelled value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The label.
    pub label: String,
    /// The value, cleaned.
    pub value: String,
}

/// One row: a thread, a mailbox, a contact, a pending change, a check.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultRow {
    /// The first line (a sender, a mailbox's name, a check's name).
    pub title: String,
    /// The second line (a subject, an address, an error).
    pub detail: Option<String>,
    /// The trailing text (a date, counts).
    pub meta: Option<String>,
    /// The marks before the meta.
    pub marks: Vec<Mark>,
    /// What a tap runs, if anything.
    pub command: Option<Command>,
}

/// A mark on a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Has unread messages.
    Unread,
    /// Starred.
    Starred,
    /// Pinned.
    Pinned,
    /// Has attachments.
    Attachment,
    /// A check that passed.
    Passed,
    /// A check that failed.
    Failed,
    /// A check still running or never run.
    Pending,
}

impl Mark {
    /// A one-character sign, for text front ends.
    pub const fn sign(self) -> &'static str {
        match self {
            Self::Unread => "●",
            Self::Starred => "★",
            Self::Pinned => "📌",
            Self::Attachment => "📎",
            Self::Passed => "✓",
            Self::Failed => "✗",
            Self::Pending => "…",
        }
    }
}

/// The view each query gets. A new query has no view until it is added here; the test
/// `every_query_has_a_view` fails until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Message,
    Thread,
    Threads,
    Mailboxes,
    Contacts,
    PendingOps,
    Approvals,
    Fields,
    TestAccount,
    Discovery,
}

fn view_of(name: CommandName) -> Option<View> {
    if name.kind() != CommandKind::Query {
        return None;
    }
    Some(match name {
        CommandName::GetMessage => View::Message,
        CommandName::GetThread => View::Thread,
        CommandName::ListThreads | CommandName::Search => View::Threads,
        CommandName::ListMailboxes => View::Mailboxes,
        CommandName::ListContacts => View::Contacts,
        CommandName::ListPendingOps => View::PendingOps,
        CommandName::ListApprovals => View::Approvals,
        CommandName::GetSettings | CommandName::GetView => View::Fields,
        CommandName::TestAccount => View::TestAccount,
        CommandName::DiscoverServers => View::Discovery,
        // reason: the kind check above leaves only queries; a new one lands here and the test
        // `every_query_has_a_view` names it.
        _ => return None,
    })
}

/// Whether `name` has a view of its own (every query should).
pub fn has_view(name: CommandName) -> bool {
    view_of(name).is_some()
}

/// The document of a query's result; `None` for a command that is not a query. Data a view does
/// not know falls back to labelled fields.
pub fn doc(name: CommandName, data: &Value, state: &AppState, now_ms: i64) -> Option<ResultDoc> {
    if name.kind() != CommandKind::Query {
        return None;
    }
    let cx = Cx { state, now_ms };
    let sections = match view_of(name) {
        Some(View::Message) => cx.message(data.get("message").unwrap_or(data)),
        Some(View::Thread) => cx.thread(data),
        Some(View::Threads) => cx.threads(data),
        Some(View::Mailboxes) => cx.mailboxes(data),
        Some(View::Contacts) => cx.contacts(data),
        Some(View::PendingOps) => cx.pending_ops(data),
        Some(View::Approvals) => cx.approvals(data),
        Some(View::TestAccount) => cx.test_account(data),
        Some(View::Discovery) => cx.discovery(data),
        Some(View::Fields) | None => grouped(data),
    };
    Some(ResultDoc {
        title: name.spec().title.to_owned(),
        sections,
    })
}

struct Cx<'a> {
    state: &'a AppState,
    now_ms: i64,
}

impl Cx<'_> {
    fn message(&self, m: &Value) -> Vec<Section> {
        let u = &m["untrusted_content"];
        let mut fields = Vec::new();
        push(&mut fields, "From", addrs(&u["from"]));
        push(&mut fields, "To", addrs(&u["to"]));
        push(&mut fields, "Cc", addrs(&u["cc"]));
        push(
            &mut fields,
            "Date",
            m["date_ms"].as_i64().map(format::long_date),
        );
        push(&mut fields, "Subject", text(&u["subject"]));
        push(&mut fields, "Mailbox", self.mailbox(&m["mailbox"]));
        push(&mut fields, "Size", m["size"].as_u64().map(format::size));
        push(&mut fields, "Flags", flags(&m["flags"]));
        push(&mut fields, "Message-ID", text(&u["message_id"]));
        push(
            &mut fields,
            "Attachments",
            m["has_attachments"]
                .as_bool()
                .map(|a| if a { "yes" } else { "none" }.to_owned()),
        );
        let open = id::<ThreadId>(&m["thread"]).map(|id| ResultRow {
            title: "Open conversation".to_owned(),
            detail: None,
            meta: None,
            marks: Vec::new(),
            command: Some(Command::OpenThread { id }),
        });
        vec![Section {
            title: None,
            fields,
            rows: open.into_iter().collect(),
        }]
    }

    fn thread(&self, t: &Value) -> Vec<Section> {
        let id = id::<ThreadId>(&t["id"]);
        let mut fields = Vec::new();
        push(
            &mut fields,
            "Subject",
            text(&t["untrusted_content"]["subject"]),
        );
        push(
            &mut fields,
            "Last message",
            t["last_date_ms"].as_i64().map(format::long_date),
        );
        let rows = items(&t["messages"])
            .map(|m| {
                let u = &m["untrusted_content"];
                let f = m["flags"].as_object();
                let flag = |k: &str| f.and_then(|f| f.get(k)).and_then(Value::as_bool);
                let mut marks = Vec::new();
                if flag("seen") == Some(false) {
                    marks.push(Mark::Unread);
                }
                if flag("flagged") == Some(true) {
                    marks.push(Mark::Starred);
                }
                if m["has_attachments"].as_bool() == Some(true) {
                    marks.push(Mark::Attachment);
                }
                ResultRow {
                    title: addrs(&u["from"]).unwrap_or_default(),
                    detail: text(&u["snippet"]),
                    meta: self.date(&m["date_ms"]),
                    marks,
                    command: id.map(|id| Command::OpenThread { id }),
                }
            })
            .collect();
        vec![Section {
            title: None,
            fields,
            rows,
        }]
    }

    fn threads(&self, data: &Value) -> Vec<Section> {
        let threads = if data["threads"].is_array() {
            &data["threads"]
        } else {
            &data["results"]
        };
        let rows: Vec<ResultRow> = items(threads).map(|t| self.thread_row(t)).collect();
        let mut fields = Vec::new();
        push(&mut fields, "Mailbox", self.mailbox(&data["mailbox"]));
        push(&mut fields, "Query", text(&data["query"]));
        push(
            &mut fields,
            "Threads",
            data["total"]
                .as_u64()
                .map(|n| format::thousands(usize::try_from(n).unwrap_or(usize::MAX))),
        );
        if rows.is_empty() && !threads.is_array() {
            return grouped(data);
        }
        vec![Section {
            title: None,
            fields,
            rows,
        }]
    }

    /// A thread drawn as the list draws it.
    fn thread_row(&self, t: &Value) -> ResultRow {
        let u = &t["untrusted_content"];
        let mut marks = Vec::new();
        if t["unread"].as_u64().is_some_and(|n| n > 0) || t["unread"].as_bool() == Some(true) {
            marks.push(Mark::Unread);
        }
        if t["flagged"].as_bool() == Some(true) {
            marks.push(Mark::Starred);
        }
        if t["pinned"].as_bool() == Some(true) {
            marks.push(Mark::Pinned);
        }
        if t["has_attachments"].as_bool() == Some(true) {
            marks.push(Mark::Attachment);
        }
        let senders = match &u["from"] {
            Value::Array(list) => list
                .iter()
                .filter_map(|s| s.as_str().map(clean).or_else(|| addr(s)))
                .collect::<Vec<_>>()
                .join(", "),
            other => text(other).unwrap_or_default(),
        };
        let messages = t["messages"].as_u64().filter(|n| *n > 1);
        ResultRow {
            title: match messages {
                Some(n) => format!("{senders}  {n}"),
                None => senders,
            },
            detail: text(&u["subject"]),
            meta: self.date(&t["last_date_ms"]),
            marks,
            command: id::<ThreadId>(&t["id"]).map(|id| Command::OpenThread { id }),
        }
    }

    fn mailboxes(&self, data: &Value) -> Vec<Section> {
        let mut sections: Vec<(Option<AccountId>, Section)> = Vec::new();
        for m in items(&data["mailboxes"]) {
            let account = id::<AccountId>(&m["account"]);
            let mailbox = id::<MailboxId>(&m["id"]);
            let depth = m["depth"].as_u64().unwrap_or(0);
            let name = mailbox
                .and_then(|id| self.state.mailboxes.get(&id))
                .map(mailbox_name)
                .or_else(|| text(&m["path"]))
                .unwrap_or_default();
            let indent = "  ".repeat(usize::try_from(depth).unwrap_or(0));
            let unread = m["unread"].as_u64().unwrap_or(0);
            let total = m["total"].as_u64().unwrap_or(0);
            let mut meta = format!("{unread} unread · {total}");
            if let Some(window) = m["window"].as_u64() {
                meta.push_str(&format!(" · window {window}"));
            }
            let row = ResultRow {
                title: format!("{indent}{name}"),
                detail: None,
                meta: Some(meta),
                marks: if unread > 0 {
                    vec![Mark::Unread]
                } else {
                    Vec::new()
                },
                command: mailbox.map(|id| Command::OpenMailbox { id }),
            };
            match sections.iter_mut().find(|(a, _)| *a == account) {
                Some((_, section)) => section.rows.push(row),
                None => sections.push((
                    account,
                    Section {
                        title: account
                            .and_then(|a| self.state.accounts.get(&a))
                            .map(|a| clean(&a.email)),
                        fields: Vec::new(),
                        rows: vec![row],
                    },
                )),
            }
        }
        if sections.is_empty() {
            return vec![empty("No mailboxes")];
        }
        sections.into_iter().map(|(_, s)| s).collect()
    }

    fn contacts(&self, data: &Value) -> Vec<Section> {
        let rows: Vec<ResultRow> = items(&data["contacts"])
            .map(|c| {
                let u = &c["untrusted_content"];
                let email = text(&u["email"]).unwrap_or_default();
                let count = c["count"].as_u64().unwrap_or(0);
                let mut meta = format!("{count}×");
                if let Some(last) = self.date(&c["last_ms"]) {
                    meta.push_str(&format!(" · {last}"));
                }
                ResultRow {
                    title: text(&u["name"]).unwrap_or_else(|| email.clone()),
                    detail: Some(email),
                    meta: Some(meta),
                    marks: Vec::new(),
                    command: None,
                }
            })
            .collect();
        if rows.is_empty() {
            return vec![empty("No contacts")];
        }
        vec![Section {
            rows,
            ..Section::default()
        }]
    }

    fn pending_ops(&self, data: &Value) -> Vec<Section> {
        let rows: Vec<ResultRow> = items(&data["ops"])
            .map(|op| {
                let what = text(&op["action"]).unwrap_or_default().replace('_', " ");
                let detail = &op["detail"];
                let place = self
                    .mailbox(&detail["to"])
                    .or_else(|| self.mailbox(&detail["mailbox"]));
                let flag = text(&detail["flag"]);
                let title = [Some(what), flag, place.map(|p| format!("→ {p}"))]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ");
                let attempts = op["attempts"].as_u64().unwrap_or(0);
                let state = text(&op["state"]).unwrap_or_default();
                ResultRow {
                    title,
                    detail: text(&op["last_error"]),
                    meta: Some(format!("{state} · {attempts} attempts")),
                    marks: if op["last_error"].is_null() {
                        Vec::new()
                    } else {
                        vec![Mark::Failed]
                    },
                    command: None,
                }
            })
            .collect();
        if rows.is_empty() {
            return vec![empty("No pending changes")];
        }
        vec![Section {
            rows,
            ..Section::default()
        }]
    }

    fn approvals(&self, data: &Value) -> Vec<Section> {
        let rows: Vec<ResultRow> = items(&data["approvals"])
            .map(|a| {
                let u = &a["message"]["untrusted_content"];
                ResultRow {
                    title: text(&u["subject"]).unwrap_or_else(|| "(no subject)".to_owned()),
                    detail: addrs(&u["to"]).map(|to| format!("To {to}")),
                    meta: self.date(&a["requested_ms"]),
                    marks: Vec::new(),
                    command: None,
                }
            })
            .collect();
        if rows.is_empty() {
            return vec![empty("No send requests")];
        }
        vec![Section {
            rows,
            ..Section::default()
        }]
    }

    fn test_account(&self, data: &Value) -> Vec<Section> {
        let Some(account) =
            id::<AccountId>(&data["account"]).and_then(|id| self.state.accounts.get(&id))
        else {
            return grouped(data);
        };
        let (mark, detail) = match &account.status {
            AccountStatus::Unverified => (Mark::Pending, Some("Testing…".to_owned())),
            AccountStatus::Verified { at_ms } => (
                Mark::Passed,
                Some(format!("Signed in {}", format::age(*at_ms, self.now_ms))),
            ),
            AccountStatus::Failed { error, .. } => (Mark::Failed, Some(clean(error))),
        };
        let check = |name: &str, host: &str, port: u16| ResultRow {
            title: name.to_owned(),
            detail: detail.clone(),
            meta: Some(clean(&format!("{host}:{port}"))),
            marks: vec![mark],
            command: None,
        };
        vec![Section {
            title: Some(clean(&account.email)),
            fields: Vec::new(),
            rows: vec![
                check("IMAP", &account.imap.host, account.imap.port),
                check("SMTP", &account.smtp.host, account.smtp.port),
            ],
        }]
    }

    fn discovery(&self, data: &Value) -> Vec<Section> {
        let mut fields = Vec::new();
        push(&mut fields, "Address", text(&data["email"]));
        push(&mut fields, "Domain", text(&data["domain"]));
        push(&mut fields, "Found in", text(&data["source"]));
        let server = |name: &str, e: &Value| {
            let host = text(&e["host"])?;
            let port = e["port"].as_u64().unwrap_or(0);
            Some(ResultRow {
                title: name.to_owned(),
                detail: text(&e["security"]),
                meta: Some(format!("{host}:{port}")),
                marks: vec![Mark::Passed],
                command: None,
            })
        };
        let rows: Vec<ResultRow> = [server("IMAP", &data["imap"]), server("SMTP", &data["smtp"])]
            .into_iter()
            .flatten()
            .collect();
        if rows.is_empty() {
            push(
                &mut fields,
                "State",
                text(&data["state"]).map(|s| {
                    if s == "started" {
                        "Looking…".to_owned()
                    } else {
                        s
                    }
                }),
            );
        }
        vec![Section {
            title: None,
            fields,
            rows,
        }]
    }

    fn mailbox(&self, v: &Value) -> Option<String> {
        let id = id::<MailboxId>(v)?;
        self.state
            .mailboxes
            .get(&id)
            .map(mailbox_name)
            .map(|n| clean(&n))
    }

    fn date(&self, v: &Value) -> Option<String> {
        v.as_i64().map(|ms| format::short_date(ms, self.now_ms))
    }
}

/// Grouped labelled fields: scalars of the top level first, then one section per object.
fn grouped(data: &Value) -> Vec<Section> {
    let Some(object) = data.as_object() else {
        let mut fields = Vec::new();
        push(&mut fields, "Value", scalar(data));
        return vec![Section {
            fields,
            ..Section::default()
        }];
    };
    let mut top = Section::default();
    let mut nested = Vec::new();
    for (key, value) in object {
        match value {
            Value::Object(inner) => nested.push(Section {
                title: Some(label(key)),
                fields: flatten(inner, ""),
                rows: Vec::new(),
            }),
            _ => push(&mut top.fields, &label(key), scalar(value)),
        }
    }
    let mut out = Vec::new();
    if !top.fields.is_empty() {
        out.push(top);
    }
    out.extend(nested);
    out
}

/// Every scalar under `object`, labelled by its path.
fn flatten(object: &Map<String, Value>, prefix: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    for (key, value) in object {
        let name = if prefix.is_empty() {
            label(key)
        } else {
            format!("{prefix} · {}", label(key))
        };
        match value {
            Value::Object(inner) => fields.extend(flatten(inner, &name)),
            _ => push(&mut fields, &name, scalar(value)),
        }
    }
    fields
}

fn empty(text: &str) -> Section {
    Section {
        fields: vec![Field {
            label: String::new(),
            value: text.to_owned(),
        }],
        ..Section::default()
    }
}

fn push(fields: &mut Vec<Field>, label: &str, value: Option<String>) {
    if let Some(value) = value.filter(|v| !v.is_empty()) {
        fields.push(Field {
            label: label.to_owned(),
            value,
        });
    }
}

/// `snake_case` as a label: `poll_secs` → `Poll secs`.
fn label(key: &str) -> String {
    let words = key.replace('_', " ");
    let mut chars = words.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// A scalar or a list of scalars as text; never braces.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(if *b { "yes" } else { "no" }.to_owned()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(clean(s)),
        Value::Array(list) => Some(
            list.iter()
                .filter_map(|v| scalar(v).or_else(|| addr(v)))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Value::Object(o) => Some(
            flatten(o, "")
                .into_iter()
                .map(|f| format!("{}: {}", f.label, f.value))
                .collect::<Vec<_>>()
                .join(", "),
        ),
    }
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(clean).filter(|s| !s.is_empty())
}

/// One address, `Name <email>` or the address alone.
fn addr(value: &Value) -> Option<String> {
    let email = value["email"].as_str()?;
    Some(clean(&match value["name"].as_str() {
        Some(name) if !name.is_empty() => format!("{name} <{email}>"),
        _ => email.to_owned(),
    }))
}

fn addrs(value: &Value) -> Option<String> {
    match value {
        Value::Array(list) => {
            let all: Vec<String> = list.iter().filter_map(addr).collect();
            (!all.is_empty()).then(|| all.join(", "))
        }
        other => addr(other),
    }
}

fn flags(value: &Value) -> Option<String> {
    let flags = value.as_object()?;
    let on: Vec<&str> = [
        ("seen", "read"),
        ("flagged", "starred"),
        ("answered", "answered"),
        ("draft", "draft"),
        ("pinned", "pinned"),
    ]
    .into_iter()
    .filter(|(k, _)| flags.get(*k).and_then(Value::as_bool) == Some(true))
    .map(|(_, name)| name)
    .collect();
    Some(if on.is_empty() {
        "unread".to_owned()
    } else {
        on.join(", ")
    })
}

fn items(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn id<T: serde::de::DeserializeOwned>(value: &Value) -> Option<T> {
    serde_json::from_value(value.clone()).ok()
}
