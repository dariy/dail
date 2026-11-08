//! Sorting rules (ADR 0015, SET-4): `list_rules`, `save_rule`, `delete_rule`, `enable_rule`,
//! `apply_rules`, and the rule run on `EnvelopesFetched`.
//!
//! The evaluator ([`crate::rules`]) is pure; this module turns its plans into ordinary commands
//! (`mark_read`, `star`, `move_threads`, `archive_threads`) and applies them with
//! `Origin::Rule`, so they are checked, undoable and emit their usual remote ops. Actions are
//! state-based: a message already read, starred, in the target folder or archived is left out,
//! so two devices running the same rule converge and a second run is a no-op.
//!
//! No loops: on fetch the rules see only the messages the fetch *added*. A message a rule moved
//! reaches its target as the placeholder of that move, which a later fetch *locates* (not adds),
//! so it is not sorted again; the first fetch of a mailbox and a UIDVALIDITY reset add the
//! whole mailbox and run no rules either.

use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::command::Command;
use crate::ctx::{Ctx, Origin};
use crate::error::CoreError;
use crate::history::HistoryEntry;
use crate::ids::{MailboxId, MessageId, RuleId};
use crate::mailbox::MailboxRole;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::rules::{Placement, RulePlan, Scope, plan};
use crate::synced::{MAX_RULES, Rule, RuleEntry, SettingsError};

use super::{Applied, Engine, check_origin, invalid, not_found};

/// The commands that rules' plans became, and what was left out.
struct RuleRun {
    /// The commands, in the order they run.
    commands: Vec<Command>,
    /// `{message, rule?, action, reason}` per action that was not run.
    skipped: Vec<Value>,
    /// The messages some rule matched.
    matched: usize,
}

impl Engine {
    pub(super) fn list_rules(&self, receipt: CommandReceipt) -> Outcome {
        let rules: Vec<Value> = self
            .state
            .synced
            .settings
            .rules
            .iter()
            .filter_map(|(id, entry)| {
                entry.rule.as_ref().map(|rule| {
                    json!({
                        "id": id,
                        "enabled": rule.enabled,
                        "updated_ms": entry.updated_ms,
                        "untrusted_content": {
                            "name": rule.name,
                            "when": rule.when,
                            "then": rule.then,
                        },
                    })
                })
            })
            .collect();
        Outcome::new(receipt.affecting(EntityRef::Settings).with_data(json!({
            "rules": rules,
            "dirty": self.state.synced.dirty,
        })))
    }

    /// `save_rule`: validate, stamp and store; undo deletes a new rule or restores the old one.
    pub(super) fn save_rule(
        &mut self,
        id: Option<RuleId>,
        rule: Rule,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        rule.validate("rule").map_err(rule_error)?;
        let rules = &self.state.synced.settings.rules;
        let id = match id {
            Some(id) if rules.contains_key(&id) => id,
            Some(id) => return Err(not_found("rule", id)),
            None => {
                let mut ids = ctx.ids();
                loop {
                    let id: RuleId = ids.next_id();
                    // A tombstone's ID may be taken again: redo of an undone create does that.
                    if rules.get(&id).is_none_or(|entry| entry.rule.is_none()) {
                        break id;
                    }
                }
            }
        };
        let previous = rules.get(&id).cloned();
        let was_live = previous.as_ref().and_then(|entry| entry.rule.clone());
        if was_live.is_none() && rules.len() >= MAX_RULES {
            return Err(invalid(
                "rule",
                format!("there are {MAX_RULES} rules already"),
            ));
        }
        if was_live.as_ref() == Some(&rule) {
            return Ok(Applied::plain(Outcome::new(
                receipt
                    .affecting(EntityRef::Settings)
                    .with_data(json!({ "id": id, "created": false, "changed": false })),
            )));
        }
        let created = was_live.is_none();
        let label = format!("Save rule {}", rule.name);
        self.put_rule(id, Some(rule), previous.as_ref(), ctx);
        let inverse = match was_live {
            Some(old) => Command::SaveRule {
                id: Some(id),
                rule: old,
            },
            None => Command::DeleteRule { id },
        };
        Ok(Applied {
            outcome: Outcome::new(
                receipt
                    .affecting(EntityRef::Settings)
                    .with_data(json!({ "id": id, "created": created, "changed": true })),
            )
            .with_effect(self.persist_synced()),
            undo: Some((label, vec![inverse])),
        })
    }

    /// `delete_rule`: leave a tombstone; undo saves the rule again.
    pub(super) fn delete_rule(
        &mut self,
        id: RuleId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let previous = self.live_rule(id)?;
        let label = format!("Delete rule {}", previous.1.name);
        let (entry, rule) = previous;
        self.put_rule(id, None, Some(&entry), ctx);
        Ok(Applied {
            outcome: Outcome::new(
                receipt
                    .affecting(EntityRef::Settings)
                    .with_data(json!({ "id": id })),
            )
            .with_effect(self.persist_synced()),
            undo: Some((label, vec![Command::SaveRule { id: Some(id), rule }])),
        })
    }

    /// `enable_rule`: set `enabled`; undo sets it back.
    pub(super) fn enable_rule(
        &mut self,
        id: RuleId,
        enabled: bool,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let (entry, mut rule) = self.live_rule(id)?;
        let receipt = receipt.affecting(EntityRef::Settings);
        if rule.enabled == enabled {
            return Ok(Applied::plain(Outcome::new(receipt.with_data(
                json!({ "id": id, "enabled": enabled, "changed": false }),
            ))));
        }
        let label = format!(
            "{} rule {}",
            if enabled { "Enable" } else { "Disable" },
            rule.name
        );
        rule.enabled = enabled;
        self.put_rule(id, Some(rule), Some(&entry), ctx);
        Ok(Applied {
            outcome: Outcome::new(
                receipt.with_data(json!({ "id": id, "enabled": enabled, "changed": true })),
            )
            .with_effect(self.persist_synced()),
            undo: Some((
                label,
                vec![Command::EnableRule {
                    id,
                    enabled: !enabled,
                }],
            )),
        })
    }

    /// `apply_rules`: run the enabled rules over the messages shown in `mailbox`, as one undo
    /// step (redo runs the rules again over the restored state, which plans the same commands).
    pub(super) fn apply_rules(
        &mut self,
        mailbox: MailboxId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        if !self.state.mailboxes.contains_key(&mailbox) {
            return Err(not_found("mailbox", mailbox));
        }
        let shown: Vec<MessageId> = self
            .state
            .messages
            .values()
            .filter(|m| m.location() == mailbox && !m.flags.draft)
            .map(|m| m.id)
            .collect();
        let run = self.plan_rules(&shown, Scope::Explicit);
        let receipt = receipt.affecting(EntityRef::Mailbox(mailbox));
        let snapshot = self.state.clone();
        let mut outcome = Outcome::new(receipt);
        let mut inverses: Vec<Vec<Command>> = Vec::new();
        let mut ran: Vec<&'static str> = Vec::new();
        for (index, cmd) in run.commands.into_iter().enumerate() {
            let sub = rule_ctx(ctx, index);
            ran.push(cmd.name());
            match self.apply_rule_command(cmd, &sub) {
                Ok(applied) => {
                    merge_outcome(&mut outcome, applied.outcome);
                    if let Some((_, inverse)) = applied.undo {
                        inverses.push(inverse);
                    }
                }
                Err(err) => {
                    self.state = snapshot;
                    return Err(err);
                }
            }
        }
        outcome.receipt = outcome.receipt.with_data(json!({
            "mailbox": mailbox,
            "matched": run.matched,
            "commands": ran,
            "skipped": run.skipped,
        }));
        if inverses.is_empty() {
            return Ok(Applied::plain(outcome));
        }
        let inverse = inverses.into_iter().rev().flatten().collect();
        let label = format!("Apply rules ({} commands)", ran.len());
        Ok(Applied {
            outcome,
            undo: Some((label, inverse)),
        })
    }

    /// After a fetch merged `added` into `mailbox`: run the enabled rules on those messages.
    /// Each command is its own undo step, labelled `Rule: …`. A failure is reported in the
    /// receipt's `rules.errors` and does not stop the others.
    pub(super) fn run_rules_on_fetch(
        &mut self,
        mailbox: MailboxId,
        added: &[MessageId],
        ctx: &Ctx,
        outcome: &mut Outcome,
    ) {
        if added.is_empty() || self.state.synced.settings.live_rules().next().is_none() {
            return;
        }
        let inbox = self.role(mailbox) == Some(MailboxRole::Inbox);
        let run = self.plan_rules(added, Scope::Fetch { inbox });
        if run.matched == 0 {
            return;
        }
        let mut ran = Vec::new();
        let mut errors = Vec::new();
        for (index, cmd) in run.commands.into_iter().enumerate() {
            let sub = rule_ctx(ctx, index);
            let forward = cmd.clone();
            let name = cmd.name();
            match self.apply_rule_command(cmd, &sub) {
                Ok(applied) => {
                    if let Some((label, inverse)) = applied.undo {
                        // Undoable, but a background fetch must not take the user's redo away.
                        let cap = self.history_cap();
                        self.history.push_undo(
                            HistoryEntry {
                                label: format!("Rule: {label}"),
                                forward,
                                inverse,
                                id_seed: sub.id_seed,
                            },
                            cap,
                        );
                    }
                    merge_outcome(outcome, applied.outcome);
                    ran.push(name);
                }
                Err(err) => errors.push(json!({ "command": name, "error": err.to_string() })),
            }
        }
        let report = json!({
            "matched": run.matched,
            "commands": ran,
            "skipped": run.skipped,
            "errors": errors,
        });
        if let Value::Object(data) = &mut outcome.receipt.data {
            data.insert("rules".to_owned(), report);
        }
    }

    /// Applies one command a rule produced, with the agent policy and the rule-origin check.
    fn apply_rule_command(&mut self, cmd: Command, ctx: &Ctx) -> Result<Applied, CoreError> {
        check_origin(&cmd, ctx)?;
        self.apply(cmd, ctx)
    }

    /// Plans the rules over `messages` and turns the plans into commands, leaving out what is
    /// done already.
    fn plan_rules(&self, messages: &[MessageId], scope: Scope) -> RuleRun {
        let rules: Vec<(RuleId, &Rule)> = self.state.synced.settings.live_rules().collect();
        let mut plans: Vec<RulePlan> = Vec::new();
        for id in messages {
            let Some(envelope) = self.state.messages.get(id) else {
                continue;
            };
            let Some(path) = self
                .state
                .mailboxes
                .get(&envelope.location())
                .map(|m| m.path.as_str())
            else {
                continue;
            };
            if let Some(found) = plan(rules.iter().copied(), envelope, path, scope) {
                plans.push(found);
            }
        }
        let matched = plans.len();
        let mut read = Vec::new();
        let mut star = Vec::new();
        let mut moves: IndexMap<MailboxId, Vec<MessageId>> = IndexMap::new();
        let mut archive = Vec::new();
        let mut skipped = Vec::new();
        for found in plans {
            let Some(envelope) = self.state.messages.get(&found.message) else {
                continue;
            };
            if found.mark_read && !envelope.flags.seen {
                read.push(found.message);
            }
            if found.star && !envelope.flags.flagged {
                star.push(found.message);
            }
            let here = envelope.location();
            match found.place {
                Some(Placement::Move(path)) => {
                    let target = self
                        .state
                        .mailboxes
                        .values()
                        .find(|m| m.account == envelope.account && m.path == path)
                        .map(|m| m.id);
                    match target {
                        Some(to) if to == here => {}
                        Some(to) => moves.entry(to).or_default().push(found.message),
                        None => skipped.push(json!({
                            "message": found.message,
                            "action": "move",
                            "reason": "no folder with this path",
                            "untrusted_content": { "to": path },
                        })),
                    }
                }
                Some(Placement::Archive)
                    if !matches!(
                        self.role(here),
                        Some(MailboxRole::Archive | MailboxRole::All)
                    ) =>
                {
                    archive.push(found.message);
                }
                Some(Placement::Archive) | None => {}
            }
            for label in found.labels {
                skipped.push(json!({
                    "message": found.message,
                    "action": "add_label",
                    "reason": "add_label does not run in this version",
                    "untrusted_content": { "label": label },
                }));
            }
        }
        let mut commands = Vec::new();
        if !read.is_empty() {
            commands.push(Command::MarkRead {
                ids: Vec::new(),
                messages: read,
            });
        }
        if !star.is_empty() {
            commands.push(Command::Star {
                ids: Vec::new(),
                messages: star,
            });
        }
        for (to, messages) in moves {
            commands.push(Command::MoveThreads {
                ids: Vec::new(),
                messages,
                to,
            });
        }
        if !archive.is_empty() {
            commands.push(Command::ArchiveThreads {
                ids: Vec::new(),
                messages: archive,
            });
        }
        RuleRun {
            commands,
            skipped,
            matched,
        }
    }

    /// The live rule `id` with its entry, or `ERR004`.
    fn live_rule(&self, id: RuleId) -> Result<(RuleEntry, Rule), CoreError> {
        self.state
            .synced
            .settings
            .rules
            .get(&id)
            .and_then(|entry| entry.rule.clone().map(|rule| (entry.clone(), rule)))
            .ok_or_else(|| not_found("rule", id))
    }

    /// Stores `rule` (`None`: a tombstone) under `id`, stamped later than `previous`, and marks
    /// the document as changed locally.
    fn put_rule(
        &mut self,
        id: RuleId,
        rule: Option<Rule>,
        previous: Option<&RuleEntry>,
        ctx: &Ctx,
    ) {
        let floor = previous.map_or(i64::MIN, |entry| entry.updated_ms.saturating_add(1));
        let updated_ms = ctx.now_ms.max(floor);
        let synced = &mut self.state.synced;
        synced
            .settings
            .rules
            .insert(id, RuleEntry { updated_ms, rule });
        synced.settings.updated_ms = synced.settings.updated_ms.max(updated_ms);
        synced.dirty = true;
    }
}

/// The context of the `index`-th command of a rule run: `Origin::Rule` and a seed of its own,
/// so that the commands of one run never derive the same IDs.
fn rule_ctx(ctx: &Ctx, index: usize) -> Ctx {
    let step = u128::try_from(index).unwrap_or(u128::MAX).wrapping_add(1);
    Ctx {
        now_ms: ctx.now_ms,
        id_seed: ctx.id_seed.wrapping_add(step << 96),
        origin: Origin::Rule,
    }
}

/// Adds `more`'s effects and affected entities to `outcome`.
fn merge_outcome(outcome: &mut Outcome, more: Outcome) {
    for entity in more.receipt.affected {
        if !outcome.receipt.affected.contains(&entity) {
            outcome.receipt.affected.push(entity);
        }
    }
    outcome.effects.extend(more.effects);
}

/// A rule that breaks a limit of the format, as `ERR002` on the field that is wrong.
fn rule_error(err: SettingsError) -> CoreError {
    match err {
        SettingsError::Invalid { at, reason } => invalid(&at, reason),
        other => invalid("rule", other.to_string()),
    }
}
