//! The sorting-rule evaluator (ADR 0015, SET-4): a pure function from the live rules and a
//! message's envelope to what the rules want done with it.
//!
//! Semantics, which every device shares so that a rule sorts a message the same way whichever
//! device fetches it:
//!
//! - Text tests (`from`, `to`, `cc`, `subject`, `list_id`) are case-insensitive substring
//!   matches (Unicode lower case). An address matches if its name or its email contains the
//!   text. A missing header matches nothing. `mailbox` compares the folder's full path exactly.
//!   `all` of nothing matches, `any` of nothing does not.
//! - Header text and rule text are data: they are compared, never interpreted.
//! - Rules run in ID order; every enabled rule whose condition matches contributes its actions.
//!   Flags (`mark_read`, `star`) add up. A message goes to at most one place: the first `move` or
//!   `archive` wins, later ones are ignored.
//! - Scope on fetch: a rule whose condition has no `mailbox` test applies to new mail of the
//!   Inbox only (as a delivery filter would); one that tests the mailbox applies wherever its
//!   condition holds. `apply_rules` runs every enabled rule over the mailbox it names.

use crate::ids::{MessageId, RuleId};
use crate::message::{Addr, Envelope};
use crate::synced::{Condition, Rule, RuleAction};

/// Where the rules want a message to go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// To the folder with this full path.
    Move(String),
    /// To the account's archive.
    Archive,
}

/// What the matching rules want done with one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePlan {
    /// The message.
    pub message: MessageId,
    /// The rules that matched, in ID order.
    pub rules: Vec<RuleId>,
    /// Mark it read.
    pub mark_read: bool,
    /// Star it.
    pub star: bool,
    /// Move or archive it.
    pub place: Option<Placement>,
    /// `add_label` targets, which this version does not run yet (reported as skipped).
    pub labels: Vec<String>,
}

/// When the rules run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// New mail fetched into a mailbox; `inbox` if that mailbox is the account's Inbox.
    Fetch {
        /// The mailbox has the Inbox role.
        inbox: bool,
    },
    /// `apply_rules` over a mailbox the caller named.
    Explicit,
}

impl Condition {
    /// Whether the condition holds for `envelope`, shown in the folder with full path `path`.
    pub fn matches(&self, envelope: &Envelope, path: &str) -> bool {
        match self {
            Self::All { of } => of.iter().all(|c| c.matches(envelope, path)),
            Self::Any { of } => of.iter().any(|c| c.matches(envelope, path)),
            Self::Not { of } => !of.matches(envelope, path),
            Self::From { contains } => addrs_contain(&envelope.from, contains),
            Self::To { contains } => addrs_contain(&envelope.to, contains),
            Self::Cc { contains } => addrs_contain(&envelope.cc, contains),
            Self::Subject { contains } => contains_ci(&envelope.subject, contains),
            Self::ListId { contains } => envelope
                .list_id
                .as_deref()
                .is_some_and(|id| contains_ci(id, contains)),
            Self::HasAttachment => envelope.has_attachments,
            Self::Mailbox { path: wanted } => wanted == path,
        }
    }

    /// Whether a `mailbox` test appears anywhere in the condition.
    pub fn names_mailbox(&self) -> bool {
        match self {
            Self::All { of } | Self::Any { of } => of.iter().any(Self::names_mailbox),
            Self::Not { of } => of.names_mailbox(),
            Self::Mailbox { .. } => true,
            Self::From { .. }
            | Self::To { .. }
            | Self::Cc { .. }
            | Self::Subject { .. }
            | Self::ListId { .. }
            | Self::HasAttachment => false,
        }
    }
}

impl Rule {
    /// Whether the rule applies to `envelope` in `path` under `scope`: enabled, in scope and
    /// its condition holds.
    pub fn applies(&self, envelope: &Envelope, path: &str, scope: Scope) -> bool {
        let in_scope = match scope {
            Scope::Fetch { inbox } => inbox || self.when.names_mailbox(),
            Scope::Explicit => true,
        };
        self.enabled && in_scope && self.when.matches(envelope, path)
    }
}

/// What `rules` (in ID order) want done with `envelope`, shown in `path`; `None` if no rule
/// applies.
pub fn plan<'a>(
    rules: impl IntoIterator<Item = (RuleId, &'a Rule)>,
    envelope: &Envelope,
    path: &str,
    scope: Scope,
) -> Option<RulePlan> {
    let mut plan = RulePlan {
        message: envelope.id,
        rules: Vec::new(),
        mark_read: false,
        star: false,
        place: None,
        labels: Vec::new(),
    };
    for (id, rule) in rules {
        if !rule.applies(envelope, path, scope) {
            continue;
        }
        plan.rules.push(id);
        for action in &rule.then {
            match action {
                RuleAction::MarkRead => plan.mark_read = true,
                RuleAction::Star => plan.star = true,
                RuleAction::Move { to } => {
                    plan.place
                        .get_or_insert_with(|| Placement::Move(to.clone()));
                }
                RuleAction::Archive => {
                    plan.place.get_or_insert(Placement::Archive);
                }
                RuleAction::AddLabel { label } => {
                    if !plan.labels.contains(label) {
                        plan.labels.push(label.clone());
                    }
                }
            }
        }
    }
    (!plan.rules.is_empty()).then_some(plan)
}

fn addrs_contain(addrs: &[Addr], needle: &str) -> bool {
    addrs.iter().any(|addr| {
        contains_ci(&addr.email, needle)
            || addr
                .name
                .as_deref()
                .is_some_and(|name| contains_ci(name, needle))
    })
}

/// Case-insensitive substring test (Unicode lower case of both sides).
fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}
