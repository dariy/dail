//! View commands: `open_mailbox`, `select_threads`, `open_thread`, `close_thread`,
//! `expand_messages`, `collapse_messages`, `open_composer`, `close_composer`, `focus_pane`,
//! `toggle_sidebar`. They change only
//! `ViewState` (opening a thread also starts body downloads, tracked in
//! `status.bodies_in_flight`).

use crate::error::CoreError;
use crate::ids::{DraftId, MailboxId, MessageId, ThreadId};
use crate::message::BodyState;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::Pane;

use super::{Engine, invalid, not_found};

impl Engine {
    pub(super) fn open_mailbox(
        &mut self,
        id: MailboxId,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !self.state.mailboxes.contains_key(&id) {
            return Err(not_found("mailbox", id));
        }
        let view = &mut self.state.view;
        view.mailbox = Some(id);
        view.unified = false;
        view.selected.clear();
        view.checked.clear();
        view.thread = None;
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::View)
                .affecting(EntityRef::Mailbox(id))
                .with_data(serde_json::json!({ "mailbox": id })),
        ))
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
