//! Composer rules that do not depend on a toolkit: address parsing, recipient fields, the
//! draft's status line and whether it can be sent.

use app_core::{Addr, AppState, AttachmentState, Command, Draft};

/// A recipient field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// `To`.
    To,
    /// `Cc`.
    Cc,
    /// `Bcc`.
    Bcc,
}

impl Field {
    /// The three fields, in order.
    pub const ALL: [Self; 3] = [Self::To, Self::Cc, Self::Bcc];

    /// `To`, `Cc` or `Bcc`.
    pub const fn label(self) -> &'static str {
        match self {
            Self::To => "To",
            Self::Cc => "Cc",
            Self::Bcc => "Bcc",
        }
    }

    /// The field's addresses in `draft`.
    pub fn of(self, draft: &Draft) -> &[Addr] {
        match self {
            Self::To => &draft.to,
            Self::Cc => &draft.cc,
            Self::Bcc => &draft.bcc,
        }
    }

    /// The `update_draft` that sets this field of `draft` to `list`.
    pub fn update(self, draft: &Draft, list: Vec<Addr>) -> Command {
        let (to, cc, bcc) = match self {
            Self::To => (Some(list), None, None),
            Self::Cc => (None, Some(list), None),
            Self::Bcc => (None, None, Some(list)),
        };
        Command::UpdateDraft {
            id: draft.id,
            to,
            cc,
            bcc,
            subject: None,
            body: None,
        }
    }
}

/// Parses `Name <a@b>`, `"Name" <a@b>` or `a@b`.
pub fn parse_addr(text: &str) -> Option<Addr> {
    let text = text.trim().trim_end_matches([',', ';']).trim();
    let (name, email) = match (text.rfind('<'), text.strip_suffix('>')) {
        (Some(open), Some(inner)) => {
            let name = text[..open].trim().trim_matches('"').trim();
            (
                (!name.is_empty()).then(|| name.to_owned()),
                inner.get(open + 1..)?.trim(),
            )
        }
        _ => (None, text),
    };
    let valid = email.contains('@')
        && !email.starts_with('@')
        && !email.ends_with('@')
        && !email.chars().any(char::is_whitespace);
    valid.then(|| Addr {
        name,
        email: email.to_owned(),
    })
}

/// The result of adding typed text to a recipient field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// The field's new addresses; `None` if nothing was added.
    pub list: Option<Vec<Addr>>,
    /// The parts that are not addresses, to leave in the input (`, `-separated).
    pub rejected: String,
}

/// Adds the addresses in `input` (separated by `,` or `;`) to `current`, skipping ones already
/// there (case-insensitive). Invalid parts stay in the input.
pub fn add_recipients(current: &[Addr], input: &str) -> Added {
    let mut list = current.to_vec();
    let mut rejected = Vec::new();
    for part in input.split([',', ';']) {
        if part.trim().is_empty() {
            continue;
        }
        match parse_addr(part) {
            Some(addr)
                if !list
                    .iter()
                    .any(|a| a.email.eq_ignore_ascii_case(&addr.email)) =>
            {
                list.push(addr);
            }
            Some(_) => {}
            None => rejected.push(part.trim().to_owned()),
        }
    }
    Added {
        list: (list.len() != current.len()).then_some(list),
        rejected: rejected.join(", "),
    }
}

/// Seconds until a scheduled send leaves, rounded up.
pub const fn seconds_left(send_at: i64, now_ms: i64) -> i64 {
    let left = send_at - now_ms;
    (if left > 0 { left } else { 0 } + 999) / 1000
}

/// The draft's state in a word or two: `sending…`, `sending in 7 s`, `saving…`, `saved`, …
pub fn draft_status(state: &AppState, draft: &Draft, now_ms: i64) -> String {
    if draft.sending {
        "sending…".to_owned()
    } else if let Some(at) = draft.send_at {
        format!("sending in {} s", seconds_left(at, now_ms))
    } else if draft.save_error.is_some() {
        "not saved to the server".to_owned()
    } else if state.status.drafts_saving.contains(&draft.id) {
        "saving…".to_owned()
    } else if draft.saved_ms == Some(draft.updated_ms) {
        "saved".to_owned()
    } else {
        "draft".to_owned()
    }
}

/// Whether `draft` has a recipient and every attachment is checked (the composer's *Send*).
pub fn sendable(draft: &Draft) -> bool {
    !(draft.to.is_empty() && draft.cc.is_empty() && draft.bcc.is_empty())
        && draft
            .attachments
            .iter()
            .all(|a| matches!(a.state, AttachmentState::Ready { .. }))
}

/// The composer's title for `draft`: `Reply`, `Forward` or `New message`.
pub fn title(draft: &Draft) -> &'static str {
    match &draft.quote {
        Some(q) if q.style == app_core::QuoteStyle::Forward => "Forward",
        Some(_) => "Reply",
        None => "New message",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_parse() {
        assert_eq!(
            parse_addr("Bob <bob@example.org>"),
            Some(Addr {
                name: Some("Bob".into()),
                email: "bob@example.org".into()
            })
        );
        assert_eq!(
            parse_addr(" \"Bob B\" <bob@example.org>, "),
            Some(Addr {
                name: Some("Bob B".into()),
                email: "bob@example.org".into()
            })
        );
        assert_eq!(
            parse_addr("carol@example.org"),
            Some(Addr {
                name: None,
                email: "carol@example.org".into()
            })
        );
        for bad in ["", "bob", "@x", "x@", "a b@c", "<>"] {
            assert_eq!(parse_addr(bad), None, "{bad}");
        }
    }

    #[test]
    fn recipients_are_added_once_and_bad_text_stays() {
        let bob = Addr {
            name: None,
            email: "bob@example.org".into(),
        };
        let added = add_recipients(
            std::slice::from_ref(&bob),
            "BOB@example.org, carol@example.org; nope",
        );
        assert_eq!(added.rejected, "nope");
        let list = added.list.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].email, "carol@example.org");
        assert_eq!(add_recipients(&[bob], "bob@example.org").list, None);
        assert_eq!(seconds_left(10_000, 1_500), 9);
        assert_eq!(seconds_left(1_000, 5_000), 0);
    }
}
