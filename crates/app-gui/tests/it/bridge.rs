//! The bridge between the UI thread and a `Session` on a background runtime.

use std::time::{Duration, Instant};

use crate::support::{add_alice, inbox_of, runtime, session, synced, wait_for_replies};
use app_core::{Command, CommandName, Origin};
use app_gui::{Bridge, Purpose, Request};
use app_sync::{IpcClient, IpcServer};

#[test]
fn submits_in_order_and_returns_replies_with_their_purpose() {
    let rt = runtime();
    let (_backend, session) = rt.block_on(session());
    let (bridge, _task) = Bridge::start(rt.handle(), session, egui::Context::default());
    bridge.submit(Request::user(add_alice()));
    bridge.submit(Request::user(Command::Undo {}));
    let mut replies = wait_for_replies(&bridge);
    if replies.len() < 2 {
        replies.extend(wait_for_replies(&bridge));
    }
    let names: Vec<CommandName> = replies.iter().map(|r| r.name).collect();
    assert_eq!(names, [CommandName::AddAccount, CommandName::Undo]);
    assert!(replies.iter().all(|r| r.purpose == Purpose::User));
    let receipt = replies[0].result.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(receipt.name, "add_account");
    assert!(bridge.state().accounts.is_empty(), "undone");
}

#[test]
fn rejections_come_back_as_replies() {
    let rt = runtime();
    let (_backend, session) = rt.block_on(session());
    let (bridge, _task) = Bridge::start(rt.handle(), session, egui::Context::default());
    bridge.submit(Request::user(Command::Undo {}));
    let replies = wait_for_replies(&bridge);
    assert!(replies[0].result.as_ref().unwrap().is_err());
}

#[test]
fn state_changes_are_visible_and_request_a_repaint() {
    let rt = runtime();
    let (_backend, session) = rt.block_on(session());
    let ctx = egui::Context::default();
    let (bridge, _task) = Bridge::start(rt.handle(), session, ctx.clone());
    // Settle the first frame so that only later repaint requests count.
    let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
    let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
    assert!(!ctx.has_requested_repaint());
    bridge.submit(Request::user(add_alice()));
    wait_for_replies(&bridge);
    assert_eq!(bridge.state().accounts.len(), 1);
    assert!(ctx.has_requested_repaint());
}

#[test]
fn closing_the_bridge_shuts_the_session_down() {
    let rt = runtime();
    let (_backend, session) = rt.block_on(session());
    let (mut bridge, task) = Bridge::start(rt.handle(), session, egui::Context::default());
    bridge.submit(Request::user(add_alice()));
    bridge.close();
    // Commands after closing are dropped.
    bridge.submit(Request::user(Command::Undo {}));
    rt.block_on(async { tokio::time::timeout(Duration::from_secs(5), task).await })
        .expect("session task ended")
        .unwrap();
    let replies = bridge.replies();
    assert_eq!(
        replies.len(),
        1,
        "only the command queued before closing ran"
    );
}

#[test]
fn an_agent_attached_over_ipc_changes_what_the_gui_shows() {
    let rt = runtime();
    let profile = std::env::temp_dir().join(format!("dail-gui-attach-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&profile);
    std::fs::create_dir_all(&profile).unwrap();
    let (_backend, session, _account) = rt.block_on(synced());
    // The GUI owns the profile and listens on its socket, as `dail gui` does.
    let server = rt
        .block_on(async { IpcServer::listen(&profile, session.client()) })
        .unwrap();
    let ctx = egui::Context::default();
    let (bridge, _task) = Bridge::start(rt.handle(), session, ctx.clone());
    // Frames until the UI is idle: the sync's state changes wake it once more after the start.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        if !ctx.has_requested_repaint() {
            break;
        }
        assert!(Instant::now() < deadline, "the GUI never went idle");
        std::thread::sleep(Duration::from_millis(10));
    }
    let (inbox, threads) = inbox_of(&bridge.state());
    let thread = threads[0];

    // `dail mcp`, attached to the GUI, submits as the agent over the socket.
    rt.block_on(async {
        let agent = IpcClient::connect(server.endpoint()).await.unwrap();
        let archive = Command::ArchiveThreads {
            ids: vec![thread],
            messages: Vec::new(),
        };
        agent.submit(&archive, Origin::Mcp).await.unwrap().unwrap();
        agent.flush().await.unwrap();
    });

    // The GUI's next frame no longer lists the thread in the inbox, and the change woke the UI.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ctx.has_requested_repaint()
        || app_core::listing::threads_in(&bridge.state(), inbox)
            .iter()
            .any(|t| t.id == thread)
    {
        assert!(
            Instant::now() < deadline,
            "the GUI never saw the agent's archive"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(server);
    let _ = std::fs::remove_dir_all(profile);
}
