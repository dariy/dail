//! The Rules screen (ADR 0015, SET-5): rows and summaries of the sorting rules, the add and edit
//! steps, and *Make a rule from this message*.

// reason: helpers and `let … else` arms may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AppState, Command, Condition, Rule, RuleAction};
use app_front::palette::{self, Choice, Flow, Target};
use app_front::rules::{panel, summary};

use crate::palette::rich_state;
use crate::support::{inbox_of, synced_state};

/// Answers the current step with `choice`.
fn answer(state: &AppState, flow: Flow, choice: Choice) -> Flow {
    let Flow::Ask(step) = flow else {
        panic!("expected a step, got {flow:?}");
    };
    step.answer(state, choice)
}

fn titles(state: &AppState, flow: &Flow) -> Vec<String> {
    let Flow::Ask(step) = flow else {
        panic!("expected a step");
    };
    palette::ask_rows(state, &step.kind, "")
        .into_iter()
        .map(|r| r.title)
        .collect()
}

#[test]
fn the_summary_reads_the_condition_and_the_actions() {
    let rule = Rule {
        name: "x".into(),
        enabled: true,
        when: Condition::All {
            of: vec![
                Condition::From {
                    contains: "news@".into(),
                },
                Condition::Not {
                    of: Box::new(Condition::HasAttachment),
                },
            ],
        },
        then: vec![
            RuleAction::Move {
                to: "Reading".into(),
            },
            RuleAction::MarkRead,
        ],
    };
    assert_eq!(
        summary(&rule),
        "sender contains “news@” and not (has an attachment) → move to Reading, mark read"
    );
}

#[test]
fn rule_text_is_cleaned() {
    let mut state = synced_state();
    let mut engine = app_core::Engine::new(state.clone());
    let ctx = app_core::Ctx {
        now_ms: 1,
        id_seed: 9 << 64,
        origin: app_core::Origin::Gui,
    };
    engine
        .dispatch(
            Command::SaveRule {
                id: None,
                rule: Rule {
                    name: "evil\u{1b}]52;c;x\u{7}\u{202e}name".into(),
                    enabled: false,
                    when: Condition::Subject {
                        contains: "a\nb".into(),
                    },
                    then: vec![RuleAction::Star],
                },
            },
            &ctx,
        )
        .unwrap();
    state = engine.state().clone();
    let p = panel(&state);
    assert!(p.dirty, "an edit waits for the push");
    let row = &p.rows[0];
    assert!(!row.enabled);
    assert!(!row.name.contains('\u{1b}') && !row.name.contains('\u{202e}'));
    assert!(!row.summary.contains('\n'));
}

#[test]
fn add_a_rule_asks_the_test_the_text_and_the_action() {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let flow = palette::new_rule();
    assert_eq!(titles(&state, &flow)[0], "Sender");
    let flow = answer(&state, flow, Choice::Index(3));
    let flow = answer(&state, flow, Choice::Text(" invoice ".into()));
    assert_eq!(titles(&state, &flow)[0], "Move to a folder…");
    let flow = answer(&state, flow, Choice::Index(0));
    let Flow::Run(Command::SaveRule { id: None, rule }) =
        answer(&state, flow, Choice::Mailbox(inbox))
    else {
        panic!("save_rule");
    };
    assert_eq!(
        rule.when,
        Condition::Subject {
            contains: "invoice".into()
        }
    );
    assert_eq!(
        rule.then,
        [RuleAction::Move {
            to: state.mailboxes[&inbox].path.clone()
        }]
    );
    assert!(rule.enabled);
    assert_eq!(rule.name, summary(&rule));
}

#[test]
fn edit_renames_replaces_or_deletes() {
    let state = rich_state();
    let (id, rule) = state
        .synced
        .settings
        .live_rules()
        .map(|(id, r)| (id, r.clone()))
        .next()
        .unwrap();
    let flow = answer(&state, palette::edit_rule(&state, id), Choice::Index(0));
    let Flow::Run(Command::SaveRule {
        id: Some(saved),
        rule: renamed,
    }) = answer(&state, flow, Choice::Text("Newsletters".into()))
    else {
        panic!("save_rule");
    };
    assert_eq!(saved, id);
    assert_eq!(renamed.name, "Newsletters");
    assert_eq!(renamed.when, rule.when);
    let flow = answer(&state, palette::edit_rule(&state, id), Choice::Index(3));
    let Flow::Run(Command::SaveRule { rule: changed, .. }) = answer(&state, flow, Choice::Index(1))
    else {
        panic!("save_rule");
    };
    assert_eq!(changed.then, [RuleAction::Archive]);
    assert_eq!(changed.name, rule.name);
    let Flow::Run(cmd) = answer(&state, palette::edit_rule(&state, id), Choice::Index(4)) else {
        panic!("delete_rule");
    };
    assert_eq!(cmd, Command::DeleteRule { id });
}

#[test]
fn make_a_rule_from_this_message_prefills_the_sender() {
    let state = rich_state();
    let rows = palette::root_rows(&state);
    assert!(rows.iter().any(|r| r.target == Target::RuleFromMessage));
    let message = palette::rule_source(&state).unwrap();
    let sender = state.messages[&message].from[0].email.clone();
    let mut p = palette::Palette::default();
    let chosen = p.choose(&state, Target::RuleFromMessage);
    assert!(chosen.commands.is_empty(), "a step opens");
    let rows = p.rows(&state);
    assert!(rows[0].detail.contains(&sender), "{rows:?}");
    let Target::Choice(first) = rows[0].target.clone() else {
        panic!("a choice");
    };
    p.choose(&state, Target::Choice(first));
    let star = p
        .rows(&state)
        .into_iter()
        .find(|r| r.title == "Star")
        .unwrap();
    let chosen = p.choose(&state, star.target);
    let Some(Command::SaveRule { id: None, rule }) = chosen.commands.first() else {
        panic!("save_rule, got {:?}", chosen.commands);
    };
    assert_eq!(rule.when, Condition::From { contains: sender });
    assert_eq!(rule.then, [RuleAction::Star]);
}

#[test]
fn make_a_rule_needs_a_message() {
    let state = synced_state();
    assert!(palette::rule_source(&state).is_none());
    assert!(
        !palette::root_rows(&state)
            .iter()
            .any(|r| r.target == Target::RuleFromMessage)
    );
    assert!(matches!(palette::rule_from_message(&state), Flow::Hidden));
}

/// A rule with a nested condition or several actions offers no condition or action step,
/// so editing it here cannot drop parts of it; a simple rule still does.
#[test]
fn a_complex_rule_offers_only_rename_toggle_and_delete() {
    let mut engine = app_core::Engine::new(synced_state());
    let ctx = app_core::Ctx {
        now_ms: 1,
        id_seed: 9 << 64,
        origin: app_core::Origin::Gui,
    };
    let leaf = Condition::From {
        contains: "a@".into(),
    };
    for (when, then) in [
        (
            Condition::All {
                of: vec![leaf.clone(), Condition::HasAttachment],
            },
            vec![RuleAction::Star],
        ),
        (leaf.clone(), vec![RuleAction::Star, RuleAction::MarkRead]),
        (
            leaf.clone(),
            vec![RuleAction::AddLabel { label: "L".into() }],
        ),
        (leaf.clone(), vec![RuleAction::Star]),
    ] {
        engine
            .dispatch(
                Command::SaveRule {
                    id: None,
                    rule: Rule {
                        name: "r".into(),
                        enabled: true,
                        when,
                        then,
                    },
                },
                &ctx,
            )
            .unwrap();
    }
    let state = engine.state().clone();
    let ids: Vec<_> = state
        .synced
        .settings
        .live_rules()
        .map(|(id, r)| (id, r.clone()))
        .collect();
    for (id, rule) in ids {
        let flow = palette::edit_rule(&state, id);
        let Flow::Ask(step) = &flow else { panic!() };
        let offered = titles(&state, &flow);
        if palette::editable_here(&rule) {
            assert_eq!(
                offered,
                ["Name", "Turn off", "Condition", "Action", "Delete"]
            );
        } else {
            assert_eq!(offered, ["Name", "Turn off", "Delete"], "{rule:?}");
            assert!(step.prompt.contains("too complex"), "{}", step.prompt);
        }
    }
    let simple = state
        .synced
        .settings
        .live_rules()
        .filter(|(_, r)| palette::editable_here(r))
        .count();
    assert_eq!(simple, 1);
}

/// Placeholders inside untrusted values are never expanded (one pass over the template).
#[test]
fn placeholders_in_rule_text_stay_literal() {
    let rule = Rule {
        name: "x".into(),
        enabled: true,
        when: Condition::Subject {
            contains: "{then} {to} {when}".into(),
        },
        then: vec![RuleAction::Move {
            to: "{text}".into(),
        }],
    };
    assert_eq!(
        summary(&rule),
        "subject contains “{then} {to} {when}” → move to {text}"
    );
}
