//! A device linked to a dail server (ADR 0017, SRV-1): command placement, `StateDiff`, mirror
//! mode and the offline queue.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, names};
use crate::undo::{Action, action, apply, mailbox};
use app_core::{
    AppState, Command, CommandKind, CommandName, Event, LinkStatus, MessageId, Placement,
    SettingKey, Settings, StateDiff, StatusState, ThreadId,
};
use proptest::prelude::*;
use serde_json::json;

#[test]
fn every_view_command_runs_on_the_device() {
    for &name in CommandName::ALL {
        if name.kind() == CommandKind::View {
            assert_eq!(name.placement(), Placement::Client, "{}", name.as_str());
        }
    }
}

#[test]
fn data_changing_commands_run_on_the_server() {
    // Mutations, sends and destructive commands, except those that concern the device itself.
    let device = [
        CommandName::SetKeybinding,
        CommandName::SetKeyPreset,
        CommandName::SetTheme,
        CommandName::CompactStore,
        CommandName::CheckForUpdates,
        CommandName::DownloadUpdate,
        CommandName::InstallUpdate,
        CommandName::SkipUpdateVersion,
        CommandName::SetUpdatePolicy,
        CommandName::SaveAttachments,
        // The device's own link to its server (SRV-2).
        CommandName::ConnectServer,
        CommandName::TrustServer,
        CommandName::DisconnectServer,
        CommandName::RetryOffline,
        CommandName::DismissOfflineRejection,
    ];
    for &name in CommandName::ALL {
        let data = matches!(
            name.kind(),
            CommandKind::Mutation | CommandKind::Outbound | CommandKind::Destructive
        );
        if data && !device.contains(&name) {
            assert!(name.placement().on_server(), "{}", name.as_str());
        }
    }
    for name in device {
        assert!(!name.placement().on_server(), "{}", name.as_str());
    }
}

#[test]
fn placement_table() {
    let table = [
        (CommandName::AttachFile, Placement::ServerWithUpload),
        (CommandName::SaveAttachments, Placement::ClientWithBlob),
        (CommandName::SendDraft, Placement::Server),
        (CommandName::AddAccount, Placement::Server),
        (CommandName::SaveRule, Placement::Server),
        (CommandName::Search, Placement::Server),
        (CommandName::ListThreads, Placement::Server),
        (CommandName::Undo, Placement::Server),
        (CommandName::GetView, Placement::Client),
        (CommandName::GetSettings, Placement::Client),
        (CommandName::SetTheme, Placement::Client),
        (CommandName::InstallUpdate, Placement::Client),
        (CommandName::OpenMailbox, Placement::Client),
    ];
    for (name, placement) in table {
        assert_eq!(name.placement(), placement, "{}", name.as_str());
        assert_eq!(name.example().placement(), placement, "{}", name.as_str());
    }
    let json = serde_json::to_value(Placement::ServerWithUpload).unwrap();
    assert_eq!(json, json!("server_with_upload"));
}

#[test]
fn set_setting_follows_its_key() {
    let set = |key| Command::SetSetting {
        key,
        value: json!(1),
    };
    for (key, placement) in [
        (SettingKey::Theme, Placement::Client),
        (SettingKey::TextScale, Placement::Client),
        (SettingKey::Notifications, Placement::Client),
        (SettingKey::TuiColors, Placement::Client),
        (SettingKey::BackgroundSync, Placement::Client),
        (SettingKey::PollSecs, Placement::Server),
        (SettingKey::UndoSendMs, Placement::Server),
        (SettingKey::HistoryCap, Placement::Server),
    ] {
        assert_eq!(set(key).placement(), placement, "{key:?}");
    }
    // Settings every device shares go through the server.
    for &key in SettingKey::ALL {
        if key.is_shared() {
            assert_eq!(key.placement(), Placement::Server, "{key:?}");
        }
    }
}

fn ids(m: &Mail) -> (Vec<MessageId>, Vec<ThreadId>) {
    let messages: Vec<MessageId> = (1..=4).map(|uid| m.message("INBOX", uid)).collect();
    let mut threads = Vec::new();
    for id in &messages {
        let thread = m.thread_of(*id);
        if !threads.contains(&thread) {
            threads.push(thread);
        }
    }
    (messages, threads)
}

/// `state` without its device-local fields.
fn shared(state: &AppState) -> AppState {
    let mut state = state.clone();
    state.view = AppState::default().view;
    state.update = AppState::default().update;
    state.link = None;
    state.settings.keep_device_local(&Settings::default());
    state.status.keep_device_local(&StatusState::default());
    state
}

/// The JSON of the shared fields: unlike `==` on `IndexMap`, it compares entry order too.
fn shared_json(state: &AppState) -> serde_json::Value {
    serde_json::to_value(shared(state)).unwrap()
}

#[test]
fn diff_of_a_state_with_itself_is_empty() {
    let m = mailbox();
    let state = m.engine.state();
    let diff = StateDiff::diff(state, state);
    assert!(diff.is_empty());
    assert_eq!(serde_json::to_value(&diff).unwrap(), json!({}));
}

#[test]
fn diff_ignores_device_local_fields() {
    let m = mailbox();
    let mut other = m.engine.state().clone();
    other.view.palette_open = true;
    other.link = Some(app_core::LinkState::default());
    other.update.last_check_ms = Some(1);
    assert!(StateDiff::diff(m.engine.state(), &other).is_empty());
}

#[test]
fn snapshot_rebuilds_the_shared_state() {
    let m = mailbox();
    let mut state = AppState::default();
    StateDiff::snapshot(m.engine.state()).apply(&mut state);
    assert_eq!(shared_json(&state), shared_json(m.engine.state()));
}

#[test]
fn diff_restores_entry_order() {
    let m = mailbox();
    let a = m.engine.state().clone();
    let mut b = a.clone();
    b.messages.reverse();
    let diff = StateDiff::diff(&a, &b);
    assert!(diff.messages.upsert.is_empty());
    assert!(diff.messages.order.is_some());
    let mut c = a.clone();
    diff.apply(&mut c);
    assert_eq!(shared_json(&c), shared_json(&b));
    // Round trip through JSON, as it crosses the network.
    let back: StateDiff = serde_json::from_value(serde_json::to_value(&diff).unwrap()).unwrap();
    assert_eq!(back, diff);
}

fn run_all(actions: &[Action]) -> Mail {
    let mut m = mailbox();
    let (messages, threads) = ids(&m);
    for action in actions {
        apply(&mut m, action, &messages, &threads);
    }
    m
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn diff_then_apply_gives_the_new_state(
        first in prop::collection::vec(action(), 0..20),
        second in prop::collection::vec(action(), 0..20),
        more_mail in any::<bool>(),
    ) {
        let a = run_all(&first);
        let mut b = run_all(&second);
        if more_mail {
            // Device-local settings and status never travel.
            b.run(Command::SetSetting { key: SettingKey::TextScale, value: json!(150) }).unwrap();
            b.feed(Event::NetworkChanged { metered: true });
            b.fetch("INBOX", vec![mail(9, "n@x", Some("a@x"), "Re: Plans", false)]);
            let account = b.account;
            b.run(Command::CreateDraft {
                account,
                to: vec![],
                cc: vec![],
                bcc: vec![],
                subject: "Hi".into(),
                body: String::new(),
            })
            .unwrap();
        }
        for (old, new) in [(&a, &b), (&b, &a)] {
            let (old, new) = (old.engine.state(), new.engine.state());
            let diff = StateDiff::diff(old, new);
            let mut applied = old.clone();
            diff.apply(&mut applied);
            prop_assert_eq!(shared_json(&applied), shared_json(new));
            prop_assert_eq!(&applied.view, &old.view);
            let mut device = applied.settings.clone();
            device.keep_device_local(&old.settings);
            prop_assert_eq!(&device, &applied.settings);
            let mut device = applied.status.clone();
            device.keep_device_local(&old.status);
            prop_assert_eq!(&device, &applied.status);
            let json = serde_json::to_value(&diff).unwrap();
            prop_assert_eq!(serde_json::from_value::<StateDiff>(json).unwrap(), diff);
        }
        prop_assert!(StateDiff::diff(a.engine.state(), a.engine.state()).is_empty());
    }
}

/// `mailbox()` turned into a mirror whose link has `status`.
fn mirror(status: LinkStatus) -> Mail {
    let mut m = mailbox();
    m.feed(Event::LinkChanged { status });
    m
}

#[test]
fn offline_mirror_applies_and_queues_without_network_effects() {
    let mut m = mirror(LinkStatus::Offline);
    let (messages, threads) = ids(&m);
    let seed = m.seed;
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![threads[0]],
            messages: vec![],
        })
        .unwrap();
    assert!(
        outcome
            .effects
            .iter()
            .all(app_core::Effect::is_device_local),
        "{:?}",
        names(&outcome.effects)
    );
    assert!(
        names(&outcome.effects)
            .iter()
            .all(|n| *n != "run_remote_op")
    );
    let state = m.engine.state();
    let archive = m.mailbox("Archive");
    assert_eq!(
        state.messages[&messages[0]].moved_to,
        Some(archive),
        "applied"
    );
    let link = state.link.as_ref().unwrap();
    assert_eq!(link.queued.len(), 1);
    assert_eq!(link.queued[0].seed, seed << 64);
    assert_eq!(link.queued[0].command.name(), "archive_threads");
    // View commands and queries are not queued.
    m.run(Command::OpenMailbox { id: archive }).unwrap();
    m.run(Command::ListThreads {
        mailbox: archive,
        offset: 0,
        limit: 10,
    })
    .unwrap();
    assert_eq!(m.engine.state().link.as_ref().unwrap().queued.len(), 1);
    // Undo is a server command too.
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().link.as_ref().unwrap().queued.len(), 2);
}

#[test]
fn online_mirror_does_not_queue() {
    let mut m = mirror(LinkStatus::Online);
    let (_, threads) = ids(&m);
    let outcome = m
        .run(Command::Star {
            ids: vec![threads[1]],
            messages: vec![],
        })
        .unwrap();
    assert!(
        outcome
            .effects
            .iter()
            .all(app_core::Effect::is_device_local)
    );
    assert!(m.engine.state().link.as_ref().unwrap().queued.is_empty());
}

#[test]
fn ticks_of_a_mirror_never_reach_mail_servers() {
    let mut m = mirror(LinkStatus::Online);
    let outcome = m.feed(Event::Tick {
        now_ms: crate::support::T0 + 86_400_000,
    });
    assert!(
        outcome
            .effects
            .iter()
            .all(app_core::Effect::is_device_local),
        "{:?}",
        names(&outcome.effects)
    );
}

#[test]
fn without_a_link_nothing_changes() {
    let mut m = mailbox();
    let (_, threads) = ids(&m);
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![threads[0]],
            messages: vec![],
        })
        .unwrap();
    assert!(names(&outcome.effects).contains(&"run_remote_op"));
    assert!(m.engine.state().link.is_none());
}

#[test]
fn remote_synced_overwrites_the_mirror() {
    let server = mailbox();
    let mut m = mirror(LinkStatus::Offline);
    let (messages, threads) = ids(&m);
    m.run(Command::Star {
        ids: vec![threads[2]],
        messages: vec![],
    })
    .unwrap();
    assert!(m.engine.state().messages[&messages[3]].flags.flagged);
    let diff = StateDiff::diff(m.engine.state(), server.engine.state());
    m.feed(Event::RemoteSynced {
        revision: 5,
        diff: Box::new(diff),
        snapshot: false,
    });
    let state = m.engine.state();
    assert_eq!(shared_json(state), shared_json(server.engine.state()));
    let link = state.link.as_ref().unwrap();
    assert_eq!(link.revision, 5);
    assert_eq!(link.queued.len(), 1, "the queue waits for the replay");
    // An older diff is ignored; a snapshot is not.
    let outcome = m.feed(Event::RemoteSynced {
        revision: 4,
        diff: Box::new(StateDiff::snapshot(&AppState::default())),
        snapshot: false,
    });
    assert_eq!(outcome.receipt.data["stale"], true);
    m.feed(Event::RemoteSynced {
        revision: 1,
        diff: Box::new(StateDiff::snapshot(server.engine.state())),
        snapshot: true,
    });
    assert_eq!(
        shared_json(m.engine.state()),
        shared_json(server.engine.state())
    );
    assert_eq!(m.engine.state().link.as_ref().unwrap().revision, 1);
}

#[test]
fn remote_synced_is_not_undoable() {
    let server = mailbox();
    let mut m = mirror(LinkStatus::Online);
    let before = m.engine.history().undo_len();
    m.feed(Event::RemoteSynced {
        revision: 1,
        diff: Box::new(StateDiff::snapshot(server.engine.state())),
        snapshot: true,
    });
    assert_eq!(m.engine.history().undo_len(), before);
}

/// A value of `key` other than its default, within its range.
fn other_value(key: SettingKey) -> serde_json::Value {
    let value = match key {
        SettingKey::UndoSendMs | SettingKey::MarkReadAfterMs => 5000,
        SettingKey::PollSecs => 300,
        SettingKey::InitialWindow | SettingKey::HistoryCap => 50,
        SettingKey::Theme | SettingKey::ListDensity | SettingKey::AfterTriage => 2,
        SettingKey::TextScale => 150,
        SettingKey::Notifications => u64::from(Settings::default().notifications == 0),
        SettingKey::TuiColors => 1,
        SettingKey::BackgroundSync => 2,
        SettingKey::BackgroundIntervalMins => 60,
        SettingKey::StaleSecs => 300,
        SettingKey::FullSyncSecs => 600,
        SettingKey::Language => 3,
    };
    json!(value)
}

/// Which side the settings fields a command changes belong to: `Client` if it changed only
/// device fields (`Settings::keep_device_local`), `Server` if it changed a server field.
fn side_of(setup: &[Command], cmd: &Command) -> Placement {
    let mut m = mailbox();
    for step in setup {
        m.run(step.clone()).unwrap();
    }
    let before = m.engine.state().settings.clone();
    m.run(cmd.clone()).unwrap();
    let after = m.engine.state().settings.clone();
    assert_ne!(before, after, "{} changed no setting", cmd.name());
    let mut server_only = after;
    server_only.keep_device_local(&before);
    if server_only == before {
        Placement::Client
    } else {
        Placement::Server
    }
}

#[test]
fn settings_fields_follow_the_placement_of_the_commands_that_set_them() {
    let m = mailbox();
    let (inbox, account) = (m.mailbox("INBOX"), m.account);
    let sender = "a@x".to_owned();
    let mut cases: Vec<(Vec<Command>, Command)> = vec![
        (vec![], CommandName::SetTheme.example()),
        (vec![], CommandName::SetKeybinding.example()),
        (vec![], CommandName::SetKeyPreset.example()),
        (vec![], CommandName::SetUpdatePolicy.example()),
        (vec![], CommandName::SkipUpdateVersion.example()),
        (
            vec![],
            Command::ShowImagesFrom {
                sender: sender.clone(),
            },
        ),
        (
            vec![Command::ShowImagesFrom {
                sender: sender.clone(),
            }],
            Command::HideImagesFrom { sender },
        ),
        (vec![], Command::PinMailbox { id: inbox }),
        (
            vec![Command::PinMailbox { id: inbox }],
            Command::UnpinMailbox { id: inbox },
        ),
        (vec![], Command::HideMailbox { id: inbox }),
        (
            vec![Command::HideMailbox { id: inbox }],
            Command::ShowMailbox { id: inbox },
        ),
        (vec![], Command::SetSettingsAccount { account }),
    ];
    for &key in SettingKey::ALL {
        cases.push((
            vec![],
            Command::SetSetting {
                key,
                value: other_value(key),
            },
        ));
    }
    for (setup, cmd) in cases {
        assert_eq!(side_of(&setup, &cmd), cmd.placement(), "{cmd:?}");
    }
}

#[test]
fn client_settings_survive_a_server_diff() {
    let base = mailbox();
    let mut server = mailbox();
    let mut device = mailbox();
    for &key in SettingKey::ALL {
        let value = other_value(key);
        if key.placement() == Placement::Client {
            device.run(Command::SetSetting { key, value }).unwrap();
        } else {
            server.run(Command::SetSetting { key, value }).unwrap();
        }
    }
    device.run(CommandName::SetKeyPreset.example()).unwrap();
    server.feed(Event::NetworkChanged { metered: true });
    let mine = device.engine.state().clone();
    let diff = StateDiff::diff(base.engine.state(), server.engine.state());
    assert!(diff.settings.is_some());
    let mut applied = mine.clone();
    diff.apply(&mut applied);
    let mut expected = server.engine.state().settings.clone();
    expected.keep_device_local(&mine.settings);
    assert_eq!(applied.settings, expected);
    assert_eq!(applied.settings.text_scale, 150, "the device's own");
    assert_eq!(applied.settings.poll_secs, 300, "the server's");
    assert!(!applied.status.metered, "the device's network");
    // A snapshot over a cleared state keeps them too.
    StateDiff::clear(&mut applied);
    StateDiff::snapshot(server.engine.state()).apply(&mut applied);
    assert_eq!(applied.settings, expected);
}

#[test]
fn a_rejected_command_is_not_queued() {
    let mut m = mirror(LinkStatus::Offline);
    let unknown = ThreadId::from_uuid(uuid::Uuid::from_u128(77));
    assert!(
        m.run(Command::ArchiveThreads {
            ids: vec![unknown],
            messages: vec![],
        })
        .is_err()
    );
    assert!(
        m.run(Command::SendDraft {
            id: app_core::DraftId::from_uuid(uuid::Uuid::from_u128(78)),
        })
        .is_err()
    );
    assert!(m.engine.state().link.as_ref().unwrap().queued.is_empty());
}

#[test]
fn save_attachments_stays_on_the_device() {
    let mut m = mirror(LinkStatus::Offline);
    let report = m.message("INBOX", 1);
    m.feed(Event::BodyFetched {
        message: report,
        blob: "ab".repeat(32),
        has_attachments: true,
        snippet: None,
    });
    let outcome = m
        .run(Command::SaveAttachments {
            message: report,
            parts: vec![],
            dir: "/home/me/Downloads".into(),
        })
        .unwrap();
    assert_eq!(names(&outcome.effects), ["save_attachments"]);
    assert!(m.engine.state().link.as_ref().unwrap().queued.is_empty());
}

// --- SRV-5b: the offline policy, refusals, the queue's events and its persistence. ---

/// The commands refused while the link is down (`OfflinePolicy::Refuse`), pinned.
const REFUSED_OFFLINE: [CommandName; 19] = [
    // Destructive commands the server runs.
    CommandName::RemoveAccount,
    CommandName::DeleteFolder,
    CommandName::EmptyTrash,
    CommandName::EmptyJunk,
    CommandName::RevokeDevice,
    // Accounts, certificates and the synced settings' account.
    CommandName::AddAccount,
    CommandName::UpdateAccount,
    CommandName::SetPassword,
    CommandName::SetIdentity,
    CommandName::RemoveIdentity,
    CommandName::TrustCertificate,
    CommandName::ForgetCertificate,
    CommandName::SetSettingsAccount,
    // Only meaningful now.
    CommandName::PushSettings,
    CommandName::PullSettings,
    CommandName::SyncNow,
    // Devices and approvals.
    CommandName::StartPairing,
    CommandName::ApproveRequest,
    CommandName::RejectRequest,
];

#[test]
fn offline_policy_table() {
    use app_core::{AgentAccess, OfflinePolicy};
    for &name in CommandName::ALL {
        let policy = name.offline_policy();
        let local = !name.placement().on_server() || name.kind() == CommandKind::Query;
        assert_eq!(
            policy == OfflinePolicy::Local,
            local,
            "{}: local iff it runs on the device or is a query",
            name.as_str()
        );
        assert_eq!(
            policy == OfflinePolicy::Refuse,
            REFUSED_OFFLINE.contains(&name),
            "{}",
            name.as_str()
        );
        if name.placement().on_server() {
            // Every destructive server command and every user-only one but `restore_draft`
            // (the user's own draft, put back as it was) is refused.
            let refused = name.kind() == CommandKind::Destructive
                || (name.agent_access() == AgentAccess::UserOnly
                    && name != CommandName::RestoreDraft);
            if refused {
                assert_eq!(policy, OfflinePolicy::Refuse, "{}", name.as_str());
            }
        }
        if name != CommandName::SetSetting {
            assert_eq!(name.example().offline_policy(), policy, "{}", name.as_str());
        }
    }
    let pinned = [
        (CommandName::SendDraft, OfflinePolicy::Queue),
        (CommandName::CancelSend, OfflinePolicy::Queue),
        (CommandName::AttachFile, OfflinePolicy::Queue),
        (CommandName::RestoreDraft, OfflinePolicy::Queue),
        (CommandName::Undo, OfflinePolicy::Queue),
        (CommandName::ArchiveThreads, OfflinePolicy::Queue),
        (CommandName::SaveRule, OfflinePolicy::Queue),
        (CommandName::SaveAttachments, OfflinePolicy::Local),
        (CommandName::Search, OfflinePolicy::Local),
        (CommandName::DisconnectServer, OfflinePolicy::Local),
        (CommandName::InstallUpdate, OfflinePolicy::Local),
        (CommandName::RetryOffline, OfflinePolicy::Local),
    ];
    for (name, policy) in pinned {
        assert_eq!(name.offline_policy(), policy, "{}", name.as_str());
    }
    // `set_setting` follows its key.
    for &key in SettingKey::ALL {
        let cmd = Command::SetSetting {
            key,
            value: json!(1),
        };
        let expected = if key.placement().on_server() {
            OfflinePolicy::Queue
        } else {
            OfflinePolicy::Local
        };
        assert_eq!(cmd.offline_policy(), expected, "{key:?}");
    }
}

#[test]
fn offline_every_refused_command_changes_nothing() {
    for status in [LinkStatus::Offline, LinkStatus::Connecting] {
        let mut m = mirror(status);
        let undo = m.engine.history().undo_len();
        for name in REFUSED_OFFLINE {
            let before = m.engine.state().clone();
            let err = m.run(name.example()).unwrap_err();
            assert_eq!(
                err,
                app_core::CoreError::RefusedOffline {
                    command: name.as_str().to_owned()
                },
                "{}",
                name.as_str()
            );
            assert_eq!(err.code(), "ERR014");
            assert!(err.to_string().starts_with("ERR014: "), "{err}");
            let after = m.engine.state();
            assert_eq!(after.link, before.link, "{}: nothing queued", name.as_str());
            assert_eq!(after.accounts, before.accounts);
            assert_eq!(after.messages, before.messages);
        }
        assert_eq!(m.engine.history().undo_len(), undo);
    }
    // The wire form, as agents get it.
    let json = serde_json::to_value(app_core::CoreError::RefusedOffline {
        command: "sync_now".into(),
    })
    .unwrap();
    assert_eq!(
        json,
        json!({ "error": "error_014_refused_offline", "command": "sync_now" })
    );
    // Online and without a link they run.
    for mut m in [mirror(LinkStatus::Online), mailbox()] {
        m.run(Command::SyncNow { account: None }).unwrap();
    }
}

/// The queue's `persist_state`, if the outcome has one.
fn persisted_queue(outcome: &app_core::Outcome) -> Option<app_core::OfflineQueue> {
    outcome.effects.iter().find_map(|effect| match effect {
        app_core::Effect::PersistState {
            delta: app_core::PersistDelta::OfflineQueue(queue),
        } => Some(queue.clone()),
        _ => None,
    })
}

fn request(seed: u128) -> app_core::RequestId {
    app_core::RequestId::from_uuid(uuid::Uuid::from_u128(seed << 64))
}

#[test]
fn the_replay_events_deliver_reject_and_requeue_in_order() {
    let mut m = mirror(LinkStatus::Offline);
    let (_, threads) = ids(&m);
    let undo = m.engine.history().undo_len();
    let first = m.seed;
    let mut seeds = Vec::new();
    for cmd in [
        Command::Star {
            ids: vec![threads[0]],
            messages: vec![],
        },
        Command::ArchiveThreads {
            ids: vec![threads[1]],
            messages: vec![],
        },
        Command::MarkUnread {
            ids: vec![threads[0]],
            messages: vec![],
        },
    ] {
        seeds.push(m.seed);
        let outcome = m.run(cmd).unwrap();
        // Every change of the queue goes to the store.
        let queue = persisted_queue(&outcome).unwrap();
        assert_eq!(queue.queued, m.engine.state().link.as_ref().unwrap().queued);
    }
    let link = m.engine.state().link.clone().unwrap();
    let order: Vec<_> = link.queued.iter().map(|q| q.command.name()).collect();
    assert_eq!(order, ["star", "archive_threads", "mark_unread"]);
    assert_eq!(link.queued[0].request_id, request(first));

    // Delivered: it leaves the queue.
    let outcome = m.feed(Event::QueuedDelivered {
        request_id: request(seeds[0]),
    });
    assert!(persisted_queue(&outcome).is_some());
    // Refused: it moves to the rejections with the server's error and the runtime's time.
    let outcome = m.feed(Event::QueuedRejected {
        request_id: request(seeds[1]),
        code: "ERR004".into(),
        error: "ERR004: thread x not found".into(),
        at_ms: 1_234,
    });
    assert!(persisted_queue(&outcome).is_some());
    let link = m.engine.state().link.clone().unwrap();
    assert_eq!(link.queued.len(), 1);
    assert_eq!(link.queued[0].command.name(), "mark_unread");
    assert_eq!(link.rejected.len(), 1);
    let rejection = &link.rejected[0];
    assert_eq!(rejection.request_id, request(seeds[1]));
    assert_eq!(rejection.seed, seeds[1] << 64);
    assert_eq!(rejection.code, "ERR004");
    assert_eq!(rejection.at_ms, 1_234);
    assert_eq!(rejection.command.name(), "archive_threads");

    // Unknown request IDs are ignored and change nothing.
    for event in [
        Event::QueuedDelivered {
            request_id: request(99),
        },
        Event::QueuedRejected {
            request_id: request(99),
            code: "ERR004".into(),
            error: "gone".into(),
            at_ms: 1,
        },
    ] {
        let outcome = m.feed(event);
        assert_eq!(outcome.receipt.data["ignored"], true);
        assert!(persisted_queue(&outcome).is_none());
    }

    // A forwarded command whose answer was lost goes behind the queue, with its request ID.
    let lost = app_core::QueuedCommand {
        request_id: request(50),
        seed: 50 << 64,
        origin: app_core::Origin::Gui,
        command: Command::Unstar {
            ids: vec![threads[0]],
            messages: vec![],
        },
        queued_at_ms: 7,
    };
    let outcome = m.feed(Event::Requeued {
        entry: Box::new(lost.clone()),
    });
    assert!(persisted_queue(&outcome).is_some());
    // Twice, or a command never queued: ignored.
    let twice = m.feed(Event::Requeued {
        entry: Box::new(lost.clone()),
    });
    assert_eq!(twice.receipt.data["ignored"], true);
    let pairing = app_core::QueuedCommand {
        request_id: request(51),
        command: CommandName::StartPairing.example(),
        ..lost.clone()
    };
    let refused = m.feed(Event::Requeued {
        entry: Box::new(pairing),
    });
    assert_eq!(refused.receipt.data["ignored"], true);
    // `retry_offline` puts the rejection after both.
    m.run(Command::RetryOffline {}).unwrap();
    let link = m.engine.state().link.clone().unwrap();
    let order: Vec<_> = link.queued.iter().map(|q| q.command.name()).collect();
    assert_eq!(order, ["mark_unread", "unstar", "archive_threads"]);
    assert!(link.rejected.is_empty());
    assert_eq!(link.queued[2].seed, seeds[1] << 64);
    // None of it is undoable history: only the three commands are.
    assert_eq!(m.engine.history().undo_len(), undo + 3);

    // Without a link the events are ignored.
    let mut plain = mailbox();
    let outcome = plain.feed(Event::Requeued {
        entry: Box::new(lost),
    });
    assert_eq!(outcome.receipt.data["ignored"], true);
    assert!(plain.engine.state().link.is_none());
}

#[test]
fn the_queue_survives_a_restart() {
    let mut m = mirror(LinkStatus::Offline);
    let (_, threads) = ids(&m);
    let outcome = m
        .run(Command::Star {
            ids: vec![threads[0]],
            messages: vec![],
        })
        .unwrap();
    let queue = persisted_queue(&outcome).unwrap();
    assert_eq!(queue.queued.len(), 1);
    // A store that keeps snapshots applies the delta to the link `[remote]` gives it.
    let mut stored = AppState {
        link: Some(app_core::LinkState::default()),
        ..AppState::default()
    };
    app_core::PersistDelta::OfflineQueue(queue.clone()).apply_to(&mut stored);
    assert_eq!(stored.link.as_ref().unwrap().queued, queue.queued);
    let restarted = app_core::Engine::new(stored);
    assert_eq!(
        restarted.state().link.as_ref().unwrap().queued,
        queue.queued
    );
    // Without a link (no `[remote]` any more) nothing comes back.
    let mut plain = AppState::default();
    app_core::PersistDelta::OfflineQueue(queue.clone()).apply_to(&mut plain);
    assert!(plain.link.is_none());
    // Its JSON keeps the seed as a decimal string.
    let json = serde_json::to_value(&queue).unwrap();
    assert_eq!(
        json["queued"][0]["seed"],
        json!(queue.queued[0].seed.to_string())
    );
    assert_eq!(
        serde_json::from_value::<app_core::OfflineQueue>(json).unwrap(),
        queue
    );
}

#[test]
fn an_offline_send_is_queued() {
    let mut m = mirror(LinkStatus::Offline);
    let account = m.account;
    let draft = m
        .run(Command::CreateDraft {
            account,
            to: vec![app_core::Addr {
                name: None,
                email: "bob@example.com".into(),
            }],
            cc: vec![],
            bcc: vec![],
            subject: "Hi".into(),
            body: "Hello".into(),
        })
        .unwrap();
    let id: app_core::DraftId = serde_json::from_value(draft.receipt.data["id"].clone()).unwrap();
    m.run(Command::SendDraft { id }).unwrap();
    let link = m.engine.state().link.as_ref().unwrap();
    let order: Vec<_> = link.queued.iter().map(|q| q.command.name()).collect();
    assert_eq!(order, ["create_draft", "send_draft"]);
}

#[test]
fn a_snapshot_persists_the_mirrored_accounts() {
    let server = mailbox();
    let account = server.account;
    let mut device = mirror(LinkStatus::Offline);
    device.engine = app_core::Engine::new(AppState {
        link: Some(app_core::LinkState::default()),
        ..AppState::default()
    });
    let snapshot = |state: &AppState| Event::RemoteSynced {
        revision: 1,
        diff: Box::new(StateDiff::snapshot(state)),
        snapshot: true,
    };
    let outcome = device.feed(snapshot(server.engine.state()));
    let stored: Vec<_> = outcome
        .effects
        .iter()
        .filter_map(|effect| match effect {
            app_core::Effect::PersistState {
                delta: app_core::PersistDelta::Account(a),
            } => Some(a.id),
            _ => None,
        })
        .collect();
    assert_eq!(stored, [account]);
    // A snapshot without it removes it from the store.
    let outcome = device.feed(snapshot(&AppState::default()));
    assert!(outcome.effects.iter().any(|effect| matches!(
        effect,
        app_core::Effect::PersistState {
            delta: app_core::PersistDelta::AccountRemoved(id)
        } if *id == account
    )));
}
