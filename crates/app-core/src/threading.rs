//! Conversation threading (JWZ-lite), pure and incremental.
//!
//! A message joins the thread of any cached message of the same account that
//!
//! 1. has the same `Message-ID` (a copy in another mailbox, e.g. Gmail's All Mail),
//! 2. is named in its `In-Reply-To` / `References` (its ancestors),
//! 3. names the message's `Message-ID` (a reply that arrived first), or
//! 4. shares an ancestor with it (a sibling whose parent is not cached).
//!
//! If these point at several threads, the threads are merged. Without any link, a message whose
//! subject starts with a reply/forward prefix (`Re:`, `Fwd:`, `AW:`, …) joins the most recent
//! thread with the same normalized subject, if that thread was active within
//! [`SUBJECT_WINDOW_MS`]. Everything else starts a new thread. Threads are never split.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{AccountId, ThreadId};
use crate::message::{Envelope, RemoteEnvelope};

/// How close in time a subject-only match must be (30 days).
pub const SUBJECT_WINDOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Reply / forward prefixes in several languages, lower case, without the colon.
const PREFIXES: &[&str] = &[
    "re", "fwd", "fw", "aw", "wg", "sv", "vs", "antw", "rif", "tr", "ref", "odp", "res",
];

/// The headers threading looks at.
#[derive(Debug, Clone, Copy)]
pub struct ThreadKey<'a> {
    /// `Message-ID`.
    pub message_id: Option<&'a str>,
    /// `In-Reply-To`.
    pub in_reply_to: Option<&'a str>,
    /// `References`.
    pub references: &'a [String],
    /// Subject.
    pub subject: &'a str,
    /// Date in Unix milliseconds.
    pub date_ms: i64,
}

impl<'a> From<&'a Envelope> for ThreadKey<'a> {
    fn from(envelope: &'a Envelope) -> Self {
        Self {
            message_id: envelope.message_id_hdr.as_deref(),
            in_reply_to: envelope.in_reply_to.as_deref(),
            references: &envelope.references,
            subject: &envelope.subject,
            date_ms: envelope.date_ms,
        }
    }
}

impl<'a> From<&'a RemoteEnvelope> for ThreadKey<'a> {
    fn from(envelope: &'a RemoteEnvelope) -> Self {
        Self {
            message_id: envelope.message_id_hdr.as_deref(),
            in_reply_to: envelope.in_reply_to.as_deref(),
            references: &envelope.references,
            subject: &envelope.subject,
            date_ms: envelope.date_ms,
        }
    }
}

impl ThreadKey<'_> {
    /// Ancestor IDs: `In-Reply-To` and `References`, nearest first.
    fn ancestors(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::new();
        for id in self
            .in_reply_to
            .into_iter()
            .chain(self.references.iter().rev().map(String::as_str))
        {
            if !id.is_empty() && !ids.contains(&id) && Some(id) != self.message_id {
                ids.push(id);
            }
        }
        ids
    }
}

/// Where a message belongs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// No related thread: start a new one.
    New,
    /// Join an existing thread.
    Join(ThreadId),
    /// The message links several threads: merge `from` into `into`, then join `into`.
    Merge {
        /// The surviving thread.
        into: ThreadId,
        /// Threads to fold into `into`.
        from: Vec<ThreadId>,
    },
}

/// Lookup tables over the cached messages of one account.
#[derive(Debug, Clone, Default)]
pub struct Threader {
    /// `Message-ID` → thread.
    by_message_id: BTreeMap<String, ThreadId>,
    /// ID → threads of messages that name it as an ancestor.
    referrers: BTreeMap<String, BTreeSet<ThreadId>>,
    /// Normalized subject → (thread, newest date) of the most recent thread with it.
    by_subject: BTreeMap<String, (ThreadId, i64)>,
}

impl Threader {
    /// Indexes `envelopes` (those of other accounts are skipped).
    pub fn new<'a>(account: AccountId, envelopes: impl IntoIterator<Item = &'a Envelope>) -> Self {
        let mut threader = Self::default();
        for envelope in envelopes {
            if envelope.account == account {
                threader.add(&ThreadKey::from(envelope), envelope.thread);
            }
        }
        threader
    }

    /// Where a message with these headers belongs. Does not change the index.
    pub fn place(&self, key: &ThreadKey<'_>) -> Placement {
        let mut found: Vec<ThreadId> = Vec::new();
        let mut push = |thread: ThreadId| {
            if !found.contains(&thread) {
                found.push(thread);
            }
        };
        if let Some(id) = key.message_id {
            if let Some(thread) = self.by_message_id.get(id) {
                push(*thread);
            }
            for thread in self.referrers.get(id).into_iter().flatten() {
                push(*thread);
            }
        }
        for id in key.ancestors() {
            if let Some(thread) = self.by_message_id.get(id) {
                push(*thread);
            }
            for thread in self.referrers.get(id).into_iter().flatten() {
                push(*thread);
            }
        }
        match found.split_first() {
            Some((into, [])) => Placement::Join(*into),
            Some((into, rest)) => Placement::Merge {
                into: *into,
                from: rest.to_vec(),
            },
            None => match normalize_subject(key.subject) {
                (subject, true) if !subject.is_empty() => match self.by_subject.get(&subject) {
                    Some((thread, last)) if (key.date_ms - last).abs() <= SUBJECT_WINDOW_MS => {
                        Placement::Join(*thread)
                    }
                    _ => Placement::New,
                },
                _ => Placement::New,
            },
        }
    }

    /// Records that a message with these headers is in `thread`.
    pub fn add(&mut self, key: &ThreadKey<'_>, thread: ThreadId) {
        if let Some(id) = key.message_id
            && !id.is_empty()
        {
            self.by_message_id.entry(id.to_owned()).or_insert(thread);
        }
        for id in key.ancestors() {
            self.referrers
                .entry(id.to_owned())
                .or_default()
                .insert(thread);
        }
        let (subject, _) = normalize_subject(key.subject);
        if !subject.is_empty() {
            let entry = self
                .by_subject
                .entry(subject)
                .or_insert((thread, key.date_ms));
            if key.date_ms >= entry.1 {
                *entry = (thread, key.date_ms);
            }
        }
    }

    /// Rewrites every reference to thread `from` into `into`.
    pub fn merge(&mut self, from: ThreadId, into: ThreadId) {
        for thread in self.by_message_id.values_mut() {
            if *thread == from {
                *thread = into;
            }
        }
        for threads in self.referrers.values_mut() {
            if threads.remove(&from) {
                threads.insert(into);
            }
        }
        for (thread, _) in self.by_subject.values_mut() {
            if *thread == from {
                *thread = into;
            }
        }
    }
}

/// Lower-cased subject without reply/forward prefixes and surrounding whitespace, and whether
/// any such prefix was present. `Re: RE[2]: Fwd: Hello ` → (`hello`, true).
pub fn normalize_subject(subject: &str) -> (String, bool) {
    let mut rest = subject.trim();
    let mut prefixed = false;
    while let Some((head, tail)) = rest.split_once(':') {
        let head = head.trim().to_lowercase();
        // `Re[2]` / `Re(2)` counters.
        let word = head
            .split(['[', '('])
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        let counter_ok = head.len() == word.len()
            || head[word.len()..]
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '[' | ']' | '(' | ')' | ' '));
        if PREFIXES.contains(&word.as_str()) && counter_ok {
            prefixed = true;
            rest = tail.trim_start();
        } else {
            break;
        }
    }
    (rest.trim().to_lowercase(), prefixed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subjects_normalize() {
        assert_eq!(normalize_subject("Hello"), ("hello".to_owned(), false));
        assert_eq!(
            normalize_subject("Re: RE[2]: Fwd:  Hello "),
            ("hello".to_owned(), true)
        );
        assert_eq!(normalize_subject("AW: Termin"), ("termin".to_owned(), true));
        assert_eq!(
            normalize_subject("Agenda: Q3 planning"),
            ("agenda: q3 planning".to_owned(), false)
        );
        assert_eq!(normalize_subject("Re:"), (String::new(), true));
    }
}
