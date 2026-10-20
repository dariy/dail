//! The search query language (`ARCHITECTURE.md §8`): a pure parser and its canonical form.
//!
//! ```text
//! query   = { term }                      terms are separated by whitespace and all must match
//! term    = [ "-" ] ( field ":" value | phrase | word )      "-" negates the term
//! field   = from | to | cc | subject | is | has | in | before | after   (case-insensitive)
//! value   = phrase | word
//! phrase  = '"' { char | '\"' | '\\' } '"'
//! ```
//!
//! `is:` takes `unread`, `read` or `starred`; `has:` takes `attachment`; `in:` names a mailbox by
//! role (`inbox`, `sent`, …) or path; `before:`/`after:` take a UTC date `YYYY-MM-DD` (or
//! `YYYY/MM/DD`): `after:` includes the day, `before:` excludes it (IMAP `SINCE` / `BEFORE`). A
//! word with an unknown `prefix:` is plain text, so `re:` or a URL can be searched for.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::date::days_from_civil;
use crate::mailbox::Mailbox;
use crate::message::{Addr, Envelope};

/// How `search` orders its threads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchOrder {
    /// Newest first.
    #[default]
    Newest,
    /// Full-text hits first, best match first; then the rest, newest first.
    Relevance,
}

impl SearchOrder {
    /// [`SearchOrder::Newest`].
    pub const fn is_newest(&self) -> bool {
        matches!(self, Self::Newest)
    }
}

/// A parsed search query: every term must match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Query {
    /// The terms, in the order they were written.
    pub terms: Vec<Term>,
}

/// One condition of a query, possibly negated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Term {
    /// The term matches messages that do *not* satisfy `condition` (`-` prefix).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub negated: bool,
    /// What the message must satisfy.
    pub condition: Condition,
}

/// What a term tests. Text comparisons are case-insensitive substring matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum Condition {
    /// A word or `"phrase"`: in the subject or an address (and, on the server, the body).
    Text(String),
    /// `from:` — a sender's name or address.
    From(String),
    /// `to:` — a `To` recipient's name or address.
    To(String),
    /// `cc:` — a `Cc` recipient's name or address.
    Cc(String),
    /// `subject:` — the subject.
    Subject(String),
    /// `is:` — a flag state.
    Is(Status),
    /// `has:attachment`.
    HasAttachment,
    /// `in:` — the mailbox the message is shown in, by role (`inbox`, `sent`, …) or path.
    In(String),
    /// `before:` — sent before the start of this day (UTC).
    Before(Date),
    /// `after:` — sent on or after the start of this day (UTC).
    After(Date),
}

/// Values of `is:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Not `\Seen`.
    Unread,
    /// `\Seen`.
    Read,
    /// `\Flagged`.
    Starred,
}

impl Status {
    const ALL: [Self; 3] = [Self::Unread, Self::Read, Self::Starred];

    /// The value as written after `is:`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unread => "unread",
            Self::Read => "read",
            Self::Starred => "starred",
        }
    }
}

/// A calendar day (proleptic Gregorian, UTC).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct Date {
    /// Year, 1–9999.
    pub year: u16,
    /// Month, 1–12.
    pub month: u8,
    /// Day of the month, 1–31.
    pub day: u8,
}

impl Date {
    /// The day, if it exists in the calendar.
    pub fn new(year: u16, month: u8, day: u8) -> Option<Self> {
        let valid = (1..=9999).contains(&year)
            && (1..=12).contains(&month)
            && day >= 1
            && day <= days_in_month(year, month);
        valid.then_some(Self { year, month, day })
    }

    /// Midnight UTC at the start of the day, in Unix milliseconds.
    pub fn start_ms(self) -> i64 {
        days_from_civil(
            i64::from(self.year),
            u32::from(self.month),
            u32::from(self.day),
        ) * 86_400_000
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}

/// Why a query could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("at column {column}: {reason}")]
pub struct ParseError {
    /// 1-based character position of the problem.
    pub column: usize,
    /// What is wrong, and how to write it instead.
    pub reason: String,
}

impl Query {
    /// Parses a query. An empty (or all-whitespace) input gives a query without terms.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        Parser {
            chars: input.chars().collect(),
            pos: 0,
        }
        .query()
    }

    /// Whether the query has no terms.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether a cached message matches every term. `shown_in` is the mailbox the message is
    /// shown in ([`Envelope::location`]); `in:` never matches without it. Text is searched in
    /// the subject and the addresses only (the core cannot read bodies).
    pub fn matches(&self, message: &Envelope, shown_in: Option<&Mailbox>) -> bool {
        self.terms
            .iter()
            .all(|term| term.condition.matches(message, shown_in) != term.negated)
    }

    /// Like [`Query::matches`] for a message the server found for this query: the server
    /// searched its text (including the body), so text terms count as satisfied and only the
    /// other terms are checked against the cached state (which may be newer than the server's).
    pub fn matches_server_hit(&self, message: &Envelope, shown_in: Option<&Mailbox>) -> bool {
        self.terms.iter().all(|term| {
            matches!(term.condition, Condition::Text(_))
                || term.condition.matches(message, shown_in) != term.negated
        })
    }

    /// Whether a mailbox satisfies every `in:` term (the mailboxes a server search must look
    /// in, before defaults).
    pub fn selects_mailbox(&self, mailbox: &Mailbox) -> bool {
        self.terms.iter().all(|term| match &term.condition {
            Condition::In(name) => mailbox_named(mailbox, name) != term.negated,
            _ => true,
        })
    }

    /// Whether some term requires a particular mailbox (a non-negated `in:`).
    pub fn names_mailbox(&self) -> bool {
        self.terms
            .iter()
            .any(|term| !term.negated && matches!(term.condition, Condition::In(_)))
    }

    /// The query without its `in:` terms: what a server search of one mailbox asks for.
    pub fn without_mailboxes(&self) -> Self {
        Self {
            terms: self
                .terms
                .iter()
                .filter(|term| !matches!(term.condition, Condition::In(_)))
                .cloned()
                .collect(),
        }
    }
}

/// Whether `name` is the mailbox's path or role name (case-insensitive).
fn mailbox_named(mailbox: &Mailbox, name: &str) -> bool {
    mailbox.path.eq_ignore_ascii_case(name)
        || mailbox
            .role
            .is_some_and(|role| role.as_str().eq_ignore_ascii_case(name))
}

impl Condition {
    fn matches(&self, message: &Envelope, shown_in: Option<&Mailbox>) -> bool {
        match self {
            Self::Text(text) => {
                let needle = text.to_lowercase();
                contains(&message.subject, &needle)
                    || [&message.from, &message.to, &message.cc]
                        .into_iter()
                        .any(|addrs| addrs_contain(addrs, &needle))
            }
            Self::From(value) => addrs_contain(&message.from, &value.to_lowercase()),
            Self::To(value) => addrs_contain(&message.to, &value.to_lowercase()),
            Self::Cc(value) => addrs_contain(&message.cc, &value.to_lowercase()),
            Self::Subject(value) => contains(&message.subject, &value.to_lowercase()),
            Self::Is(Status::Unread) => !message.flags.seen,
            Self::Is(Status::Read) => message.flags.seen,
            Self::Is(Status::Starred) => message.flags.flagged,
            Self::HasAttachment => message.has_attachments,
            Self::In(name) => shown_in.is_some_and(|mailbox| mailbox_named(mailbox, name)),
            Self::Before(date) => message.date_ms < date.start_ms(),
            Self::After(date) => message.date_ms >= date.start_ms(),
        }
    }
}

/// Case-insensitive substring test; `needle` is already lower-case.
fn contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(needle)
}

fn addrs_contain(addrs: &[Addr], needle: &str) -> bool {
    addrs.iter().any(|addr| {
        contains(&addr.email, needle) || addr.name.as_deref().is_some_and(|n| contains(n, needle))
    })
}

/// The canonical form, which parses back to the same query.
impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, term) in self.terms.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{term}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negated {
            f.write_str("-")?;
        }
        write!(f, "{}", self.condition)
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => {
                // A bare word must not look like a negation, a field or a phrase.
                if needs_quotes(text) || text.starts_with('-') || text.contains(':') {
                    write_quoted(f, text)
                } else {
                    f.write_str(text)
                }
            }
            Self::From(value) => write_field(f, "from", value),
            Self::To(value) => write_field(f, "to", value),
            Self::Cc(value) => write_field(f, "cc", value),
            Self::Subject(value) => write_field(f, "subject", value),
            Self::In(value) => write_field(f, "in", value),
            Self::Is(status) => write!(f, "is:{}", status.as_str()),
            Self::HasAttachment => f.write_str("has:attachment"),
            Self::Before(date) => write!(f, "before:{date}"),
            Self::After(date) => write!(f, "after:{date}"),
        }
    }
}

fn needs_quotes(text: &str) -> bool {
    text.is_empty()
        || text.starts_with('"')
        || text
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\\')
}

fn write_field(f: &mut fmt::Formatter<'_>, name: &str, value: &str) -> fmt::Result {
    write!(f, "{name}:")?;
    if needs_quotes(value) {
        write_quoted(f, value)
    } else {
        f.write_str(value)
    }
}

fn write_quoted(f: &mut fmt::Formatter<'_>, text: &str) -> fmt::Result {
    f.write_str("\"")?;
    for c in text.chars() {
        if matches!(c, '"' | '\\') {
            f.write_str("\\")?;
        }
        write!(f, "{c}")?;
    }
    f.write_str("\"")
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn query(mut self) -> Result<Query, ParseError> {
        let mut terms = Vec::new();
        loop {
            self.skip_whitespace();
            let Some(c) = self.peek() else {
                return Ok(Query { terms });
            };
            let negated = c == '-';
            if negated {
                self.pos += 1;
                // A lone `-` negates nothing; it is ignored.
                if self.peek().is_none_or(char::is_whitespace) {
                    continue;
                }
            }
            let condition = self.condition()?;
            terms.push(Term { negated, condition });
        }
    }

    fn condition(&mut self) -> Result<Condition, ParseError> {
        let start = self.pos;
        if self.peek() == Some('"') {
            let phrase = self.quoted()?;
            if phrase.is_empty() {
                return Err(self.error_at(start, "empty phrase; remove the quotes"));
            }
            return Ok(Condition::Text(phrase));
        }
        let word = self.word();
        let Some((name, rest)) = word.split_once(':') else {
            return Ok(Condition::Text(word));
        };
        let Some(field) = Field::from_name(name) else {
            return Ok(Condition::Text(word));
        };
        let value_start = start + name.chars().count() + 1;
        let value = if rest.is_empty() && self.peek() == Some('"') {
            self.quoted()?
        } else {
            rest.to_owned()
        };
        if value.is_empty() {
            return Err(self.error_at(value_start, &format!("`{}:` needs a value", field.name())));
        }
        field
            .condition(value)
            .map_err(|reason| self.error_at(value_start, &reason))
    }

    /// A bare word: everything up to the next whitespace. Stops before a `"` that directly
    /// follows `field:`, so that `from:"a b"` reads the quoted value.
    fn word(&mut self) -> String {
        let mut word = String::new();
        while let Some(c) = self.peek() {
            let after_field = word
                .strip_suffix(':')
                .is_some_and(|name| !name.contains(':') && Field::from_name(name).is_some());
            if c.is_whitespace() || (c == '"' && after_field) {
                break;
            }
            word.push(c);
            self.pos += 1;
        }
        word
    }

    /// A `"…"` string with `\"` and `\\` escapes. Must be followed by whitespace or the end.
    fn quoted(&mut self) -> Result<String, ParseError> {
        let open = self.pos;
        self.pos += 1;
        let mut text = String::new();
        loop {
            match self.peek() {
                None => return Err(self.error_at(open, "unterminated quote; add a closing `\"`")),
                Some('"') => {
                    self.pos += 1;
                    break;
                }
                Some('\\') if matches!(self.chars.get(self.pos + 1), Some('"' | '\\')) => {
                    text.push(self.chars[self.pos + 1]);
                    self.pos += 2;
                }
                Some(c) => {
                    text.push(c);
                    self.pos += 1;
                }
            }
        }
        if self.peek().is_some_and(|c| !c.is_whitespace()) {
            return Err(self.error_at(self.pos, "expected a space after the closing `\"`"));
        }
        Ok(text)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn error_at(&self, pos: usize, reason: &str) -> ParseError {
        ParseError {
            column: pos + 1,
            reason: reason.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Field {
    From,
    To,
    Cc,
    Subject,
    Is,
    Has,
    In,
    Before,
    After,
}

impl Field {
    const ALL: [Self; 9] = [
        Self::From,
        Self::To,
        Self::Cc,
        Self::Subject,
        Self::Is,
        Self::Has,
        Self::In,
        Self::Before,
        Self::After,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::From => "from",
            Self::To => "to",
            Self::Cc => "cc",
            Self::Subject => "subject",
            Self::Is => "is",
            Self::Has => "has",
            Self::In => "in",
            Self::Before => "before",
            Self::After => "after",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|field| field.name().eq_ignore_ascii_case(name))
    }

    fn condition(self, value: String) -> Result<Condition, String> {
        match self {
            Self::From => Ok(Condition::From(value)),
            Self::To => Ok(Condition::To(value)),
            Self::Cc => Ok(Condition::Cc(value)),
            Self::Subject => Ok(Condition::Subject(value)),
            Self::In => Ok(Condition::In(value)),
            Self::Is => Status::ALL
                .into_iter()
                .find(|status| status.as_str().eq_ignore_ascii_case(&value))
                .map(Condition::Is)
                .ok_or_else(|| {
                    format!("unknown `is:{value}`; use is:unread, is:read or is:starred")
                }),
            Self::Has => {
                if value.eq_ignore_ascii_case("attachment")
                    || value.eq_ignore_ascii_case("attachments")
                {
                    Ok(Condition::HasAttachment)
                } else {
                    Err(format!("unknown `has:{value}`; use has:attachment"))
                }
            }
            Self::Before => parse_date(&value).map(Condition::Before),
            Self::After => parse_date(&value).map(Condition::After),
        }
    }
}

/// `YYYY-MM-DD` or `YYYY/MM/DD` (month and day may have one digit).
fn parse_date(value: &str) -> Result<Date, String> {
    let invalid = || format!("invalid date `{value}`; write YYYY-MM-DD");
    let separator = if value.contains('/') { '/' } else { '-' };
    let parts: Vec<&str> = value.split(separator).collect();
    let [year, month, day] = parts.as_slice() else {
        return Err(invalid());
    };
    let digits = |s: &str, min: usize, max: usize| {
        (min..=max).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit())
    };
    if !digits(year, 4, 4) || !digits(month, 1, 2) || !digits(day, 1, 2) {
        return Err(invalid());
    }
    let (Ok(year), Ok(month), Ok(day)) = (year.parse(), month.parse(), day.parse()) else {
        return Err(invalid());
    };
    Date::new(year, month, day).ok_or_else(|| format!("`{value}` is not a calendar date"))
}
