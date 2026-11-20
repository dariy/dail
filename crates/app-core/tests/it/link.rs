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
