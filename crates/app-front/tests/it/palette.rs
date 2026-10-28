//! The palette's logic: registry parity (every command reachable through its flow), arguments
//! from the view, fuzzy ranking and the palette state.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Addr, AppState, Command, CommandName, Ctx, Engine, MailboxRole, Origin, Pane};
use app_front::palette::{self, AskKind, Choice, Flow, Palette, Target};

use crate::support::{self, inbox_of, synced_state};

fn dispatch(engine: &mut Engine, seed: u128, cmd: Command) -> app_core::CommandReceipt {
    let ctx = Ctx {
        now_ms: support::NOW,
        id_seed: seed << 64,
        origin: Origin::Cli,
    };
    engine.dispatch(cmd, &ctx).unwrap().receipt
}

/// A state where every command has something to act on: an open thread, a pending op, an open
/// draft with an attachment, a scheduled send and an agent's send request.
pub(crate) fn rich_state() -> AppState {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let account = *state.accounts.keys().next().unwrap();
    let mut engine = Engine::new(state);
    dispatch(&mut engine, 501, Command::OpenMailbox { id: inbox });
    dispatch(&mut engine, 502, Command::OpenThread { id: threads[0] });
    dispatch(
        &mut engine,
        503,
        Command::MarkRead {
            ids: vec![threads[0]],
            messages: Vec::new(),
        },
    );
    let draft = |engine: &mut Engine, seed, to: Vec<Addr>| {
        let receipt = dispatch(
            engine,
            seed,
            Command::CreateDraft {
                account,
                to,
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Hi".into(),
                body: String::new(),
            },
        );
        serde_json::from_value(receipt.data["id"].clone()).unwrap()
    };
    let scheduled = draft(
        &mut engine,
        504,
        vec![Addr {
            name: None,
            email: "bob@example.org".into(),
        }],
    );
    dispatch(&mut engine, 505, Command::SendDraft { id: scheduled });
    let asked = draft(
        &mut engine,
        510,
        vec![Addr {
            name: None,
            email: "carol@example.org".into(),
        }],
    );
    let agent = Ctx {
        now_ms: support::NOW,
        id_seed: 511 << 64,
        origin: Origin::Mcp,
    };
    engine
        .dispatch(Command::SendDraft { id: asked }, &agent)
        .unwrap();
    let open = draft(&mut engine, 506, Vec::new());
    dispatch(
        &mut engine,
        507,
        Command::AttachFile {
            draft: open,
            path: "/tmp/a.txt".into(),
            index: None,
        },
    );
    dispatch(&mut engine, 508, Command::OpenComposer { id: open });
    dispatch(&mut engine, 509, Command::FocusPane { pane: Pane::List });
    engine.state().clone()
}

/// Follows a flow, answering every step with its first choice (`text` for free text).
fn walk(state: &AppState, mut flow: Flow, text: &str) -> Flow {
    for _ in 0..5 {
        let Flow::Ask(step) = &flow else {
            return flow;
        };
        let choice = match &step.kind {
            AskKind::Text(_) => Choice::Text(text.to_owned()),
            kind => match palette::ask_rows(state, kind, "")
                .first()
                .map(|r| r.target.clone())
            {
                Some(Target::Choice(choice)) => choice,
                other => panic!("{kind:?} offered {other:?}"),
            },
        };
        flow = step.answer(state, choice);
    }
    flow
}

#[test]
fn every_registry_command_is_reachable_from_the_palette() {
    let state = rich_state();
    let roots: Vec<Target> = palette::root_rows(&state)
        .into_iter()
        .map(|r| r.target)
        .collect();
    for &name in CommandName::ALL {
        if !palette::offered(name) {
            assert!(
                matches!(palette::flow(name, &state), Flow::Hidden),
                "{name:?}"
            );
            continue;
        }
        if (name.spec().available)(&state) {
            assert!(
                roots.contains(&Target::Command(name)),
                "{name:?} is not listed"
            );
        }
        let text = if matches!(name, CommandName::SetSetting | CommandName::SetSyncWindow) {
            "300"
        } else {
            "plans"
        };
        match walk(&state, palette::flow(name, &state), text) {
            Flow::Run(cmd) => {
                let expected = match name {
                    // Drafts are edited in the composer.
                    CommandName::UpdateDraft => CommandName::OpenComposer,
                    name => name,
                };
                assert_eq!(cmd.command_name(), expected, "{name:?} built {cmd:?}");
            }
            Flow::Wizard => assert_eq!(name, CommandName::AddAccount),
            Flow::Password(_) => assert_eq!(name, CommandName::SetPassword),
            other => panic!("{name:?} ended in {other:?}"),
        }
    }
}

#[test]
fn arguments_come_from_the_view_before_any_picker() {
    let state = rich_state();
    let thread = state.view.thread.unwrap();
    let Flow::Run(cmd) = palette::flow(CommandName::ArchiveThreads, &state) else {
        panic!("asked although a thread is open");
    };
    assert_eq!(
        cmd,
        Command::ArchiveThreads {
            ids: vec![thread],
            messages: Vec::new()
        }
    );
    let Flow::Ask(step) = palette::flow(CommandName::MoveThreads, &state) else {
        panic!("move needs a target mailbox");
    };
    let account = state.threads[&thread].account;
    assert_eq!(step.kind, AskKind::Mailboxes(Some(account)));
    let Flow::Ask(confirm) = walk_one(&state, CommandName::RemoveAccount) else {
        panic!("removal must be confirmed");
    };
    assert!(matches!(confirm.kind, AskKind::Confirm(_)));
    assert!(matches!(
        confirm.answer(&state, Choice::Text(String::new())),
        Flow::Hidden
    ));
}

fn walk_one(state: &AppState, name: CommandName) -> Flow {
    let Flow::Ask(step) = palette::flow(name, state) else {
        panic!("{name:?} asks nothing");
    };
    let rows = palette::ask_rows(state, &step.kind, "");
    let Some(Target::Choice(choice)) = rows.first().map(|r| r.target.clone()) else {
        panic!("no choices");
    };
    step.answer(state, choice)
}

#[test]
fn fuzzy_filtering_ranks_the_best_match_first() {
    let state = rich_state();
    let mut matcher = nucleo_matcher::Matcher::new(nucleo_matcher::Config::DEFAULT);
    let rows = palette::filter(palette::root_rows(&state), "arch", &mut matcher);
    assert_eq!(rows[0].target, Target::Command(CommandName::ArchiveThreads));
    let rows = palette::filter(palette::root_rows(&state), "go sent", &mut matcher);
    let sent = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Sent))
        .unwrap()
        .id;
    assert_eq!(rows[0].target, Target::Mailbox(sent));
    assert!(palette::filter(palette::root_rows(&state), "zzqqxx", &mut matcher).is_empty());
    // "Send" matches "compose" only through its category: the command titled Compose wins.
    let rows = palette::filter(palette::root_rows(&state), "compose", &mut matcher);
    assert_eq!(rows[0].target, Target::Command(CommandName::CreateDraft));
}

#[test]
fn choosing_runs_the_command_then_closes_and_a_second_step_waits() {
    let state = rich_state();
    let mut p = Palette::default();
    p.set_query("archive");
    let rows = p.rows(&state);
    let chosen = p.choose(&state, rows[0].target.clone());
    assert_eq!(
        chosen.commands,
        [
            Command::ArchiveThreads {
                ids: vec![state.view.thread.unwrap()],
                messages: Vec::new()
            },
            Command::ClosePalette {}
        ]
    );
    assert_eq!(p.query(), "");
    let chosen = p.choose(&state, Target::Command(CommandName::MoveThreads));
    assert!(chosen.commands.is_empty(), "asks for the mailbox first");
    assert_eq!(p.step().unwrap().prompt, "Move to…");
    assert!(p.back(), "Escape leaves the second step");
    assert!(!p.back(), "at the top level Escape closes the palette");
    let chosen = p.choose(&state, Target::Command(CommandName::AddAccount));
    assert!(chosen.wizard);
    assert_eq!(chosen.commands, [Command::ClosePalette {}]);
}

#[test]
fn typed_text_offers_a_search_and_the_cursor_stays_in_range() {
    let state = rich_state();
    let mut p = Palette::default();
    p.set_query("from:carol");
    let rows = p.rows(&state);
    assert_eq!(
        rows.last().unwrap().target,
        Target::Search("from:carol".into())
    );
    p.move_by(50, rows.len());
    assert_eq!(p.selected(), rows.len() - 1);
    p.move_by(-100, rows.len());
    assert_eq!(p.selected(), 0);
}

#[test]
fn commands_used_lately_come_first_and_show_their_help() {
    let state = rich_state();
    let mut palette = palette::Palette::default();
    let first = palette.rows(&state);
    let position = |rows: &[palette::Row], name: CommandName| {
        rows.iter()
            .position(|r| r.target == Target::Command(name))
            .unwrap()
    };
    let before = position(&first, CommandName::GetSettings);
    assert!(before > 0);
    palette.choose(&state, Target::Command(CommandName::GetSettings));
    let rows = palette.rows(&state);
    assert_eq!(position(&rows, CommandName::GetSettings), 0, "used: first");
    // A typed query that names a command exactly still finds that command first.
    palette.set_query("archive");
    let rows = palette.rows(&state);
    assert_eq!(rows[0].target, Target::Command(CommandName::ArchiveThreads));
    let help = palette::Palette::help(&rows, 0).unwrap();
    assert!(help.starts_with("Archive"), "{help}");
    assert!(!help.contains(". "), "one sentence: {help}");
}

#[test]
fn an_empty_query_groups_the_rows_under_captions() {
    let state = rich_state();
    let mut palette = palette::Palette::default();
    let rows = palette.rows(&state);
    let sections = palette.sections(&rows);
    let titles: Vec<&str> = sections.iter().map(|s| s.title).collect();
    assert_eq!(titles.first(), Some(&"Mail"));
    assert!(
        titles.contains(&"Mailboxes") && titles.contains(&"Threads"),
        "{titles:?}"
    );
    // The sections cover every row once, in order.
    assert_eq!(sections.first().map(|s| s.start), Some(0));
    assert_eq!(sections.last().map(|s| s.end), Some(rows.len()));
    for pair in sections.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
        assert_ne!(pair[0].title, pair[1].title);
    }
    // A command used before heads a *Recent* section.
    palette.choose(&state, Target::Command(CommandName::GetSettings));
    let rows = palette.rows(&state);
    let sections = palette.sections(&rows);
    assert_eq!(sections[0].title, "Recent");
    assert_eq!((sections[0].start, sections[0].end), (0, 1));
    // With a query the rows are one ranked list.
    palette.set_query("arch");
    let rows = palette.rows(&state);
    assert!(palette.sections(&rows).is_empty());
}

#[test]
fn typed_themes_become_set_theme() {
    use app_core::ThemeMode;
    let theme = |mode, light: Option<&str>, dark: Option<&str>| Command::SetTheme {
        mode,
        light: light.map(str::to_owned),
        dark: dark.map(str::to_owned),
    };
    assert_eq!(
        palette::theme_command("dark"),
        theme(Some(ThemeMode::Dark), None, None)
    );
    assert_eq!(
        palette::theme_command("dark dail-dark-hc"),
        theme(Some(ThemeMode::Dark), None, Some("dail-dark-hc"))
    );
    assert_eq!(
        palette::theme_command("Light dail-light-hc"),
        theme(Some(ThemeMode::Light), Some("dail-light-hc"), None)
    );
    assert_eq!(
        palette::theme_command("dail-light-hc"),
        theme(None, Some("dail-light-hc"), None)
    );
    assert_eq!(
        palette::theme_command("system"),
        theme(Some(ThemeMode::System), None, None)
    );
}
