//! Agents' send requests as cards (`ARCHITECTURE.md §4.6`).
//!
//! A card shows what would leave — every recipient, the subject, the start of the text and
//! every attachment with its full path — so the user sees what the agent wrote before deciding.
//! The text is the agent's and may quote email: it is shown as plain text, never interpreted.

use app_core::{Addr, AppState, ApprovalId, AttachmentState, DraftId};

use crate::format;

/// How many characters of the body a card shows.
const EXCERPT_CHARS: usize = 280;
/// How many lines of the body a card shows.
const EXCERPT_LINES: usize = 4;

/// One request, ready to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// The request.
    pub id: ApprovalId,
    /// The draft it would send.
    pub draft: DraftId,
    /// `To:`, `Cc:` and `Bcc:` lines (only the non-empty ones), with full addresses.
    pub recipients: Vec<String>,
    /// The subject, or `(no subject)`.
    pub subject: String,
    /// The start of the text.
    pub excerpt: String,
    /// One line per attachment: the full path and its size or state.
    pub attachments: Vec<String>,
    /// The address it is sent from (the draft's identity, else the account's).
    pub from: String,
    /// Whether every attachment has been checked (approving fails otherwise).
    pub ready: bool,
}

/// The cards of every pending request, oldest first.
pub fn cards(state: &AppState) -> Vec<Card> {
    state
        .approvals
        .values()
        .filter_map(|approval| {
            let draft = state.drafts.get(&approval.draft)?;
            let line = |label: &str, addrs: &[Addr]| {
                (!addrs.is_empty()).then(|| {
                    let list: Vec<String> = addrs.iter().map(format::full).collect();
                    format!("{label} {}", list.join(", "))
                })
            };
            let recipients = [
                line("To:", &draft.to),
                line("Cc:", &draft.cc),
                line("Bcc:", &draft.bcc),
            ]
            .into_iter()
            .flatten()
            .collect();
            let subject = match draft.subject.trim() {
                "" => "(no subject)".to_owned(),
                subject => subject.to_owned(),
            };
            let attachments = draft
                .attachments
                .iter()
                .map(|attachment| {
                    let state = match &attachment.state {
                        AttachmentState::Checking => "checking…".to_owned(),
                        AttachmentState::Ready { size, .. } => format::size(*size),
                        AttachmentState::Failed { error } => format!("failed: {error}"),
                    };
                    format!("{} ({state})", attachment.path)
                })
                .collect();
            let ready = draft
                .attachments
                .iter()
                .all(|a| matches!(a.state, AttachmentState::Ready { .. }));
            // The address it goes out from: an alias if the draft has one, else the account's.
            let from = draft.from.as_ref().map_or_else(
                || {
                    state
                        .accounts
                        .get(&draft.account)
                        .map_or_else(String::new, |account| account.email.clone())
                },
                |from| from.email.clone(),
            );
            Some(Card {
                id: approval.id,
                draft: draft.id,
                recipients,
                subject,
                excerpt: excerpt(&draft.body_text),
                attachments,
                from,
                ready,
            })
        })
        .collect()
}

/// The first lines of `text`, shortened with `…`.
pub fn excerpt(text: &str) -> String {
    let lines: Vec<&str> = text.trim().lines().collect();
    let mut out = lines
        .iter()
        .take(EXCERPT_LINES)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let mut cut = lines.len() > EXCERPT_LINES;
    if out.chars().count() > EXCERPT_CHARS {
        out = out.chars().take(EXCERPT_CHARS).collect();
        cut = true;
    }
    if cut {
        out.push('…');
    }
    out
}
