//! The Rules screen of the settings (ADR 0015, SET-5): the sorting rules as rows, each with a
//! summary in words of its condition and actions, and whether local edits wait for a push.
//!
//! Rule text (names, patterns, folder paths) can come from any device or client that wrote the
//! server's settings folder: it is untrusted. Everything here passes through
//! [`clean`](crate::text::clean) and is meant to be drawn as plain text, never as markup or links.

use app_core::{AppState, Condition, MailboxId, Rule, RuleAction, RuleId};

use crate::i18n::{Key, f, s};
use crate::text::clean;

/// The longest summary of a condition, in characters; the rest is elided.
const SUMMARY_MAX: usize = 240;

/// One rule on the Rules screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRow {
    /// The rule.
    pub id: RuleId,
    /// Its name (cleaned; untrusted).
    pub name: String,
    /// Whether it runs.
    pub enabled: bool,
    /// `when → then` in words (cleaned; untrusted).
    pub summary: String,
}

/// The Rules screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// The live rules, in `RuleId` order (the order they run in).
    pub rows: Vec<RuleRow>,
    /// Local rule edits not stored on the server yet (`synced.dirty`).
    pub dirty: bool,
    /// The mailbox *Apply to this mailbox* runs on: the open one, not the unified inbox.
    pub mailbox: Option<MailboxId>,
    /// Whether *Store on the server* can run: an account exists and no push is under way.
    pub can_push: bool,
}

/// The Rules screen of `state`.
pub fn panel(state: &AppState) -> Panel {
    Panel {
        rows: rows(state),
        dirty: state.synced.dirty,
        mailbox: state.view.mailbox.filter(|_| !state.view.unified),
        can_push: !state.accounts.is_empty() && !state.synced.pushing,
    }
}

/// The live rules as rows.
pub fn rows(state: &AppState) -> Vec<RuleRow> {
    state
        .synced
        .settings
        .live_rules()
        .map(|(id, rule)| RuleRow {
            id,
            name: clean(&rule.name),
            enabled: rule.enabled,
            summary: summary(rule),
        })
        .collect()
}

/// A rule in words: `from contains “x” → move to Reading, mark read`.
pub fn summary(rule: &Rule) -> String {
    f(
        Key::RuleSummary,
        &[("when", &when(&rule.when)), ("then", &then(&rule.then))],
    )
}

/// A condition in words, cleaned and at most [`SUMMARY_MAX`] characters.
pub fn when(condition: &Condition) -> String {
    let text = clean(&condition_text(condition, 0));
    if text.chars().count() > SUMMARY_MAX {
        let mut cut: String = text.chars().take(SUMMARY_MAX).collect();
        cut.push('…');
        cut
    } else {
        text
    }
}

/// The actions in words, cleaned, separated by commas.
pub fn then(actions: &[RuleAction]) -> String {
    if actions.is_empty() {
        return s(Key::RuleDoNothing).to_owned();
    }
    let parts: Vec<String> = actions.iter().map(action_text).collect();
    clean(&parts.join(", "))
}

/// One action in words.
pub fn action_text(action: &RuleAction) -> String {
    match action {
        RuleAction::Move { to } => f(Key::RuleActMove, &[("to", to)]),
        RuleAction::AddLabel { label } => f(Key::RuleActLabel, &[("label", label)]),
        RuleAction::MarkRead => s(Key::RuleActRead).to_owned(),
        RuleAction::Star => s(Key::RuleActStar).to_owned(),
        RuleAction::Archive => s(Key::RuleActArchive).to_owned(),
    }
}

/// The deepest nesting spelled out; deeper parts read `…` (a valid rule is at most 8 deep).
const DEPTH_MAX: usize = 8;

fn condition_text(condition: &Condition, depth: usize) -> String {
    if depth > DEPTH_MAX {
        return "…".to_owned();
    }
    let join = |of: &[Condition], sep: Key| {
        let parts: Vec<String> = of
            .iter()
            .map(|c| {
                let text = condition_text(c, depth + 1);
                if matches!(c, Condition::All { of } | Condition::Any { of } if of.len() > 1) {
                    format!("({text})")
                } else {
                    text
                }
            })
            .collect();
        parts.join(s(sep))
    };
    match condition {
        Condition::All { of } if of.is_empty() => s(Key::RuleEveryMessage).to_owned(),
        Condition::Any { of } if of.is_empty() => s(Key::RuleNoMessage).to_owned(),
        Condition::All { of } => join(of, Key::RuleAnd),
        Condition::Any { of } => join(of, Key::RuleOr),
        Condition::Not { of } => f(
            Key::RuleNot,
            &[("condition", &condition_text(of, depth + 1))],
        ),
        Condition::From { contains } => f(Key::RuleCondFrom, &[("text", contains)]),
        Condition::To { contains } => f(Key::RuleCondTo, &[("text", contains)]),
        Condition::Cc { contains } => f(Key::RuleCondCc, &[("text", contains)]),
        Condition::Subject { contains } => f(Key::RuleCondSubject, &[("text", contains)]),
        Condition::ListId { contains } => f(Key::RuleCondList, &[("text", contains)]),
        Condition::HasAttachment => s(Key::RuleCondAttachment).to_owned(),
        Condition::Mailbox { path } => f(Key::RuleCondMailbox, &[("path", path)]),
    }
}
