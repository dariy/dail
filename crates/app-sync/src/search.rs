//! Server search: a [`Query`] as IMAP `SEARCH` criteria (RFC 3501 §6.4.4) or as a Gmail
//! `X-GM-RAW` query (`ARCHITECTURE.md §5.2`).
//!
//! Query text comes from users and agents, so every string is sent through
//! [`crate::imap::quoted`], which replaces control characters (CR, LF, …) by spaces: a query can
//! never end the command line and inject another IMAP command.

use app_core::search::{Condition, Date, Query, Status};

use crate::imap::quoted;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The argument of `UID SEARCH` for `query`: IMAP criteria, or `X-GM-RAW "…"` with
/// `gmail_raw`. `CHARSET UTF-8` is prepended when a value is not ASCII.
pub fn search_arguments(query: &Query, gmail_raw: bool) -> String {
    let criteria = if gmail_raw {
        format!("X-GM-RAW {}", quoted(&gmail_query(query)))
    } else {
        imap_criteria(query)
    };
    if criteria.is_ascii() {
        criteria
    } else {
        format!("CHARSET UTF-8 {criteria}")
    }
}

/// IMAP `SEARCH` keys for `query`, all of which must match (`ALL` without terms). `in:` terms
/// are ignored (the mailbox is chosen by the caller). Dates compare the `Date` header
/// (`SENTBEFORE` / `SENTSINCE`), like the local search; `has:attachment` is approximated by a
/// `multipart/mixed` content type.
pub fn imap_criteria(query: &Query) -> String {
    let mut keys = Vec::new();
    for term in &query.terms {
        let key = match &term.condition {
            Condition::Text(text) => format!("TEXT {}", quoted(text)),
            Condition::From(value) => format!("FROM {}", quoted(value)),
            Condition::To(value) => format!("TO {}", quoted(value)),
            Condition::Cc(value) => format!("CC {}", quoted(value)),
            Condition::Subject(value) => format!("SUBJECT {}", quoted(value)),
            Condition::Is(Status::Unread) => "UNSEEN".to_owned(),
            Condition::Is(Status::Read) => "SEEN".to_owned(),
            Condition::Is(Status::Starred) => "FLAGGED".to_owned(),
            Condition::HasAttachment => "HEADER Content-Type \"multipart/mixed\"".to_owned(),
            Condition::Before(date) => format!("SENTBEFORE {}", imap_date(*date)),
            Condition::After(date) => format!("SENTSINCE {}", imap_date(*date)),
            Condition::In(_) => continue,
        };
        keys.push(if term.negated {
            format!("NOT {key}")
        } else {
            key
        });
    }
    if keys.is_empty() {
        "ALL".to_owned()
    } else {
        keys.join(" ")
    }
}

/// `query` in Gmail's search syntax. Dates become Unix seconds (`after:1700000000`), which
/// Gmail compares exactly, instead of calendar days it reads in Pacific time. `in:` terms are
/// left out (the mailbox is chosen by the caller).
pub fn gmail_query(query: &Query) -> String {
    let mut out = String::new();
    for term in &query.terms {
        let operand = match &term.condition {
            Condition::Text(text) => gmail_value(text),
            Condition::From(value) => format!("from:{}", gmail_value(value)),
            Condition::To(value) => format!("to:{}", gmail_value(value)),
            Condition::Cc(value) => format!("cc:{}", gmail_value(value)),
            Condition::Subject(value) => format!("subject:{}", gmail_value(value)),
            Condition::Is(status) => format!("is:{}", status.as_str()),
            Condition::HasAttachment => "has:attachment".to_owned(),
            Condition::Before(date) => format!("before:{}", date.start_ms() / 1000),
            Condition::After(date) => format!("after:{}", date.start_ms() / 1000),
            Condition::In(_) => continue,
        };
        if !out.is_empty() {
            out.push(' ');
        }
        if term.negated {
            out.push('-');
        }
        out.push_str(&operand);
    }
    out
}

/// A Gmail search value: a bare word, or a `"phrase"` if it has spaces or characters Gmail
/// would read as syntax. Gmail has no escapes, so quotes inside become spaces.
fn gmail_value(value: &str) -> String {
    let clean: String = value
        .chars()
        .map(|c| if c == '"' || c.is_control() { ' ' } else { c })
        .collect();
    let plain = !clean.is_empty()
        && !clean.starts_with('-')
        && clean
            .chars()
            .all(|c| !c.is_whitespace() && !matches!(c, ':' | '(' | ')' | '{' | '}'));
    if plain { clean } else { format!("\"{clean}\"") }
}

/// `d-Mon-yyyy`, the IMAP date format.
fn imap_date(date: Date) -> String {
    let month = MONTHS
        .get(usize::from(date.month).saturating_sub(1))
        .copied()
        .unwrap_or("Jan");
    format!("{}-{month}-{:04}", date.day, date.year)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(text: &str) -> Query {
        Query::parse(text).unwrap()
    }

    #[test]
    fn every_term_becomes_an_imap_key() {
        assert_eq!(
            imap_criteria(&query(
                r#"plans "next week" from:alice to:bob cc:carol subject:"Q3 report" is:unread
                   -is:read is:starred has:attachment before:2024-02-09 after:2023-11-14"#
            )),
            "TEXT \"plans\" TEXT \"next week\" FROM \"alice\" TO \"bob\" CC \"carol\" \
             SUBJECT \"Q3 report\" UNSEEN NOT SEEN FLAGGED HEADER Content-Type \
             \"multipart/mixed\" SENTBEFORE 9-Feb-2024 SENTSINCE 14-Nov-2023"
        );
    }

    #[test]
    fn mailbox_terms_are_left_to_the_caller() {
        assert_eq!(imap_criteria(&query("in:inbox -in:trash")), "ALL");
        assert_eq!(
            imap_criteria(&query("in:inbox -lunch")),
            "NOT TEXT \"lunch\""
        );
        assert_eq!(gmail_query(&query("in:inbox -lunch")), "-lunch");
    }

    #[test]
    fn strings_cannot_break_out_of_the_command() {
        let evil = Query {
            terms: vec![app_core::search::Term {
                negated: false,
                condition: Condition::Subject("x\"\r\nA1 DELETE INBOX\\".into()),
            }],
        };
        let criteria = imap_criteria(&evil);
        assert_eq!(criteria, r#"SUBJECT "x\"  A1 DELETE INBOX\\""#);
        assert!(!criteria.contains(['\r', '\n']));
        let raw = search_arguments(&evil, true);
        assert!(!raw.contains(['\r', '\n']), "{raw}");
    }

    #[test]
    fn gmail_gets_its_own_syntax() {
        assert_eq!(
            gmail_query(&query(
                r#"from:"Alice Example" subject:re:plans -is:read has:attachment "say \"hi\"" after:2023-11-14 before:2023-11-15"#
            )),
            "from:\"Alice Example\" subject:\"re:plans\" -is:read has:attachment \"say  hi \" \
             after:1699920000 before:1700006400"
        );
        assert_eq!(
            search_arguments(&query(r#"from:"Alice Example" lunch"#), true),
            r#"X-GM-RAW "from:\"Alice Example\" lunch""#
        );
    }

    #[test]
    fn non_ascii_values_declare_utf8() {
        assert_eq!(
            search_arguments(&query("from:zoë"), false),
            "CHARSET UTF-8 FROM \"zoë\""
        );
        assert_eq!(
            search_arguments(&query("Grüße"), true),
            "CHARSET UTF-8 X-GM-RAW \"Grüße\""
        );
        assert_eq!(search_arguments(&query("is:unread"), false), "UNSEEN");
    }
}
