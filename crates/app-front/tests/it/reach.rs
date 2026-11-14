//! The reach table (PAR-2): its gaps are pinned, its two rules hold, and it agrees with the
//! action lists and the default keys.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use app_core::{AppState, Category, CommandKind, CommandName, MailboxRole};
use app_front::actions::{Run, account_actions, mailbox_actions, thread_actions};
use app_front::message_actions::message_actions;
use app_front::reach::{FrontEnd, Reach, reach};

use crate::support::synced_state;

/// Mail mutations that are still `Palette` on touch, with the roadmap item that gives each a
/// control. The list can only shrink: an entry that is no longer `Palette` fails the test, so
/// the item that closes a gap removes its entry.
const KNOWN_TOUCH_GAPS: &[(CommandName, &str)] = &[
    (CommandName::SaveAttachments, "PAR-4"),
    (CommandName::RetryPendingOps, "unscheduled"),
    (CommandName::DiscardPendingOp, "unscheduled"),
];

/// Every `Never`, with the ADR that decides it.
const NEVER: &[(CommandName, FrontEnd, &str)] = &[];

/// The `Flow` entries per front end (everything not `Flow`, `Palette` or `Never` is `Control`).
const FLOW_GUI: &[&str] = &[
    "set_keybinding",
    "set_identity",
    "create_folder",
    "rename_folder",
    "delete_folder",
    "empty_trash",
    "empty_junk",
    "remove_account",
    "set_settings_account",
    "save_rule",
    "set_sync_window",
    "mark_mailbox_read",
    "set_search",
    "install_update",
];
const FLOW_TUI: &[&str] = &[
    "set_setting",
    "set_keybinding",
    "set_key_preset",
    "set_theme",
    "set_settings_account",
    "save_rule",
    "install_update",
    "set_update_policy",
];
const FLOW_TOUCH: &[&str] = &[
    "set_keybinding",
    "create_folder",
    "rename_folder",
    "delete_folder",
    "empty_trash",
    "empty_junk",
    "remove_account",
    "save_rule",
    "set_sync_window",
    "mark_mailbox_read",
    "move_threads",
    "set_search",
];

fn with(front: FrontEnd, value: Reach) -> Vec<&'static str> {
    CommandName::ALL
        .iter()
        .filter(|&&name| reach(name, front) == value)
        .map(|name| name.as_str())
        .collect()
}

#[test]
fn the_gui_gaps_are_pinned() {
    assert_eq!(
        with(FrontEnd::Gui, Reach::Palette),
        [
            "get_settings",
            "redo",
            "update_account",
            "remove_identity",
            "set_mailbox_identity",
            "pull_settings",
            "list_rules",
            "list_mailboxes",
            "list_threads",
            "select_threads",
            "get_thread",
            "get_message",
            "list_pending_ops",
            "discard_pending_op",
            "search",
            "list_contacts",
            "focus_pane",
            "get_view",
            "compact_store",
            "list_approvals"
        ]
    );
    assert!(with(FrontEnd::Gui, Reach::Never).is_empty());
    assert_eq!(with(FrontEnd::Gui, Reach::Flow), FLOW_GUI);
}

#[test]
fn the_tui_gaps_are_pinned() {
    assert_eq!(
        with(FrontEnd::Tui, Reach::Palette),
        [
            "get_settings",
            "update_account",
            "set_password",
            "set_identity",
            "remove_identity",
            "set_mailbox_identity",
            "create_folder",
            "rename_folder",
            "delete_folder",
            "empty_trash",
            "empty_junk",
            "set_draft_from",
            "remove_account",
            "list_rules",
            "list_mailboxes",
            "set_sync_window",
            "list_threads",
            "expand_messages",
            "collapse_messages",
            "get_thread",
            "get_message",
            "show_images_from",
            "hide_images_from",
            "pin_mailbox",
            "unpin_mailbox",
            "hide_mailbox",
            "show_mailbox",
            "save_attachments",
            "mark_mailbox_read",
            "list_pending_ops",
            "retry_pending_ops",
            "discard_pending_op",
            "attach_file",
            "remove_attachment",
            "cancel_send",
            "search",
            "list_contacts",
            "set_list_filter",
            "clear_focus",
            "get_view",
            "compact_store",
            "list_approvals"
        ]
    );
    assert!(with(FrontEnd::Tui, Reach::Never).is_empty());
    assert_eq!(with(FrontEnd::Tui, Reach::Flow), FLOW_TUI);
}

#[test]
fn the_touch_gaps_are_pinned() {
    assert_eq!(
        with(FrontEnd::Touch, Reach::Palette),
        [
            "get_settings",
            "redo",
            "update_account",
            "set_identity",
            "remove_identity",
            "set_mailbox_identity",
            "list_rules",
            "list_mailboxes",
            "list_threads",
            "select_threads",
            "get_thread",
            "get_message",
            "show_mailbox",
            "save_attachments",
            "list_pending_ops",
            "retry_pending_ops",
            "discard_pending_op",
            "search",
            "list_contacts",
            "clear_search",
            "set_list_filter",
            "clear_focus",
            "get_view",
            "compact_store",
            "check_for_updates",
            "download_update",
            "install_update",
            "list_approvals",
            "approve_request",
            "reject_request"
        ]
    );
    assert!(with(FrontEnd::Touch, Reach::Never).is_empty());
    assert_eq!(with(FrontEnd::Touch, Reach::Flow), FLOW_TOUCH);
}

/// Rule A: on touch no `Mutation` of category *Mail* is `Palette`, but for the known gaps,
/// which must still be gaps.
#[test]
fn on_touch_every_mail_mutation_has_a_control() {
    let gaps: Vec<CommandName> = CommandName::ALL
        .iter()
        .copied()
        .filter(|name| {
            name.kind() == CommandKind::Mutation
                && name.spec().category == Category::Mail
                && reach(*name, FrontEnd::Touch) == Reach::Palette
        })
        .collect();
    let known: Vec<CommandName> = KNOWN_TOUCH_GAPS.iter().map(|(name, _)| *name).collect();
    for name in &gaps {
        assert!(
            known.contains(name),
            "{}: a Mail mutation only in the palette on touch; give it a control",
            name.as_str()
        );
    }
    for (name, item) in KNOWN_TOUCH_GAPS {
        assert!(
            gaps.contains(name),
            "{}: no longer a touch gap, remove it from KNOWN_TOUCH_GAPS ({item})",
            name.as_str()
        );
    }
}

/// Rule B: nothing is `Never` unless an ADR says so.
#[test]
fn every_never_names_an_adr() {
    let mut table = BTreeSet::new();
    for &name in CommandName::ALL {
        for &front in FrontEnd::ALL {
            if reach(name, front) == Reach::Never {
                table.insert((name.as_str(), format!("{front:?}")));
            }
        }
    }
    let listed: BTreeSet<_> = NEVER
        .iter()
        .map(|(name, front, _)| (name.as_str(), format!("{front:?}")))
        .collect();
    assert_eq!(table, listed, "every Never is listed in NEVER with its ADR");
    let root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
        .join("../..");
    for (name, _, adr) in NEVER {
        assert!(adr.starts_with("docs/adr/"), "{}: {adr}", name.as_str());
        assert!(root.join(adr).is_file(), "{}: {adr} exists", name.as_str());
    }
}

/// The command an action-list entry leads to, its flow included.
fn run_name(run: &Run) -> CommandName {
    match run {
        Run::Command(cmd) => cmd.command_name(),
        Run::Move(_) => CommandName::MoveThreads,
        Run::RuleFrom(_) => CommandName::SaveRule,
        Run::NewSubfolder(_) => CommandName::CreateFolder,
        Run::Rename(_) => CommandName::RenameFolder,
        Run::Delete(_) => CommandName::DeleteFolder,
        Run::MarkAllRead(_) => CommandName::MarkMailboxRead,
        Run::Empty(_, MailboxRole::Junk) => CommandName::EmptyJunk,
        Run::Empty(..) => CommandName::EmptyTrash,
        Run::SyncWindow(_) => CommandName::SetSyncWindow,
        Run::Password(_) => CommandName::SetPassword,
        Run::Addresses(_) => CommandName::SetIdentity,
        Run::RemoveAccount(_) => CommandName::RemoveAccount,
    }
}

/// The fixture in every flag combination, with Trash, Junk and Drafts roles and the folders
/// pinned and hidden or not, so the lists return every entry they can.
fn states() -> Vec<AppState> {
    let base = synced_state();
    let mut out = Vec::new();
    for (seen, flagged, pinned, special) in [
        (false, false, false, false),
        (true, true, true, true),
        (true, false, true, false),
        (false, true, false, true),
    ] {
        let mut state = base.clone();
        for message in state.messages.values_mut() {
            message.flags.seen = seen;
            message.flags.flagged = flagged;
            message.flags.pinned = pinned;
        }
        if special {
            let keys: Vec<_> = state.mailboxes.values().map(|m| m.key()).collect();
            state.settings.pinned_mailboxes = keys.clone();
            let roles = [MailboxRole::Trash, MailboxRole::Junk];
            for (mailbox, role) in state
                .mailboxes
                .values_mut()
                .filter(|m| m.role != Some(MailboxRole::Inbox))
                .zip(roles)
            {
                mailbox.role = Some(role);
            }
            // Hidden: one folder (for the GUI's *Show folder*); the Inbox, Trash and Junk stay
            // pinned and shown, so the mobile drawer reaches *Unpin* and *Empty*.
            state.settings.hidden_mailboxes = state
                .mailboxes
                .values()
                .filter(|m| {
                    !matches!(
                        m.role,
                        Some(MailboxRole::Inbox | MailboxRole::Trash | MailboxRole::Junk)
                    )
                })
                .map(|m| m.key())
                .collect();
            let junk = state
                .mailboxes
                .values()
                .find(|m| m.role == Some(MailboxRole::Junk))
                .map(|m| m.id);
            state.view.mailbox = junk;
            if let Some(message) = state.messages.values_mut().next() {
                message.flags.draft = true;
            }
        }
        out.push(state);
    }
    out
}

/// Every command a list can return over [`states`]: the threads', mailboxes' and messages'
/// lists (drawn by the GUI and touch; touch draws the mailboxes' in the drawer's folder sheet,
/// TCH-4) and the accounts' list (drawn by the GUI).
fn listed(front: FrontEnd) -> HashSet<CommandName> {
    let mut out = HashSet::new();
    for state in states() {
        let threads: Vec<_> = state.threads.keys().copied().collect();
        let mut runs: Vec<Run> = thread_actions(&state, &threads)
            .into_iter()
            .map(|e| e.run)
            .collect();
        for id in &threads {
            runs.extend(thread_actions(&state, &[*id]).into_iter().map(|e| e.run));
        }
        for (&id, mailbox) in &state.mailboxes {
            // The mobile drawer shows no hidden folder (no *Show hidden* toggle), so touch never
            // opens a hidden folder's sheet.
            if front == FrontEnd::Touch && state.settings.hidden_mailboxes.contains(&mailbox.key())
            {
                continue;
            }
            runs.extend(mailbox_actions(&state, id).into_iter().map(|e| e.run));
        }
        if front == FrontEnd::Gui {
            for &id in state.accounts.keys() {
                runs.extend(account_actions(&state, id).into_iter().map(|e| e.run));
            }
        }
        for &id in state.messages.keys() {
            runs.extend(message_actions(&state, id).into_iter().map(|item| item.run));
        }
        out.extend(runs.iter().map(run_name));
    }
    out
}

#[test]
fn what_the_action_lists_offer_has_a_control() {
    for front in [FrontEnd::Gui, FrontEnd::Touch] {
        let names = listed(front);
        for name in [
            CommandName::MarkNotJunk,
            CommandName::Unstar,
            CommandName::UnpinThreads,
            CommandName::EditDraft,
            CommandName::SaveRule,
        ] {
            assert!(
                names.contains(&name),
                "{front:?}: the fixtures reach {name:?}"
            );
        }
        for name in [
            CommandName::EmptyTrash,
            CommandName::EmptyJunk,
            CommandName::UnpinMailbox,
        ] {
            assert!(
                names.contains(&name),
                "{front:?}: the fixtures reach {name:?}"
            );
        }
        if front == FrontEnd::Gui {
            assert!(
                names.contains(&CommandName::ShowMailbox),
                "{front:?}: the fixtures reach ShowMailbox"
            );
            assert!(
                names.contains(&CommandName::SetPassword),
                "{front:?}: the fixtures reach SetPassword"
            );
        }
        for name in names {
            assert!(
                matches!(reach(name, front), Reach::Control | Reach::Flow),
                "{front:?}: {} is in an action list, so it is not {:?}",
                name.as_str(),
                reach(name, front)
            );
        }
    }
}

/// On the TUI a default key is a control (the rule of `app_front::reach`).
#[test]
fn on_the_tui_a_default_key_is_a_control() {
    for &name in CommandName::ALL {
        if !name.spec().default_keys.is_empty() {
            assert_eq!(
                reach(name, FrontEnd::Tui),
                Reach::Control,
                "{} has default keys",
                name.as_str()
            );
        }
    }
}

/// `Palette` means the palette offers it: the parity walks then prove it reaches it.
#[test]
fn every_palette_entry_is_offered() {
    for &front in FrontEnd::ALL {
        for &name in CommandName::ALL {
            if reach(name, front) == Reach::Palette {
                assert!(
                    app_front::palette::offered(name),
                    "{front:?}: {} is Palette but the palette does not offer it",
                    name.as_str()
                );
            }
        }
    }
}
