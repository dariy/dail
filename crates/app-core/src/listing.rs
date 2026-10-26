//! Read-only projections of `AppState` shared by the queries and the GUI: mailboxes in display
//! order, the threads of a mailbox or a search, and thread summaries. Pure functions; the GUI
//! renders from these instead of re-deriving the rules.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{AccountId, MailboxId, MessageId, ThreadId};
use crate::mailbox::Mailbox;
use crate::message::{Addr, Envelope, Thread};
use crate::search::Query;
use crate::state::AppState;

/// Someone in the cached mail ([`contacts`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    /// The address, as last seen.
    pub email: String,
    /// The most recent non-empty display name.
    pub name: Option<String>,
    /// Messages they appear in (as sender or recipient).
    pub count: u32,
    /// The newest of those, in Unix milliseconds.
    pub last_ms: i64,
    /// Whether one of your messages was addressed to them.
    pub written_to: bool,
}

impl Contact {
    /// Whether `query` (lower case) is in the name or the address.
    pub fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || self.email.to_lowercase().contains(query)
            || self
                .name
                .as_ref()
                .is_some_and(|n| n.to_lowercase().contains(query))
    }

    /// As an address for a draft.
    pub fn addr(&self) -> Addr {
        Addr {
            name: self.name.clone(),
            email: self.email.clone(),
        }
    }
}

/// Everyone in the cached mail but the accounts' own addresses: the people you wrote to first,
/// then by how often and how recently they appear. With `query`, only those whose name or address
/// contains it (ignoring case).
pub fn contacts(state: &AppState, query: &str, limit: usize) -> Vec<Contact> {
    let own: BTreeSet<String> = state
        .accounts
        .values()
        .map(|a| a.email.to_lowercase())
        .collect();
    let mut by_email: BTreeMap<String, Contact> = BTreeMap::new();
    for message in state.messages.values() {
        let from_me = message
            .from
            .iter()
            .any(|a| own.contains(&a.email.to_lowercase()));
        let people = message
            .from
            .iter()
            .map(|a| (a, false))
            .chain(message.to.iter().chain(&message.cc).map(|a| (a, from_me)));
        for (addr, written) in people {
            let key = addr.email.trim().to_lowercase();
            if key.is_empty() || own.contains(&key) {
                continue;
            }
            let entry = by_email.entry(key).or_insert_with(|| Contact {
                email: addr.email.trim().to_owned(),
                name: None,
                count: 0,
                last_ms: i64::MIN,
                written_to: false,
            });
            entry.count = entry.count.saturating_add(1);
            entry.written_to |= written;
            if message.date_ms >= entry.last_ms {
                entry.last_ms = message.date_ms;
                if let Some(name) = addr.name.as_deref().filter(|n| !n.trim().is_empty()) {
                    entry.name = Some(name.trim().to_owned());
                }
            }
            if entry.name.is_none() {
                entry.name = addr.name.clone().filter(|n| !n.trim().is_empty());
            }
        }
    }
    let query = query.trim().to_lowercase();
    let mut found: Vec<Contact> = by_email
        .into_values()
        .filter(|c| c.matches(&query))
        .collect();
    found.sort_by(|a, b| {
        b.written_to
            .cmp(&a.written_to)
            .then(b.count.cmp(&a.count))
            .then(b.last_ms.cmp(&a.last_ms))
            .then_with(|| a.email.cmp(&b.email))
    });
    found.truncate(limit);
    found
}

/// How much of a mailbox's history is cached (`set_sync_window`, the backfill).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct History {
    /// Messages cached (placeholders aside).
    pub cached: u32,
    /// Messages the window asks for: `min(window, total)`.
    pub wanted: u32,
    /// The oldest cached UID.
    pub oldest_uid: Option<u32>,
    /// The effective window (`u32::MAX`: all).
    pub window: u32,
}

/// The history of `mailbox`, if it is known.
pub fn history(state: &AppState, mailbox: MailboxId) -> Option<History> {
    let mb = state.mailboxes.get(&mailbox)?;
    let (cached, oldest_uid) = state
        .messages
        .values()
        .filter(|m| m.mailbox == mailbox && !m.is_placeholder())
        .fold((0_u32, None::<u32>), |(n, oldest), m| {
            (
                n.saturating_add(1),
                Some(oldest.map_or(m.uid, |o| o.min(m.uid))),
            )
        });
    let window = match mb.sync_window {
        Some(0) => u32::MAX,
        Some(n) => n,
        None => u32::try_from(state.settings.initial_window).unwrap_or(u32::MAX),
    };
    let wanted = if mb.history_complete {
        cached
    } else {
        window.min(mb.total)
    };
    Some(History {
        cached,
        wanted,
        oldest_uid,
        window,
    })
}

/// Whether the reader shows `message` (of the open `thread`) expanded: as the user set it
/// (`expand_messages`, `collapse_messages`), else if it is unread or the newest.
pub fn is_expanded(state: &AppState, thread: &Thread, message: &Envelope) -> bool {
    state
        .view
        .expanded
        .get(&message.id)
        .copied()
        .unwrap_or_else(|| !message.flags.seen || thread.messages.last() == Some(&message.id))
}

/// Mailboxes grouped by account (in account order), the inbox and other roles first, then by
/// path.
pub fn mailboxes(state: &AppState, account: Option<AccountId>) -> Vec<&Mailbox> {
    let mut mailboxes: Vec<&Mailbox> = state
        .mailboxes
        .values()
        .filter(|m| account.is_none_or(|id| m.account == id))
        .collect();
    mailboxes.sort_by_key(|m| {
        (
            state.accounts.get_index_of(&m.account),
            m.role.map_or(usize::MAX, |role| role.rank()),
            m.path.to_lowercase(),
        )
    });
    mailboxes
}

/// The threads with a message shown in `mailbox`, newest first.
pub fn threads_in(state: &AppState, mailbox: MailboxId) -> Vec<&Thread> {
    let ids: BTreeSet<ThreadId> = state
        .messages
        .values()
        .filter(|m| m.location() == mailbox)
        .map(|m| m.thread)
        .collect();
    let mut threads: Vec<&Thread> = ids.iter().filter_map(|id| state.threads.get(id)).collect();
    sort_newest_first(&mut threads, |t| t);
    threads
}

/// The threads matching `query` (cached envelopes, plus the hits of server searches for the
/// same query), newest first, each with its matching messages in thread order.
pub fn search<'a>(
    state: &'a AppState,
    query: &Query,
    account: Option<AccountId>,
) -> Vec<(&'a Thread, Vec<MessageId>)> {
    let key = search_key(query);
    let server_hits: BTreeSet<MessageId> = state
        .status
        .searches
        .iter()
        .filter(|s| s.query == key && account.is_none_or(|a| a == s.account))
        .flat_map(|s| s.hits.iter().copied())
        // The local full-text index matched the text terms too (in downloaded bodies).
        .chain(
            state
                .status
                .index_searches
                .iter()
                .filter(|s| s.query == key)
                .flat_map(|s| s.hits.iter().copied()),
        )
        .collect();
    let mut matches: BTreeMap<ThreadId, Vec<MessageId>> = BTreeMap::new();
    for message in state.messages.values() {
        if account.is_some_and(|id| message.account != id) {
            continue;
        }
        let shown_in = state.mailboxes.get(&message.location());
        let found = query.matches(message, shown_in)
            || (server_hits.contains(&message.id) && query.matches_server_hit(message, shown_in));
        if found {
            matches.entry(message.thread).or_default().push(message.id);
        }
    }
    let mut threads: Vec<(&Thread, Vec<MessageId>)> = matches
        .into_iter()
        .filter_map(|(id, messages)| {
            let thread = state.threads.get(&id)?;
            // Matching messages in thread order (oldest first).
            let ordered = thread
                .messages
                .iter()
                .filter(|m| messages.contains(m))
                .copied()
                .collect();
            Some((thread, ordered))
        })
        .collect();
    sort_newest_first(&mut threads, |(t, _)| t);
    threads
}

/// Reorders search results (`search` output): threads with a full-text hit for `query` first,
/// in the order of their best hit (the index's score order), then the others as they were.
pub fn by_relevance(state: &AppState, query: &Query, threads: &mut [(&Thread, Vec<MessageId>)]) {
    let key = search_key(query);
    let rank: BTreeMap<MessageId, usize> = state
        .status
        .index_searches
        .iter()
        .filter(|s| s.query == key)
        .flat_map(|s| s.hits.iter().enumerate().map(|(i, id)| (*id, i)))
        .collect();
    // A stable sort keeps the newest-first order among equals.
    threads.sort_by_key(|(_, matches)| {
        matches
            .iter()
            .filter_map(|m| rank.get(m))
            .min()
            .copied()
            .unwrap_or(usize::MAX)
    });
}

/// What the thread list shows: the results of `view.search` if set (nothing if it doesn't
/// parse), else every inbox's threads if `view.unified`, else the threads of `view.mailbox`;
/// only those that pass `view.filter` (`set_list_filter`).
pub fn visible_threads(state: &AppState) -> Vec<ThreadId> {
    let mut ids = listed_threads(state);
    let filter = state.view.filter;
    if !filter.is_off() {
        ids.retain(|id| {
            state.threads.get(id).is_some_and(|thread| {
                let messages = || thread.messages.iter().filter_map(|m| state.messages.get(m));
                (!filter.unread || messages().any(|m| !m.flags.seen))
                    && (!filter.starred || messages().any(|m| m.flags.flagged))
                    && (!filter.attachments || messages().any(|m| m.has_attachments))
            })
        });
    }
    ids
}

/// [`visible_threads`] before the filter.
fn listed_threads(state: &AppState) -> Vec<ThreadId> {
    if let Some(text) = &state.view.search {
        return Query::parse(text)
            .map(|query| {
                search(state, &query, None)
                    .into_iter()
                    .map(|(t, _)| t.id)
                    .collect()
            })
            .unwrap_or_default();
    }
    if state.view.unified {
        return unified_threads(state).iter().map(|t| t.id).collect();
    }
    state
        .view
        .mailbox
        .map(|mailbox| threads_in(state, mailbox).iter().map(|t| t.id).collect())
        .unwrap_or_default()
}

/// The threads of every account's inbox, newest first (the unified inbox).
pub fn unified_threads(state: &AppState) -> Vec<&Thread> {
    let inboxes: BTreeSet<MailboxId> = state
        .mailboxes
        .values()
        .filter(|m| m.role == Some(crate::mailbox::MailboxRole::Inbox))
        .map(|m| m.id)
        .collect();
    let ids: BTreeSet<ThreadId> = state
        .messages
        .values()
        .filter(|m| inboxes.contains(&m.location()))
        .map(|m| m.thread)
        .collect();
    let mut threads: Vec<&Thread> = ids.iter().filter_map(|id| state.threads.get(id)).collect();
    sort_newest_first(&mut threads, |t| t);
    threads
}

/// Identifies a server search: the canonical query, lower-cased (every comparison ignores
/// case, on the server too).
pub fn search_key(query: &Query) -> String {
    query.to_string().to_lowercase()
}

/// One row of a thread list.
#[derive(Debug, Clone)]
pub struct ThreadSummary<'a> {
    /// The thread.
    pub thread: &'a Thread,
    /// Its cached messages, oldest first.
    pub messages: Vec<&'a Envelope>,
    /// Distinct senders, in order of first message.
    pub senders: Vec<&'a Addr>,
    /// Unread messages.
    pub unread: usize,
    /// Whether a message is starred.
    pub flagged: bool,
    /// Whether a message has attachments.
    pub has_attachments: bool,
}

impl<'a> ThreadSummary<'a> {
    /// Summarizes `thread`.
    pub fn of(state: &'a AppState, thread: &'a Thread) -> Self {
        let messages: Vec<&Envelope> = thread
            .messages
            .iter()
            .filter_map(|id| state.messages.get(id))
            .collect();
        let mut senders: Vec<&Addr> = Vec::new();
        for addr in messages.iter().filter_map(|m| m.from.first()) {
            if !senders
                .iter()
                .any(|known| known.email.eq_ignore_ascii_case(&addr.email))
            {
                senders.push(addr);
            }
        }
        Self {
            thread,
            unread: messages.iter().filter(|m| !m.flags.seen).count(),
            flagged: messages.iter().any(|m| m.flags.flagged),
            has_attachments: messages.iter().any(|m| m.has_attachments),
            senders,
            messages,
        }
    }

    /// Messages shown in `mailbox`.
    pub fn in_mailbox(&self, mailbox: MailboxId) -> usize {
        self.messages
            .iter()
            .filter(|m| m.location() == mailbox)
            .count()
    }
}

fn sort_newest_first<T>(items: &mut [T], thread: impl Fn(&T) -> &Thread) {
    items.sort_by(|a, b| {
        let (a, b) = (thread(a), thread(b));
        b.last_date_ms
            .cmp(&a.last_date_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
}
