//! Search: `search` (Query) and `set_search` / `clear_search` (View) over the cached envelopes,
//! plus the server fallback (`ARCHITECTURE.md §5.2`). The query language lives in
//! [`crate::search`].
//!
//! The cache holds only recent mail and no body text, so searches also go to the servers
//! (`SearchMailbox`, one per mailbox). Their hits are merged into the cache like fetched
//! envelopes and remembered per query and account in `status.searches`, so that later searches
//! show them even when the match was in a body.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::{AccountId, MailboxId, MessageId};
use crate::listing::{self, search_key};
use crate::mailbox::{Mailbox, MailboxRole};
use crate::message::RemoteEnvelope;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::search::Query;
use crate::state::{IndexSearch, ServerSearch};

use super::queries::MAX_THREAD_PAGE;
use super::sync::{CacheChanges, MergeTarget, push_unique};
use super::{Engine, invalid, not_found};

/// Page size of the results `set_search` returns.
const SET_SEARCH_PAGE: u32 = 50;

/// Hits a server returns per mailbox (the newest).
pub const SERVER_SEARCH_LIMIT: u32 = 200;

/// Server searches remembered in `status.searches`.
pub const MAX_SERVER_SEARCHES: usize = 8;

/// How long a finished server search is reused by `search` before it is repeated.
pub const SERVER_SEARCH_FRESH_MS: i64 = 60_000;

/// Arguments of `search`.
pub(super) struct SearchRequest<'a> {
    pub query: &'a str,
    pub account: Option<AccountId>,
    pub offset: u32,
    pub limit: u32,
    pub server: bool,
    pub order: crate::search::SearchOrder,
}

/// Which search round a `search_results` / `search_failed` event reports on.
pub(super) struct SearchRound {
    pub account: AccountId,
    pub mailbox: MailboxId,
    pub search: String,
    pub started_ms: i64,
    pub mailboxes: u32,
}

/// What the server answered: `(uid_validity, envelopes)` or the error.
pub(super) type SearchAnswer = Result<(u32, Vec<RemoteEnvelope>), String>;

impl Engine {
    pub(super) fn search(
        &self,
        request: SearchRequest<'_>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if request.limit == 0 || request.limit > MAX_THREAD_PAGE {
            return Err(invalid(
                "limit",
                format!("must be between 1 and {MAX_THREAD_PAGE}"),
            ));
        }
        if let Some(id) = request.account
            && !self.state.accounts.contains_key(&id)
        {
            return Err(not_found("account", id));
        }
        let parsed = parse_query(request.query)?;
        let key = search_key(&parsed);
        let mut receipt = receipt;
        if let Some(id) = request.account {
            receipt = receipt.affecting(EntityRef::Account(id));
        }
        let mut data = self.search_results(
            &parsed,
            request.account,
            (request.offset, request.limit),
            request.order,
        );
        let mut effects: Vec<Effect> = index_search_effect(&parsed, &key).into_iter().collect();
        if request.server {
            // A Query changes nothing: the round is recorded when its results arrive.
            let mut server = Vec::new();
            for account in self.accounts_in(request.account) {
                let entry = self.server_search(&key, account);
                let reusable = entry.is_some_and(|e| {
                    e.pending > 0 || ctx.now_ms - e.updated_ms < SERVER_SEARCH_FRESH_MS
                });
                if let (true, Some(entry)) = (reusable, entry) {
                    server.push(server_json(entry));
                    continue;
                }
                let round = self.server_search_effects(&parsed, account, ctx.now_ms);
                if !round.is_empty() {
                    server.push(json!({ "account": account, "state": "started" }));
                    effects.extend(round);
                }
            }
            insert(&mut data, "server", Value::from(server));
        }
        Ok(Outcome {
            receipt: receipt.with_data(data),
            effects,
        })
    }

    pub(super) fn set_search(
        &mut self,
        query: String,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let parsed = parse_query(&query)?;
        let key = search_key(&parsed);
        let mut effects: Vec<Effect> = index_search_effect(&parsed, &key).into_iter().collect();
        for account in self.accounts_in(None) {
            let round = self.server_search_effects(&parsed, account, ctx.now_ms);
            if round.is_empty() {
                continue;
            }
            let pending = u32::try_from(round.len()).unwrap_or(u32::MAX);
            self.remember_search(ServerSearch {
                query: key.clone(),
                account,
                pending,
                hits: Vec::new(),
                last_error: None,
                updated_ms: ctx.now_ms,
            });
            effects.extend(round);
        }
        let mut data = self.search_results(
            &parsed,
            None,
            (0, SET_SEARCH_PAGE),
            crate::search::SearchOrder::Newest,
        );
        let query = query.trim().to_owned();
        insert(&mut data, "search", Value::from(query.clone()));
        let server: Vec<Value> = self
            .state
            .status
            .searches
            .iter()
            .filter(|s| s.query == key && s.pending > 0)
            .map(server_json)
            .collect();
        insert(&mut data, "server", Value::from(server));
        let view = &mut self.state.view;
        view.search = Some(query);
        view.selected.clear();
        view.checked.clear();
        view.thread = None;
        let mut receipt = receipt.affecting(EntityRef::View);
        if !effects.is_empty() {
            receipt = receipt.affecting(EntityRef::Status);
        }
        Ok(Outcome {
            receipt: receipt.with_data(data),
            effects,
        })
    }

    /// Records what the full-text index found for `search`.
    pub(super) fn ingest_index_search(
        &mut self,
        search: String,
        answer: Result<Vec<MessageId>, String>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let (hits, error) = match answer {
            Ok(hits) => (hits, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let found = hits.len();
        let searches = &mut self.state.status.index_searches;
        searches.retain(|s| s.query != search);
        searches.push(IndexSearch {
            query: search.clone(),
            hits,
            error: error.clone(),
            updated_ms: ctx.now_ms,
        });
        if searches.len() > MAX_SERVER_SEARCHES {
            let excess = searches.len() - MAX_SERVER_SEARCHES;
            searches.drain(..excess);
        }
        Outcome::new(receipt.affecting(EntityRef::Status).with_data(json!({
            "search": search,
            "hits": found,
            "error": error,
        })))
    }

    pub(super) fn clear_search(&mut self, receipt: CommandReceipt) -> Outcome {
        let cleared = self.state.view.search.take();
        self.state.view.checked.clear();
        Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .with_data(json!({ "cleared": cleared })),
        )
    }

    /// Merges a mailbox's search hits into the cache (like fetched envelopes: known UIDs get
    /// their flags, placeholders are located, the rest are new threaded messages) and records
    /// them in the account's server search for the query.
    pub(super) fn ingest_search_results(
        &mut self,
        report: SearchRound,
        answer: SearchAnswer,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let mut receipt = receipt.affecting(EntityRef::Status);
        let mut changes = CacheChanges::default();
        let mut hits = Vec::new();
        let mut added = 0_usize;
        let mut error = None;
        let mut op_effects = Vec::new();
        let mailbox = self
            .state
            .mailboxes
            .get(&report.mailbox)
            .filter(|m| m.account == report.account)
            .cloned();
        match (answer, mailbox) {
            (Err(failure), _) => error = Some(failure),
            (Ok(_), None) => {}
            (Ok((uid_validity, _)), Some(mailbox))
                if mailbox.uid_validity != Some(uid_validity) =>
            {
                error = Some(format!(
                    "{} changed on the server; it is re-synced, search again afterwards",
                    mailbox.path
                ));
            }
            (Ok((_, envelopes)), Some(mailbox)) => {
                let by_uid: BTreeMap<u32, MessageId> = self
                    .state
                    .messages
                    .values()
                    .filter(|m| m.mailbox == mailbox.id && !m.is_placeholder())
                    .map(|m| (m.uid, m.id))
                    .collect();
                let mut target = MergeTarget {
                    account: report.account,
                    mailbox: mailbox.id,
                    placeholders: super::sync::placeholders_in(&self.state, mailbox.id),
                    by_uid,
                    threader: self.take_threader(report.account),
                    ids: ctx.ids(),
                };
                for remote in envelopes {
                    let (id, merged) = self.merge_remote(remote, &mut target, ctx, &mut changes);
                    if merged == super::sync::Merged::Added {
                        added += 1;
                    }
                    push_unique(&mut hits, id);
                }
                self.keep_threader(report.account, std::mem::take(&mut target.threader));
                // The server answered: ops waiting for it can run now.
                self.retry_now(report.account, &mut changes);
                op_effects = self.pump_ops(report.account, ctx.now_ms, &mut changes);
                receipt = receipt.affecting(EntityRef::Mailbox(mailbox.id));
            }
        }
        self.record_round(&report, ctx.now_ms, &hits, error.clone());
        let delta = self.delta(changes);
        for thread in &delta.threads {
            receipt = receipt.affecting(EntityRef::Thread(thread.id));
        }
        let mut outcome = Outcome::new(receipt.with_data(json!({
            "search": report.search,
            "mailbox": report.mailbox,
            "hits": hits.len(),
            "added": added,
            "error": error,
        })));
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome.effects.extend(op_effects);
        outcome
    }

    /// Records one mailbox's report in the account's server search for `key`. A report of a
    /// newer round than the remembered one starts that round over.
    fn record_round(
        &mut self,
        round: &SearchRound,
        now_ms: i64,
        hits: &[MessageId],
        error: Option<String>,
    ) {
        let (key, account) = (round.search.as_str(), round.account);
        let (started_ms, mailboxes) = (round.started_ms, round.mailboxes);
        let index = self
            .state
            .status
            .searches
            .iter()
            .position(|s| s.query == key && s.account == account);
        let mut entry = match index {
            Some(i) => self.state.status.searches.remove(i),
            None => ServerSearch {
                query: key.to_owned(),
                account,
                pending: mailboxes,
                hits: Vec::new(),
                last_error: None,
                updated_ms: started_ms,
            },
        };
        if started_ms > entry.updated_ms && index.is_some() && entry.pending == 0 {
            // A later round (started by `search`, which records nothing up front).
            entry.pending = mailboxes;
            entry.last_error = None;
        }
        entry.pending = entry.pending.saturating_sub(1);
        entry.updated_ms = now_ms;
        for id in hits {
            push_unique(&mut entry.hits, *id);
        }
        if error.is_some() {
            entry.last_error = error;
        }
        self.remember_search(entry);
    }

    /// Stores a server search as the most recent one (replacing the same query and account),
    /// forgetting the oldest beyond [`MAX_SERVER_SEARCHES`].
    fn remember_search(&mut self, entry: ServerSearch) {
        let searches = &mut self.state.status.searches;
        searches.retain(|s| !(s.query == entry.query && s.account == entry.account));
        searches.push(entry);
        if searches.len() > MAX_SERVER_SEARCHES {
            let excess = searches.len() - MAX_SERVER_SEARCHES;
            searches.drain(..excess);
        }
    }

    fn server_search(&self, key: &str, account: AccountId) -> Option<&ServerSearch> {
        self.state
            .status
            .searches
            .iter()
            .find(|s| s.query == key && s.account == account)
    }

    /// One account, or every account in order.
    fn accounts_in(&self, account: Option<AccountId>) -> Vec<AccountId> {
        self.state
            .accounts
            .keys()
            .copied()
            .filter(|id| account.is_none_or(|a| a == *id))
            .collect()
    }

    /// `SearchMailbox` effects for one account: the mailboxes the query's `in:` terms name, or
    /// else the `all` mailbox if there is one (Gmail: All Mail), or else every mailbox except
    /// Trash and Junk. Only synced mailboxes are searched. Nothing if only `in:` terms remain.
    fn server_search_effects(&self, query: &Query, account: AccountId, now_ms: i64) -> Vec<Effect> {
        let Some(owner) = self.state.accounts.get(&account) else {
            return Vec::new();
        };
        let remote = query.without_mailboxes();
        if remote.is_empty() {
            return Vec::new();
        }
        let candidates: Vec<&Mailbox> = self
            .state
            .mailboxes
            .values()
            .filter(|m| m.account == account && m.uid_validity.is_some())
            .filter(|m| query.selects_mailbox(m))
            .collect();
        let chosen: Vec<&Mailbox> = if query.names_mailbox() {
            candidates
        } else if let Some(all) = candidates.iter().find(|m| m.role == Some(MailboxRole::All)) {
            vec![*all]
        } else {
            candidates
                .into_iter()
                .filter(|m| !matches!(m.role, Some(MailboxRole::Trash | MailboxRole::Junk)))
                .collect()
        };
        let mailboxes = u32::try_from(chosen.len()).unwrap_or(u32::MAX);
        let key = search_key(query);
        chosen
            .into_iter()
            .map(|mailbox| Effect::SearchMailbox {
                account,
                mailbox: mailbox.id,
                path: mailbox.path.clone(),
                imap: owner.imap_login(),
                search: key.clone(),
                started_ms: now_ms,
                mailboxes,
                query: remote.clone(),
                gmail_raw: owner.preset.profile().raw_search,
                limit: SERVER_SEARCH_LIMIT,
            })
            .collect()
    }

    /// The threads with a message matching `query` (or found by the server for it), newest
    /// first, one page of them.
    fn search_results(
        &self,
        query: &Query,
        account: Option<AccountId>,
        (offset, limit): (u32, u32),
        order: crate::search::SearchOrder,
    ) -> Value {
        let mut threads = listing::search(&self.state, query, account);
        if order == crate::search::SearchOrder::Relevance {
            listing::by_relevance(&self.state, query, &mut threads);
        }
        let total = threads.len();
        let skip = usize::try_from(offset).unwrap_or(usize::MAX);
        let take = usize::try_from(limit).unwrap_or(usize::MAX);
        let page: Vec<Value> = threads
            .into_iter()
            .skip(skip)
            .take(take)
            .map(|(thread, matches)| {
                let mut summary = self.thread_summary(thread, None);
                insert(&mut summary, "matches", json!(matches));
                summary
            })
            .collect();
        json!({
            "query": query.to_string(),
            "total": total,
            "offset": offset,
            "limit": limit,
            "threads": page,
        })
    }
}

/// A server search as reported in receipts.
fn server_json(entry: &ServerSearch) -> Value {
    let state = if entry.pending > 0 {
        "searching"
    } else {
        "done"
    };
    json!({
        "account": entry.account,
        "state": state,
        "hits": entry.hits.len(),
        "error": entry.last_error,
    })
}

fn insert(object: &mut Value, key: &str, value: Value) {
    if let Some(object) = object.as_object_mut() {
        object.insert(key.to_owned(), value);
    }
}

/// Parses a non-empty query; errors name the `query` argument.
fn parse_query(query: &str) -> Result<Query, CoreError> {
    let parsed = Query::parse(query).map_err(|err| invalid("query", err.to_string()))?;
    if parsed.is_empty() {
        return Err(invalid(
            "query",
            "is empty; give at least one term, e.g. `is:unread`",
        ));
    }
    Ok(parsed)
}

/// Hits asked of the full-text index per search.
pub const INDEX_SEARCH_LIMIT: u32 = 500;

/// The full-text index search for `query`'s text terms, if it has any (negated ones aside).
fn index_search_effect(query: &Query, key: &str) -> Option<Effect> {
    let words: Vec<String> = query
        .terms
        .iter()
        .filter(|t| !t.negated)
        .filter_map(|t| match &t.condition {
            crate::search::Condition::Text(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    (!words.is_empty()).then(|| Effect::SearchIndex {
        search: key.to_owned(),
        words,
        limit: INDEX_SEARCH_LIMIT,
    })
}
