//! Opens the TUI on the offline memory backend with the `basic` fixture, its account added and
//! synced: `cargo run -p app-tui --example tui-demo [fixture] [open] [compose] [agent] [palette]`
//! (`open` opens the newest inbox thread, `compose` a reply to it, `agent` lets an agent ask to
//! send a reply, `palette` the command palette). Under tmux an agent can drive it:
//! `tmux new -d -s t -x 50 -y 20 'cargo run -p app-tui --example tui-demo'`, then
//! `tmux send-keys -t t j Enter` and `tmux capture-pane -p -t t`.

// reason: a demo binary; failing loudly is the right behaviour.
#![allow(clippy::expect_used)]

use std::sync::Arc;

use app_core::{Command, Origin, ProviderPreset, Secret};
use app_sync::{Backend, RandomSeeds, Session, SystemClock};
use app_testkit::{Fixture, MemoryBackend};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let fixture = args
        .first()
        .filter(|a| !["open", "compose", "agent", "palette"].contains(&a.as_str()))
        .cloned()
        .unwrap_or_else(|| "basic".to_owned());
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let session = runtime.block_on(async {
        let fixture = Fixture::builtin(&fixture).expect("fixture");
        let email = fixture
            .accounts
            .first()
            .map(|a| a.email.clone())
            .expect("fixture has an account");
        let backend: Arc<dyn Backend> = Arc::new(MemoryBackend::from_fixture(fixture));
        let session = Session::open(backend, Arc::new(SystemClock), Arc::new(RandomSeeds))
            .await
            .expect("session");
        for cmd in [
            Command::AddAccount {
                preset: ProviderPreset::Hostinger,
                email,
                display_name: None,
                password: Some(Secret::new("demo")),
                imap: None,
                smtp: None,
            },
            Command::SyncNow { account: None },
        ] {
            let _ = session.submit(cmd, Origin::Cli).await.expect("submit");
            session.flush().await.expect("flush");
        }
        let inbox = session
            .state()
            .mailboxes
            .values()
            .find(|m| m.role == Some(app_core::MailboxRole::Inbox))
            .map(|m| m.id);
        if let Some(id) = inbox {
            let _ = session
                .submit(Command::OpenMailbox { id }, Origin::Cli)
                .await;
        }
        let newest = app_core::listing::visible_threads(&session.state())
            .first()
            .copied();
        if flag("open")
            && let Some(id) = newest
        {
            let _ = session
                .submit(Command::OpenThread { id }, Origin::Cli)
                .await;
            session.flush().await.expect("flush");
        }
        let newest_message = newest.and_then(|t| {
            session
                .state()
                .threads
                .get(&t)
                .and_then(|t| t.messages.last().copied())
        });
        if flag("compose")
            && let Some(message) = newest_message
        {
            let reply = session
                .submit(
                    Command::Reply {
                        message,
                        body: Some("Sounds good!".into()),
                    },
                    Origin::Cli,
                )
                .await
                .expect("submit")
                .expect("reply");
            let id = serde_json::from_value(reply.data["id"].clone()).expect("draft id");
            let _ = session
                .submit(Command::OpenComposer { id }, Origin::Cli)
                .await;
        }
        if flag("agent")
            && let Some(message) = newest_message
        {
            let reply = session
                .submit(
                    Command::Reply {
                        message,
                        body: Some("Thanks Bob, I'll review the numbers today.".into()),
                    },
                    Origin::Mcp,
                )
                .await
                .expect("submit")
                .expect("reply");
            let id = serde_json::from_value(reply.data["id"].clone()).expect("draft id");
            let _ = session.submit(Command::SendDraft { id }, Origin::Mcp).await;
        }
        if flag("palette") {
            let _ = session.submit(Command::OpenPalette {}, Origin::Cli).await;
        }
        session
    });
    app_tui::run(
        &runtime,
        app_tui::Target::Own(session),
        app_tui::Tui::default().owning_session(true),
    )
    .expect("TUI");
}
