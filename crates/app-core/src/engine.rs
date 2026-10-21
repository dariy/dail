//! `Engine`: the only place where `AppState` changes.

mod accounts;
mod approvals;
mod backfill;
mod draft_saves;
mod drafts;
mod identities;
mod images;
mod keys;
mod moves;
mod outbox;
mod pending;
mod preview;
mod queries;
mod saves;
mod search;
mod sends;
mod sync;
mod triage;
mod view;
mod watch;

pub use preview::{Change, Diff, Preview};
pub use watch::WATCH_RETRY_MS;

use std::collections::BTreeMap;

use crate::command::{Command, SettingKey, ThemeMode};
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, Event, PersistDelta};
use crate::history::{History, HistoryEntry};
use crate::ids::AccountId;
use crate::message::BodyState;
use crate::ops::{FlagKind, OpState};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::registry::AgentAccess;
use crate::state::{AppState, HIGH_CONTRAST_DARK_THEME, HIGH_CONTRAST_LIGHT_THEME};
use crate::threading::Threader;

/// The deterministic state machine. State changes enter only through [`Engine::dispatch`] and
/// [`Engine::ingest`].
#[derive(Debug, Clone, Default)]
pub struct Engine {
    state: AppState,
    history: History,
    /// Threading indexes kept between merges, per account, with the `index_epoch` they were
    /// built at: rebuilding one costs ~60 ms at 100k messages. Derived from `state`, never
    /// serialized.
    threaders: BTreeMap<AccountId, (u64, Threader)>,
    /// Bumped whenever messages change outside a merge (removed, placeholders added), which
    /// the kept indexes don't follow.
    index_epoch: u64,
}

/// The result of applying one command, before history bookkeeping.
struct Applied {
    outcome: Outcome,
    /// `(label, inverse commands)` if the command is undoable.
    undo: Option<(String, Vec<Command>)>,
}

impl Applied {
    fn plain(outcome: Outcome) -> Self {
        Self {
            outcome,
            undo: None,
        }
    }
}

impl Engine {
    /// An engine over an existing state (e.g. a snapshot loaded from the store). Ops that were
    /// running when the state was saved are queued again (every remote step is safe to repeat).
    pub fn new(state: AppState) -> Self {
        let mut state = state;
        for op in state.outbox.values_mut() {
            if op.state == OpState::Running {
                op.state = OpState::Queued;
            }
        }
        // Saves that were running are simply made again (the draft still needs saving).
        state.status.drafts_saving.clear();
        // Approval requests go with their drafts.
        let drafts = state.drafts.clone();
        state
            .approvals
            .retain(|_, approval| drafts.contains_key(&approval.draft));
        // Server searches in flight never report; their hits are in the cache anyway.
        state.status.searches.clear();
        // A send is never made behind the user's back after a restart: scheduled sends lost
        // their undo window, and an interrupted one may or may not have left.
        for draft in state.drafts.values_mut() {
            if draft.sending {
                draft.send_error =
                    Some("sending was interrupted; check Sent before sending it again".to_owned());
            } else if draft.send_at.is_some() {
                draft.send_error =
                    Some("the scheduled send was cancelled by a restart; send it again".to_owned());
            }
            draft.sending = false;
            draft.send_at = None;
        }
        Self {
            state,
            history: History::default(),
            threaders: BTreeMap::new(),
            index_epoch: 0,
        }
    }

    /// The account's threading index: the kept one if nothing changed since, else a new one.
    fn take_threader(&mut self, account: AccountId) -> Threader {
        match self.threaders.remove(&account) {
            Some((epoch, threader)) if epoch == self.index_epoch => threader,
            _ => Threader::new(account, self.state.messages.values()),
        }
    }

    /// Keeps the index a merge brought up to date.
    fn keep_threader(&mut self, account: AccountId, threader: Threader) {
        self.threaders.insert(account, (self.index_epoch, threader));
    }

    /// Messages changed in a way the kept threading indexes don't follow.
    fn messages_changed(&mut self) {
        self.index_epoch = self.index_epoch.wrapping_add(1);
    }

    /// Read-only view of the current state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Read-only view of the undo/redo history.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// User/agent intent. Validates, mutates, records history and returns the effects to run. A
    /// rejected command leaves the state and the history untouched.
    pub fn dispatch(&mut self, cmd: Command, ctx: &Ctx) -> Result<Outcome, CoreError> {
        self.state.revision.bump();
        let outcome = self.dispatch_unsettled(cmd, ctx)?;
        self.settle_view();
        Ok(outcome)
    }

    /// Keeps `ViewState` consistent after every change: a composer whose draft is gone closes,
    /// and the focus leaves a pane that is no longer shown.
    fn settle_view(&mut self) {
        let AppState { view, drafts, .. } = &mut self.state;
        view.settle(|id| drafts.contains_key(id));
    }

    fn dispatch_unsettled(&mut self, cmd: Command, ctx: &Ctx) -> Result<Outcome, CoreError> {
        match cmd {
            Command::Undo {} => self.undo(ctx),
            Command::Redo {} => self.redo(ctx),
            cmd => {
                check_origin(&cmd, ctx)?;
                let forward = self.recorded_forward(&cmd);
                let Applied { mut outcome, undo } = self.apply(cmd, ctx)?;
                if let Some((label, inverse)) = undo {
                    outcome.receipt.undo_label = Some(label.clone());
                    self.history.record(
                        HistoryEntry {
                            label,
                            forward,
                            inverse,
                            id_seed: ctx.id_seed,
                        },
                        self.history_cap(),
                    );
                }
                Ok(outcome)
            }
        }
    }

    /// Fact from the outside world (the result of an `Effect`). Never recorded in history.
    pub fn ingest(&mut self, event: Event, ctx: &Ctx) -> Result<Outcome, CoreError> {
        self.state.revision.bump();
        let outcome = self.ingest_unsettled(event, ctx)?;
        self.settle_view();
        Ok(outcome)
    }

    fn ingest_unsettled(&mut self, event: Event, ctx: &Ctx) -> Result<Outcome, CoreError> {
        let receipt = CommandReceipt::event(event.name());
        match event {
            Event::AccountVerified { .. }
            | Event::AccountVerifyFailed { .. }
            | Event::SecretStored { .. }
            | Event::SecretDeleted { .. }
            | Event::SecretFailed { .. } => Ok(self.ingest_account_event(event, ctx, receipt)),
            Event::MailboxesListed { account, mailboxes } => {
                Ok(self.ingest_mailboxes_listed(account, mailboxes, ctx, receipt))
            }
            Event::EnvelopesFetched {
                account,
                mailbox,
                uid_validity,
                uid_next,
                total,
                unread,
                envelopes,
            } => Ok(self.ingest_envelopes_fetched(
                sync::FetchedEnvelopes {
                    account,
                    mailbox,
                    uid_validity,
                    uid_next,
                    total,
                    unread,
                    envelopes,
                },
                ctx,
                receipt,
            )),
            Event::FlagsChanged {
                account,
                mailbox,
                uid_validity,
                from_uid,
                to_uid,
                flags,
                existing,
                highest_modseq,
            } => Ok(self.ingest_flags_changed(
                sync::ChangedFlags {
                    account,
                    mailbox,
                    uid_validity,
                    from_uid,
                    to_uid,
                    flags,
                    existing,
                    highest_modseq,
                },
                ctx,
                receipt,
            )),
            Event::BodyFetched {
                message,
                blob,
                has_attachments,
            } => Ok(self.ingest_body(
                message,
                BodyState::Fetched { blob },
                Some(has_attachments),
                receipt,
            )),
            Event::BodyFetchFailed { message, error } => {
                Ok(self.ingest_body(message, BodyState::Failed { error }, None, receipt))
            }
            Event::SyncFailed {
                account,
                mailbox,
                error,
            } => Ok(self.ingest_sync_failed(account, mailbox, error, ctx, receipt)),
            Event::RemoteOpSucceeded { op } => Ok(self.ingest_op_succeeded(op, ctx, receipt)),
            Event::RemoteOpFailed {
                op,
                error,
                permanent,
            } => Ok(self.ingest_op_failed(op, error, permanent, ctx, receipt)),
            Event::Tick { now_ms } => Ok(self.ingest_tick(now_ms, receipt)),
            Event::Closing {} => Ok(self.ingest_closing(ctx, receipt)),
            Event::DraftSaved { draft, revision } => {
                Ok(self.ingest_draft_saved(draft, Ok(revision), ctx, receipt))
            }
            Event::DraftSaveFailed { draft, error } => {
                Ok(self.ingest_draft_saved(draft, Err(error), ctx, receipt))
            }
            Event::DraftDeleted { draft } => {
                Ok(Outcome::new(receipt.affecting(EntityRef::Draft(draft))))
            }
            Event::DraftDeleteFailed {
                account,
                draft,
                error,
            } => Ok(self.ingest_draft_delete_failed(account, draft, error, ctx, receipt)),
            Event::MessageSent {
                draft,
                sent_copy_error,
            } => Ok(self.ingest_message_sent(draft, sent_copy_error, ctx, receipt)),
            Event::SendFailed { draft, error } => {
                Ok(self.ingest_send_failed(draft, error, receipt))
            }
            Event::AttachmentInspected {
                draft,
                path,
                size,
                content_type,
            } => Ok(self.ingest_attachment(
                draft,
                &path,
                crate::draft::AttachmentState::Ready { size, content_type },
                receipt,
            )),
            Event::AttachmentFailed { draft, path, error } => Ok(self.ingest_attachment(
                draft,
                &path,
                crate::draft::AttachmentState::Failed { error },
                receipt,
            )),
            Event::ServersDiscovered {
                email,
                imap,
                smtp,
                source,
            } => Ok(self.ingest_discovery(
                email,
                Ok(crate::state::DiscoveredServers { imap, smtp, source }),
                receipt,
            )),
            Event::DiscoveryFailed { email, error } => {
                Ok(self.ingest_discovery(email, Err(error), receipt))
            }
            Event::IndexSearched { search, hits } => {
                Ok(self.ingest_index_search(search, Ok(hits), ctx, receipt))
            }
            Event::IndexSearchFailed { search, error } => {
                Ok(self.ingest_index_search(search, Err(error), ctx, receipt))
            }
            Event::StoreCompacted {
                removed_blobs,
                freed_bytes,
            } => Ok(self.ingest_compacted(
                crate::state::Compaction::Done {
                    at_ms: ctx.now_ms,
                    removed_blobs,
                    freed_bytes,
                },
                receipt,
            )),
            Event::CompactFailed { error } => Ok(self.ingest_compacted(
                crate::state::Compaction::Failed {
                    at_ms: ctx.now_ms,
                    error,
                },
                receipt,
            )),
            Event::OlderFetched {
                account,
                mailbox,
                uid_validity,
                envelopes,
                complete,
            } => Ok(self.ingest_older(
                account,
                mailbox,
                Ok((uid_validity, envelopes, complete)),
                ctx,
                receipt,
            )),
            Event::BackfillFailed {
                account,
                mailbox,
                error,
            } => Ok(self.ingest_older(account, mailbox, Err(error), ctx, receipt)),
            Event::MailboxWatched {
                account,
                mailbox,
                changed,
            } => Ok(self.ingest_watched(account, mailbox, changed, receipt)),
            Event::WatchFailed {
                account,
                mailbox,
                error,
                unsupported,
            } => Ok(self.ingest_watch_failed(account, mailbox, error, unsupported, ctx, receipt)),
            Event::AttachmentsSaved { message, files } => Ok(self.ingest_saved(
                message,
                crate::state::SaveState::Saved { files },
                ctx,
                receipt,
            )),
            Event::AttachmentsSaveFailed {
                message,
                files,
                error,
            } => Ok(self.ingest_saved(
                message,
                crate::state::SaveState::Failed { files, error },
                ctx,
                receipt,
            )),
            Event::SearchResults {
                account,
                mailbox,
                uid_validity,
                search,
                started_ms,
                mailboxes,
                envelopes,
            } => Ok(self.ingest_search_results(
                search::SearchRound {
                    account,
                    mailbox,
                    search,
                    started_ms,
                    mailboxes,
                },
                Ok((uid_validity, envelopes)),
                ctx,
                receipt,
            )),
            Event::SearchFailed {
                account,
                mailbox,
                search,
                started_ms,
                mailboxes,
                error,
            } => Ok(self.ingest_search_results(
                search::SearchRound {
                    account,
                    mailbox,
                    search,
                    started_ms,
                    mailboxes,
                },
                Err(error),
                ctx,
                receipt,
            )),
            Event::Persisted {} => {
                self.state.status.persist_error = None;
                Ok(Outcome::new(receipt.affecting(EntityRef::Status)))
            }
            Event::PersistFailed { error } => {
                self.state.status.persist_error = Some(error);
                Ok(Outcome::new(receipt.affecting(EntityRef::Status)))
            }
        }
    }

    fn history_cap(&self) -> usize {
        usize::try_from(self.state.settings.history_cap).unwrap_or(usize::MAX)
    }

    /// Applies one command without touching the history.
    fn apply(&mut self, cmd: Command, ctx: &Ctx) -> Result<Applied, CoreError> {
        let edited = match &cmd {
            Command::UpdateDraft { .. }
            | Command::SetDraftFrom { .. }
            | Command::AttachFile { .. }
            | Command::RemoveAttachment { .. } => cmd.draft_id(),
            _ => None,
        };
        let mut applied = self.apply_command(cmd, ctx)?;
        if let Some(draft) = edited.filter(|_| ctx.origin.is_agent()) {
            self.withdraw_on_agent_edit(draft, &mut applied);
        }
        Ok(applied)
    }

    fn apply_command(&mut self, cmd: Command, ctx: &Ctx) -> Result<Applied, CoreError> {
        let receipt = CommandReceipt::command(cmd.name(), cmd.kind());
        match cmd {
            Command::GetSettings {} => {
                // reason: serializing a plain struct cannot fail; `Null` is a harmless fallback.
                let data = serde_json::to_value(&self.state.settings).unwrap_or_default();
                Ok(Applied::plain(Outcome::new(
                    receipt.affecting(EntityRef::Settings).with_data(data),
                )))
            }
            Command::SetKeybinding { command, keys } => {
                self.set_keybinding(&command, keys, receipt)
            }
            Command::SetKeyPreset { preset } => Ok(self.set_key_preset(preset, receipt)),
            Command::SetTheme { mode, light, dark } => self.set_theme(mode, light, dark, receipt),
            Command::SetSetting { key, value } => {
                let new = validate_setting(key, &value)?;
                // The legacy high-contrast values name a theme now (ADR 0008).
                if key == SettingKey::Theme && new >= 3 {
                    let (mode, light, dark) = if new == 3 {
                        (
                            ThemeMode::Dark,
                            None,
                            Some(HIGH_CONTRAST_DARK_THEME.to_owned()),
                        )
                    } else {
                        (
                            ThemeMode::Light,
                            Some(HIGH_CONTRAST_LIGHT_THEME.to_owned()),
                            None,
                        )
                    };
                    return self.set_theme(Some(mode), light, dark, receipt);
                }
                let previous = key.get(&self.state.settings);
                key.set(&mut self.state.settings, new);
                let outcome = Outcome::new(receipt.affecting(EntityRef::Settings).with_data(
                    serde_json::json!({
                        "key": key,
                        "value": new,
                        "previous": previous,
                    }),
                ))
                .with_effect(Effect::PersistState {
                    delta: PersistDelta::Settings(self.state.settings.clone()),
                });
                let inverse = Command::SetSetting {
                    key,
                    value: serde_json::Value::from(previous),
                };
                Ok(Applied {
                    outcome,
                    undo: Some((format!("Set {}", key.as_str()), vec![inverse])),
                })
            }
            Command::OpenPalette {} => {
                self.state.view.palette_open = true;
                Ok(Applied::plain(Outcome::new(
                    receipt.affecting(EntityRef::View),
                )))
            }
            Command::ClosePalette {} => {
                self.state.view.palette_open = false;
                Ok(Applied::plain(Outcome::new(
                    receipt.affecting(EntityRef::View),
                )))
            }
            Command::AddAccount {
                preset,
                email,
                display_name,
                password,
                imap,
                smtp,
            } => self.add_account(
                accounts::NewAccount {
                    preset,
                    email,
                    display_name,
                    password,
                    imap,
                    smtp,
                },
                ctx,
                receipt,
            ),
            Command::TestAccount { id } => self.test_account(id, receipt).map(Applied::plain),
            Command::UpdateAccount {
                id,
                display_name,
                imap,
                smtp,
            } => self.update_account(
                accounts::AccountPatch {
                    id,
                    display_name,
                    imap,
                    smtp,
                },
                receipt,
            ),
            Command::SetPassword { id, password } => {
                self.set_password(id, password, receipt).map(Applied::plain)
            }
            Command::SetIdentity {
                account,
                email,
                name,
                signature,
            } => self.set_identity(
                account,
                crate::account::Identity {
                    email,
                    name,
                    signature,
                },
                receipt,
            ),
            Command::RemoveIdentity { account, email } => {
                self.remove_identity(account, &email, receipt)
            }
            Command::SetMailboxIdentity { mailbox, email } => {
                self.set_mailbox_identity(mailbox, email, receipt)
            }
            Command::SetDraftFrom { id, email } => self.set_draft_from(id, &email, ctx, receipt),
            Command::RemoveAccount { id, confirm } => self
                .remove_account(id, confirm, receipt)
                .map(Applied::plain),
            Command::SyncNow { account } => {
                self.sync_now(account, ctx, receipt).map(Applied::plain)
            }
            Command::ListMailboxes { account } => {
                self.list_mailboxes(account, receipt).map(Applied::plain)
            }
            Command::ListThreads {
                mailbox,
                offset,
                limit,
            } => self
                .list_threads(mailbox, offset, limit, receipt)
                .map(Applied::plain),
            Command::SetSyncWindow { mailbox, window } => {
                self.set_sync_window(mailbox, window, receipt)
            }
            Command::OpenMailbox { id } => self.open_mailbox(id, receipt).map(Applied::plain),
            Command::OpenUnifiedInbox {} => Ok(Applied::plain(self.open_unified_inbox(receipt))),
            Command::SelectThreads { ids } => self.select_threads(ids, receipt).map(Applied::plain),
            Command::CheckThreads { ids } => {
                self.check_threads(ids, true, receipt).map(Applied::plain)
            }
            Command::UncheckThreads { ids } => {
                self.check_threads(ids, false, receipt).map(Applied::plain)
            }
            Command::OpenThread { id } => self.open_thread(id, receipt).map(Applied::plain),
            Command::CloseThread {} => Ok(Applied::plain(self.close_thread(receipt))),
            Command::ExpandMessages { ids } => {
                self.set_expanded(ids, true, receipt).map(Applied::plain)
            }
            Command::CollapseMessages { ids } => {
                self.set_expanded(ids, false, receipt).map(Applied::plain)
            }
            Command::GetThread {
                id, include_body, ..
            } => self
                .get_thread(id, include_body, receipt)
                .map(Applied::plain),
            Command::GetMessage { id, include_body } => self
                .get_message(id, include_body, receipt)
                .map(Applied::plain),
            Command::SaveAttachments {
                message,
                parts,
                dir,
            } => self
                .save_attachments(message, parts, dir, ctx, receipt)
                .map(Applied::plain),
            Command::ShowImagesFrom { sender } => self.images_from(&sender, true, receipt),
            Command::HideImagesFrom { sender } => self.images_from(&sender, false, receipt),
            Command::MarkRead { ids, messages } => self.set_flag(
                flag_change(FlagKind::Seen, true, ids, messages),
                ctx,
                receipt,
            ),
            Command::MarkUnread { ids, messages } => self.set_flag(
                flag_change(FlagKind::Seen, false, ids, messages),
                ctx,
                receipt,
            ),
            Command::Star { ids, messages } => self.set_flag(
                flag_change(FlagKind::Flagged, true, ids, messages),
                ctx,
                receipt,
            ),
            Command::Unstar { ids, messages } => self.set_flag(
                flag_change(FlagKind::Flagged, false, ids, messages),
                ctx,
                receipt,
            ),
            Command::ArchiveThreads { ids, messages } => self.move_messages(
                moves::MoveRequest {
                    kind: moves::MoveKind::Archive,
                    threads: ids,
                    messages,
                },
                ctx,
                receipt,
            ),
            Command::TrashThreads { ids, messages } => self.move_messages(
                moves::MoveRequest {
                    kind: moves::MoveKind::Trash,
                    threads: ids,
                    messages,
                },
                ctx,
                receipt,
            ),
            Command::ListPendingOps { account } => {
                self.list_pending_ops(account, receipt).map(Applied::plain)
            }
            Command::RetryPendingOps { id } => {
                self.retry_pending_ops(id, ctx, receipt).map(Applied::plain)
            }
            Command::DiscardPendingOp { id } => self
                .discard_pending_op(id, ctx, receipt)
                .map(Applied::plain),
            Command::MoveThreads { ids, messages, to } => self.move_messages(
                moves::MoveRequest {
                    kind: moves::MoveKind::To(to),
                    threads: ids,
                    messages,
                },
                ctx,
                receipt,
            ),
            Command::CreateDraft {
                account,
                to,
                cc,
                bcc,
                subject,
                body,
            } => self.create_draft(
                account,
                drafts::DraftFields {
                    to: Some(to),
                    cc: Some(cc),
                    bcc: Some(bcc),
                    subject: Some(subject),
                    body: Some(body),
                },
                ctx,
                receipt,
            ),
            Command::Reply { message, body } => {
                self.answer(message, body, drafts::Answer::Reply, ctx, receipt)
            }
            Command::ReplyAll { message, body } => {
                self.answer(message, body, drafts::Answer::ReplyAll, ctx, receipt)
            }
            Command::Forward { message, body } => {
                self.answer(message, body, drafts::Answer::Forward, ctx, receipt)
            }
            Command::UpdateDraft {
                id,
                to,
                cc,
                bcc,
                subject,
                body,
            } => self.update_draft(
                id,
                drafts::DraftFields {
                    to,
                    cc,
                    bcc,
                    subject,
                    body,
                },
                ctx,
                receipt,
            ),
            Command::DiscardDraft { id } => self.discard_draft(id, receipt),
            Command::RestoreDraft { draft } => self.restore_draft(*draft, ctx, receipt),
            Command::AttachFile { draft, path, index } => {
                self.attach_file(draft, path, index, ctx, receipt)
            }
            Command::RemoveAttachment { draft, index } => {
                self.remove_attachment(draft, index, ctx, receipt)
            }
            Command::SendDraft { id } => self.send_draft(id, ctx, receipt),
            Command::CancelSend { id } => self.cancel_send(id, ctx, receipt),
            Command::Search {
                query,
                account,
                offset,
                limit,
                server,
                order,
            } => self
                .search(
                    search::SearchRequest {
                        query: &query,
                        account,
                        offset,
                        limit,
                        server,
                        order,
                    },
                    ctx,
                    receipt,
                )
                .map(Applied::plain),
            Command::SetSearch { query } => {
                self.set_search(query, ctx, receipt).map(Applied::plain)
            }
            Command::ClearSearch {} => Ok(Applied::plain(self.clear_search(receipt))),
            Command::OpenComposer { id } => self.open_composer(id, receipt).map(Applied::plain),
            Command::CloseComposer {} => Ok(Applied::plain(self.close_composer(receipt))),
            Command::FocusPane { pane } => self.focus_pane(pane, receipt).map(Applied::plain),
            Command::ToggleSidebar {} => Ok(Applied::plain(self.toggle_sidebar(receipt))),
            Command::GetView {} => Ok(Applied::plain(self.get_view(receipt))),
            Command::DiscoverServers { email } => {
                self.discover_servers(&email, receipt).map(Applied::plain)
            }
            Command::CompactStore {} => Ok(Applied::plain(self.compact_store(ctx, receipt))),
            Command::ListApprovals {} => Ok(Applied::plain(self.list_approvals(receipt))),
            Command::ListContacts { query, limit } => self
                .list_contacts(&query, limit, receipt)
                .map(Applied::plain),
            Command::ApproveRequest { id } => self.approve_request(id, ctx, receipt),
            Command::RejectRequest { id } => self.reject_request(id, receipt).map(Applied::plain),
            // History commands are handled by `dispatch`; nested undo is not a thing.
            Command::Undo {} => Err(CoreError::NothingToUndo),
            Command::Redo {} => Err(CoreError::NothingToRedo),
        }
    }

    /// Applies `commands` in order as one transaction: on error, state is restored.
    fn apply_all(
        &mut self,
        commands: &[Command],
        ctx: &Ctx,
        mut receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let snapshot = self.state.clone();
        let mut effects = Vec::new();
        for cmd in commands {
            match self.apply(cmd.clone(), ctx) {
                Ok(applied) => {
                    for entity in applied.outcome.receipt.affected {
                        if !receipt.affected.contains(&entity) {
                            receipt.affected.push(entity);
                        }
                    }
                    effects.extend(applied.outcome.effects);
                }
                Err(err) => {
                    self.state = snapshot;
                    return Err(err);
                }
            }
        }
        Ok(Outcome { receipt, effects })
    }

    fn undo(&mut self, ctx: &Ctx) -> Result<Outcome, CoreError> {
        let entry = self.history.pop_undo().ok_or(CoreError::NothingToUndo)?;
        let receipt = CommandReceipt::command("undo", crate::CommandKind::Mutation)
            .with_data(serde_json::json!({ "undone": entry.label }));
        match self.apply_all(&entry.inverse, ctx, receipt) {
            Ok(outcome) => {
                self.history.push_redo(entry);
                Ok(outcome)
            }
            Err(err) => {
                let cap = self.history_cap();
                self.history.push_undo(entry, cap);
                Err(err)
            }
        }
    }

    fn redo(&mut self, ctx: &Ctx) -> Result<Outcome, CoreError> {
        let entry = self.history.pop_redo().ok_or(CoreError::NothingToRedo)?;
        let receipt = CommandReceipt::command("redo", crate::CommandKind::Mutation)
            .with_data(serde_json::json!({ "redone": entry.label }));
        let ctx = Ctx {
            id_seed: entry.id_seed,
            ..*ctx
        };
        match self.apply_all(std::slice::from_ref(&entry.forward), &ctx, receipt) {
            Ok(mut outcome) => {
                outcome.receipt.undo_label = Some(entry.label.clone());
                let cap = self.history_cap();
                self.history.push_undo(entry, cap);
                Ok(outcome)
            }
            Err(err) => {
                self.history.push_redo(entry);
                Err(err)
            }
        }
    }
}

/// The agent policy (`CommandSpec::agents`): a user-only command is refused from every origin but
/// the user's. Only commands from front-ends get here; `undo`/`redo` apply commands from history,
/// which the engine built itself or which passed this check when they were first dispatched.
fn check_origin(cmd: &Command, ctx: &Ctx) -> Result<(), CoreError> {
    let name = cmd.command_name();
    if name.agent_access() == AgentAccess::UserOnly && !ctx.origin.is_user() {
        return Err(CoreError::OriginNotAllowed {
            command: name.as_str().to_owned(),
            origin: ctx.origin.as_str().to_owned(),
        });
    }
    Ok(())
}

/// `not_found` for an `entity` (e.g. `"draft"`) and the ID that was looked up.
fn not_found(entity: &str, id: impl ToString) -> CoreError {
    CoreError::NotFound {
        entity: entity.to_owned(),
        id: id.to_string(),
    }
}

/// `invalid_argument` for `field`, with the reason and the fix.
fn invalid(field: &str, reason: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

fn flag_change(
    flag: FlagKind,
    value: bool,
    threads: Vec<crate::ids::ThreadId>,
    messages: Vec<crate::ids::MessageId>,
) -> triage::FlagChange {
    triage::FlagChange {
        flag,
        value,
        threads,
        messages,
    }
}

fn validate_setting(key: SettingKey, value: &serde_json::Value) -> Result<u64, CoreError> {
    let (min, max) = key.range();
    let number = value.as_u64().ok_or_else(|| {
        invalid(
            "value",
            format!("{} expects a non-negative integer", key.as_str()),
        )
    })?;
    if number < min || number > max {
        return Err(invalid(
            "value",
            format!("{} must be between {min} and {max}", key.as_str()),
        ));
    }
    Ok(number)
}
