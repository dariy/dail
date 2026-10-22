//! Prints the demo's frames as JSON, one per line: the `basic` fixture synced, its inbox, the
//! newest thread and the drawer, at a phone's, a small tablet's and a large tablet's size:
//! `cargo run -p app-mobile --example frames [width height]`.

// reason: a demo binary; failing loudly is the right behaviour.
#![allow(clippy::expect_used)]

use std::io::Write;

use app_core::{Command, Ctx, Engine, MailboxRole, Origin, Pane, listing};
use app_mobile::{Mobile, Reply, UiEvent};
use app_testkit::Fixture;

fn now() -> i64 {
    1_700_035_200_000
}

fn main() {
    let args: Vec<u32> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let sizes = match args.as_slice() {
        [w, h] => vec![(*w, *h)],
        _ => vec![(360, 800), (700, 1000), (1280, 800)],
    };
    let state = app_testkit::synced_state(Fixture::builtin("basic").expect("fixture"))
        .expect("synced state");
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Inbox))
        .map(|m| m.id)
        .expect("an inbox");
    let newest = listing::threads_in(&state, inbox).first().map(|t| t.id);
    let mut out = std::io::stdout().lock();
    for (width_dp, height_dp) in sizes {
        let mut engine = Engine::new(state.clone());
        let mut mobile = Mobile::with_clock(now);
        let mut seed = 1u128;
        let mut run = |engine: &mut Engine, mobile: &mut Mobile, cmds: Vec<Command>| {
            let mut replies = Vec::new();
            for cmd in cmds {
                seed += 1;
                let ctx = Ctx {
                    now_ms: now(),
                    id_seed: seed << 64,
                    origin: Origin::Gui,
                };
                let name = cmd.command_name();
                let result = engine.dispatch(cmd, &ctx).map(|o| o.receipt);
                replies.push(Reply {
                    purpose: app_mobile::Purpose::User,
                    name,
                    result: Ok(result),
                });
            }
            let state = engine.state().clone();
            let _ = mobile.replies(replies, &state);
        };
        let configure = UiEvent::Configure {
            width_dp,
            height_dp,
            dark: false,
            font_scale: 1.0,
            locale: None,
            keyboard: false,
        };
        let _ = mobile.event(configure, engine.state());
        let mut steps = vec![vec![Command::OpenMailbox { id: inbox }]];
        if let Some(id) = newest {
            steps.push(vec![Command::OpenThread { id }]);
        }
        steps.push(vec![Command::FocusPane {
            pane: Pane::Sidebar,
        }]);
        for cmds in steps {
            run(&mut engine, &mut mobile, cmds);
            let frame = mobile.frame(engine.state());
            let line = serde_json::to_string(&frame).expect("a frame serializes");
            writeln!(out, "{line}").expect("stdout");
        }
    }
}
