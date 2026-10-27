//! View commands: `open_mailbox`, `select_threads`, `open_thread`, `close_thread`,
//! `expand_messages`, `collapse_messages`, `open_composer`, `close_composer`, `focus_pane`,
//! `toggle_sidebar`. They change only
//! `ViewState` (opening a thread also starts body downloads, tracked in
//! `status.bodies_in_flight`).

use crate::error::CoreError;
use crate::ids::{AccountId, DraftId, MailboxId, MessageId, ThreadId};
use crate::mailbox::{FolderKey, MailboxRole};
use crate::message::BodyState;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::{FocusTarget, Pane};

use super::{Engine, invalid, not_found};

impl Engine {
    pub(super) fn open_mailbox(
        &mut self,
        id: MailboxId,
        now_ms: i64,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let Some(mailbox) = self.state.mailboxes.get(&id) else {
            return Err(not_found("mailbox", id));
        };
        // Explicit navigation out of the focus ends it.
        let inside = match self.state.view.focused {
            Some(FocusTarget::Mailbox(focus)) => {
                self.state.mailboxes.get(&focus).is_some_and(|f| {
                    f.id == id
                        || (f.account == mailbox.account
                            && mailbox
                                .path
                                .strip_prefix(f.path.as_str())
                                .is_some_and(|rest| rest.starts_with(f.separator())))
                })
            }
            Some(FocusTarget::Account(account)) => mailbox.account == account,
            None => true,
        };
        let view = &mut self.state.view;
        if !inside {
            view.focused = None;
        }
        view.mailbox = Some(id);
        view.unified = false;
        view.selected.clear();
        view.checked.clear();
        view.thread = None;
        // Refresh on open (UX-6): fetch what is new when the last fetch is stale.
        let stale_ms = i64::try_from(self.state.settings.stale_secs)
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        let stale = self.state.mailboxes.get(&id).is_some_and(|m| {
            m.last_fetch_ms
                .is_none_or(|at| now_ms.saturating_sub(at) >= stale_ms)
        });
        let fetch = if stale { self.refresh(id) } else { None };
        let mut receipt = receipt
            .affecting(EntityRef::View)
            .affecting(EntityRef::Mailbox(id));
        if fetch.is_some() {
            receipt = receipt.affecting(EntityRef::Status);
        }
        Ok(Outcome {
            receipt: receipt
                .with_data(serde_json::json!({ "mailbox": id, "fetching": fetch.is_some() })),
            effects: fetch.into_iter().collect(),
        })
    }

    pub(super) fn select_threads(
        &mut self,
        ids: Vec<ThreadId>,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let mut selected: Vec<ThreadId> = Vec::with_capacity(ids.len());
        for id in ids {
            if !self.state.threads.contains_key(&id) {
                return Err(not_found("thread", id));
            }
            if !selected.contains(&id) {
                selected.push(id);
            }
        }
        let mut receipt = receipt.affecting(EntityRef::View);
        for id in &selected {
            receipt = receipt.affecting(EntityRef::Thread(*id));
        }
        let data = serde_json::json!({ "selected": selected });
        self.state.view.selected = selected;
        Ok(Outcome::new(receipt.with_data(data)))
    }

    pub(super) fn open_unified_inbox(&mut self, receipt: CommandReceipt) -> Outcome {
        let view = &mut self.state.view;
        view.focused = None;
        view.unified = true;
        view.mailbox = None;
        view.selected.clear();
        view.checked.clear();
        view.thread = None;
        Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .with_data(serde_json::json!({
                    "threads": crate::listing::unified_threads(&self.state).len(),
                })),
        )
    }

    pub(super) fn check_threads(
        &mut self,
        ids: Vec<ThreadId>,
        check: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if let Some(missing) = ids.iter().find(|id| !self.state.threads.contains_key(*id)) {
            return Err(not_found("thread", *missing));
        }
        let ids = match (ids.is_empty(), check) {
            (true, true) => crate::listing::visible_threads(&self.state),
            (true, false) => std::mem::take(&mut self.state.view.checked),
            (false, _) => ids,
        };
        let checked = &mut self.state.view.checked;
        for id in &ids {
            if check {
                if !checked.contains(id) {
                    checked.push(*id);
                }
            } else {
                checked.retain(|c| c != id);
            }
        }
        let data = serde_json::json!({ "checked": checked.len() });
        Ok(Outcome::new(
            receipt.affecting(EntityRef::View).with_data(data),
        ))
    }

    pub(super) fn open_thread(
        &mut self,
        id: ThreadId,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let thread = self
            .state
            .threads
            .get(&id)
            .ok_or_else(|| not_found("thread", id))?;
        let missing: Vec<_> = thread
            .messages
            .iter()
            .filter(|m| {
                self.state
                    .messages
                    .get(*m)
                    .is_some_and(|e| !matches!(e.body, BodyState::Fetched { .. }))
            })
            .copied()
            .collect();
        let mut effects = Vec::new();
        let mut fetching = Vec::new();
        for message in missing {
            if let Some(effect) = self.start_body_fetch(message) {
                effects.push(effect);
                fetching.push(message);
            }
        }
        let view = &mut self.state.view;
        if view.thread != Some(id) {
            view.expanded.clear();
        }
        view.thread = Some(id);
        if !view.selected.contains(&id) {
            view.selected = vec![id];
        }
        let mut receipt = receipt
            .affecting(EntityRef::View)
            .affecting(EntityRef::Thread(id));
        if !fetching.is_empty() {
            receipt = receipt.affecting(EntityRef::Status);
        }
        Ok(Outcome {
            receipt: receipt.with_data(serde_json::json!({
                "thread": id,
                "fetching_bodies": fetching,
            })),
            effects,
        })
    }

    pub(super) fn close_thread(&mut self, receipt: CommandReceipt) -> Outcome {
        self.state.view.thread = None;
        self.state.view.expanded.clear();
        Outcome::new(receipt.affecting(EntityRef::View))
    }

    pub(super) fn set_expanded(
        &mut self,
        ids: Vec<MessageId>,
        expanded: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let Some(thread) = self
            .state
            .view
            .thread
            .and_then(|id| self.state.threads.get(&id))
        else {
            return Err(invalid("ids", "no conversation is open (open_thread)"));
        };
        let ids = if ids.is_empty() {
            thread.messages.clone()
        } else {
            if let Some(stray) = ids.iter().find(|id| !thread.messages.contains(id)) {
                return Err(invalid(
                    "ids",
                    format!("message {stray} is not in the open conversation"),
                ));
            }
            ids
        };
        let thread = thread.id;
        for id in &ids {
            self.state.view.expanded.insert(*id, expanded);
        }
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .affecting(EntityRef::Thread(thread))
                .with_data(serde_json::json!({ "messages": ids, "expanded": expanded })),
        ))
    }

    pub(super) fn open_composer(
        &mut self,
        id: DraftId,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !self.state.drafts.contains_key(&id) {
            return Err(not_found("draft", id));
        }
        let view = &mut self.state.view;
        view.composer = Some(id);
        view.focus = Pane::Composer;
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .affecting(EntityRef::Draft(id))
                .with_data(serde_json::json!({ "composer": id })),
        ))
    }

    pub(super) fn close_composer(&mut self, receipt: CommandReceipt) -> Outcome {
        let closed = self.state.view.composer.take();
        Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .with_data(serde_json::json!({ "closed": closed })),
        )
    }

    pub(super) fn focus_pane(
        &mut self,
        pane: Pane,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let view = &mut self.state.view;
        if pane == Pane::Sidebar {
            view.sidebar_collapsed = false;
        }
        if !view.can_focus(pane) {
            let reason = match pane {
                Pane::Reader => "no thread is open (open_thread)",
                Pane::Composer => "no draft is open (open_composer)",
                Pane::Sidebar | Pane::List => "the pane is not shown",
            };
            return Err(invalid("pane", reason));
        }
        view.focus = pane;
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .with_data(serde_json::json!({ "focus": pane })),
        ))
    }

    /// `collapse_folders` / `expand_folders`: adds the keys to `view.collapsed` or takes them
    /// out. Without keys, every account and every folder with folders below it / all of them.
    pub(super) fn set_collapsed(
        &mut self,
        keys: Vec<FolderKey>,
        collapse: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        for key in &keys {
            if !self.state.accounts.contains_key(&key.account) {
                return Err(not_found("account", key.account));
            }
            let known = key.path.is_empty()
                || self.state.mailboxes.values().any(|m| {
                    m.account == key.account
                        && (m.path == key.path
                            || m.path
                                .strip_prefix(key.path.as_str())
                                .is_some_and(|rest| rest.starts_with(m.separator())))
                });
            if !known {
                return Err(invalid(
                    "keys",
                    format!("account {} has no folder {:?}", key.account, key.path),
                ));
            }
        }
        let keys: Vec<FolderKey> = if keys.is_empty() && collapse {
            let mut all: Vec<FolderKey> = self
                .state
                .accounts
                .keys()
                .map(|account| FolderKey {
                    account: *account,
                    path: String::new(),
                })
                .collect();
            for mailbox in self.state.mailboxes.values() {
                let mut path = mailbox.parent_path();
                while let Some(parent) = path {
                    all.push(FolderKey {
                        account: mailbox.account,
                        path: parent.to_owned(),
                    });
                    path = parent
                        .rsplit_once(mailbox.separator())
                        .map(|(above, _)| above)
                        .filter(|above| !above.is_empty());
                }
            }
            all
        } else if keys.is_empty() {
            self.state.view.collapsed.iter().cloned().collect()
        } else {
            keys
        };
        let collapsed = &mut self.state.view.collapsed;
        let mut changed = 0_usize;
        for key in keys {
            let did = if collapse {
                collapsed.insert(key)
            } else {
                collapsed.remove(&key)
            };
            changed += usize::from(did);
        }
        Ok(Outcome::new(receipt.affecting(EntityRef::View).with_data(
            serde_json::json!({ "changed": changed, "collapsed": collapsed.len() }),
        )))
    }

    /// `focus_mailbox`: narrows the sidebar to a mailbox or an account and opens it.
    pub(super) fn focus_mailbox(
        &mut self,
        id: Option<MailboxId>,
        account: Option<AccountId>,
        now_ms: i64,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let (target, open) = match (id, account) {
            (Some(id), None) => {
                if !self.state.mailboxes.contains_key(&id) {
                    return Err(not_found("mailbox", id));
                }
                (FocusTarget::Mailbox(id), Some(id))
            }
            (None, Some(account)) => {
                if !self.state.accounts.contains_key(&account) {
                    return Err(not_found("account", account));
                }
                let current = self
                    .state
                    .view
                    .mailbox
                    .and_then(|m| self.state.mailboxes.get(&m))
                    .filter(|m| m.account == account && !self.state.view.unified);
                let open = match current {
                    Some(_) => None,
                    None => self
                        .state
                        .mailboxes
                        .values()
                        .find(|m| m.account == account && m.role == Some(MailboxRole::Inbox))
                        .map(|m| m.id),
                };
                (FocusTarget::Account(account), open)
            }
            _ => return Err(invalid("id", "give either id or account")),
        };
        let mut receipt = receipt;
        let mut effects = Vec::new();
        if let Some(open) = open
            && (self.state.view.mailbox != Some(open) || self.state.view.unified)
        {
            let opened = self.open_mailbox(open, now_ms, receipt)?;
            receipt = opened.receipt;
            effects = opened.effects;
        }
        self.state.view.focused = Some(target);
        self.state.view.search = None;
        Ok(Outcome {
            receipt: receipt.affecting(EntityRef::View).with_data(
                serde_json::json!({ "focused": target, "mailbox": self.state.view.mailbox }),
            ),
            effects,
        })
    }

    /// `set_list_filter`: the given filters replace the current ones.
    pub(super) fn set_list_filter(
        &mut self,
        unread: Option<bool>,
        starred: Option<bool>,
        attachments: Option<bool>,
        receipt: CommandReceipt,
    ) -> Outcome {
        let filter = &mut self.state.view.filter;
        if let Some(on) = unread {
            filter.unread = on;
        }
        if let Some(on) = starred {
            filter.starred = on;
        }
        if let Some(on) = attachments {
            filter.attachments = on;
        }
        let filter = *filter;
        // Checked threads the filter hides stay checked only if they still show.
        let shown = crate::listing::visible_threads(&self.state);
        self.state.view.checked.retain(|id| shown.contains(id));
        Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .with_data(serde_json::json!({
                    "filter": filter,
                    "threads": shown.len(),
                })),
        )
    }

    /// `clear_focus`.
    pub(super) fn clear_focus(&mut self, receipt: CommandReceipt) -> Outcome {
        let cleared = self.state.view.focused.take().is_some();
        Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .with_data(serde_json::json!({ "cleared": cleared })),
        )
    }

    pub(super) fn toggle_sidebar(&mut self, receipt: CommandReceipt) -> Outcome {
        let view = &mut self.state.view;
        view.sidebar_collapsed = !view.sidebar_collapsed;
        let collapsed = view.sidebar_collapsed;
        Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .with_data(serde_json::json!({ "sidebar_collapsed": collapsed })),
        )
    }

    /// `get_view`: the `ViewState` as it is.
    pub(super) fn get_view(&self, receipt: CommandReceipt) -> Outcome {
        // reason: serializing a plain struct cannot fail; `Null` is a harmless fallback.
        let data = serde_json::to_value(&self.state.view).unwrap_or_default();
        Outcome::new(receipt.affecting(EntityRef::View).with_data(data))
    }
}
