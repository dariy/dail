//! Sorting rules (ADR 0015, SET-4): the evaluator, `save_rule` / `delete_rule` / `enable_rule`
//! / `apply_rules` / `list_rules`, the run on `EnvelopesFetched` with `Origin::Rule`, undo and
//! redo, convergence and the no-forward guarantee.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, T0, mail};
use app_core::{
    Addr, Command, CommandName, Condition, CoreError, Ctx, Effect, Event, Origin, RemoteEnvelope,
    Rule, RuleAction, RuleId, Scope,
};
use proptest::prelude::*;

fn from_rule(sender: &str, then: Vec<RuleAction>) -> Rule {
    Rule {
        name: format!("From {sender}"),
        enabled: true,
        when: Condition::From {
            contains: sender.into(),
        },
        then,
    }
}

fn news(uid: u32, sender: &str) -> RemoteEnvelope {
    let mut envelope = mail(uid, &format!("{uid}@x"), None, "Weekly", false);
    envelope.from = vec![Addr {
        name: Some("The News".into()),
        email: sender.into(),
    }];
    envelope
}

/// Hostinger with INBOX, Archive, Trash, Sent and Reading, each fetched once (empty), so later
/// fetches bring new mail.
fn mailbox() -> Mail {
    let mut m = app_core_mail();
    for path in ["INBOX", "Archive", "Trash", "Sent", "Reading"] {
        m.fetch(path, vec![]);
    }
    m
}

fn app_core_mail() -> Mail {
    Mail::new(
        app_core::ProviderPreset::Hostinger,
        &[
            ("INBOX", None),
            ("Archive", Some(app_core::MailboxRole::Archive)),
            ("Trash", Some(app_core::MailboxRole::Trash)),
            ("Sent", Some(app_core::MailboxRole::Sent)),
            ("Reading", None),
        ],
    )
}

fn save(m: &mut Mail, rule: Rule) -> RuleId {
    let out = m.run(Command::SaveRule { id: None, rule }).unwrap();
    serde_json::from_value(out.receipt.data["id"].clone()).unwrap()
}

fn live(m: &Mail) -> Vec<(RuleId, Rule)> {
    m.engine
        .state()
        .synced
        .settings
        .live_rules()
        .map(|(id, rule)| (id, rule.clone()))
        .collect()
}

fn rule_ctx(seed: u128) -> Ctx {
    Ctx {
        now_ms: T0,
        id_seed: seed << 64,
        origin: Origin::Rule,
    }
}

#[test]
fn conditions_match_case_insensitively_and_compose() {
    let mut envelope = mail(1, "a@x", None, "Your WEEKLY digest", false);
    envelope.list_id = Some("<News.Example.org>".into());
    envelope.has_attachments = true;
    let m = mailbox();
    let inbox = m.mailbox("INBOX");
    let e = envelope.into_envelope(
        app_core::MessageId::from_uuid(uuid::Uuid::from_u128(9)),
        m.account,
        inbox,
        app_core::ThreadId::from_uuid(uuid::Uuid::from_u128(10)),
    );
    let text = |s: &str| s.to_owned();
    let yes = [
        Condition::Subject {
            contains: text("weekly"),
        },
        Condition::From {
            contains: text("BOB"),
        },
        Condition::From {
            contains: text("example.ORG"),
        },
        Condition::ListId {
            contains: text("news.example"),
        },
        Condition::HasAttachment,
        Condition::Mailbox {
            path: text("INBOX"),
        },
        Condition::All { of: vec![] },
        Condition::Not {
            of: Box::new(Condition::Any { of: vec![] }),
        },
    ];
    for c in &yes {
        assert!(c.matches(&e, "INBOX"), "{c:?}");
    }
    let no = [
        Condition::To {
            contains: text("bob"),
        },
        Condition::Cc {
            contains: text("bob"),
        },
        Condition::Mailbox {
            path: text("inbox"),
        },
        Condition::Any { of: vec![] },
        Condition::All {
            of: vec![
                Condition::HasAttachment,
                Condition::Subject {
                    contains: text("monthly"),
                },
            ],
        },
    ];
    for c in &no {
        assert!(!c.matches(&e, "INBOX"), "{c:?}");
    }
    // Scope: a rule without a mailbox test runs on Inbox fetches only.
    let rule = from_rule("bob", vec![RuleAction::Star]);
    assert!(rule.applies(&e, "Reading", Scope::Fetch { inbox: true }));
    assert!(!rule.applies(&e, "Reading", Scope::Fetch { inbox: false }));
    assert!(rule.applies(&e, "Reading", Scope::Explicit));
}

#[test]
fn save_list_enable_delete_with_undo_and_redo() {
    let mut m = mailbox();
    let rule = from_rule("news@", vec![RuleAction::MarkRead]);
    let out = m
        .run(Command::SaveRule {
            id: None,
            rule: rule.clone(),
        })
        .unwrap();
    assert_eq!(out.receipt.data["created"], true);
    let id: RuleId = serde_json::from_value(out.receipt.data["id"].clone()).unwrap();
    assert!(
        m.engine.state().synced.dirty,
        "a local edit marks the document"
    );

    let listed = m.run(Command::ListRules {}).unwrap().receipt.data;
    assert_eq!(listed["dirty"], true);
    assert_eq!(listed["rules"][0]["id"], serde_json::json!(id));
    assert_eq!(
        listed["rules"][0]["untrusted_content"]["name"], "From news@",
        "rule text is returned as untrusted content"
    );

    m.run(Command::EnableRule { id, enabled: false }).unwrap();
    assert!(!live(&m)[0].1.enabled);
    let same = m.run(Command::EnableRule { id, enabled: false }).unwrap();
    assert_eq!(same.receipt.data["changed"], false);
    assert!(same.receipt.undo_label.is_none());

    m.run(Command::DeleteRule { id }).unwrap();
    assert!(live(&m).is_empty());
    assert!(
        m.engine.state().synced.settings.rules[&id].rule.is_none(),
        "a tombstone stays"
    );

    m.run(Command::Undo {}).unwrap(); // delete
    assert!(!live(&m)[0].1.enabled);
    m.run(Command::Undo {}).unwrap(); // disable
    assert_eq!(live(&m), vec![(id, rule.clone())]);
    m.run(Command::Undo {}).unwrap(); // create
    assert!(live(&m).is_empty());
    m.run(Command::Redo {}).unwrap();
    assert_eq!(live(&m), vec![(id, rule.clone())], "redo takes the same ID");

    // Replacing a rule; undo restores the old one.
    let other = from_rule("letters@", vec![RuleAction::Star]);
    m.run(Command::SaveRule {
        id: Some(id),
        rule: other.clone(),
    })
    .unwrap();
    assert_eq!(live(&m), vec![(id, other)]);
    m.run(Command::Undo {}).unwrap();
    assert_eq!(live(&m), vec![(id, rule)]);
}

#[test]
fn every_change_is_stamped_later_than_the_last() {
    let mut m = mailbox();
    let id = save(&mut m, from_rule("a", vec![RuleAction::Star]));
    let first = m.engine.state().synced.settings.rules[&id].updated_ms;
    // Same clock: the next change still wins the merge.
    m.engine
        .dispatch(Command::EnableRule { id, enabled: false }, &{
            let mut ctx = crate::support::ctx(90);
            ctx.now_ms = 0;
            ctx
        })
        .unwrap();
    let second = m.engine.state().synced.settings.rules[&id].updated_ms;
    assert!(second > first);
    assert!(m.engine.state().synced.settings.updated_ms >= second);
}

#[test]
fn every_error_is_reported() {
    let mut m = mailbox();
    let unknown = RuleId::from_uuid(uuid::Uuid::from_u128(77));
    let mut empty_name = from_rule("x", vec![RuleAction::Star]);
    empty_name.name = " ".into();
    assert!(matches!(
        m.run(Command::SaveRule { id: None, rule: empty_name }),
        Err(CoreError::InvalidArgument { field, .. }) if field == "rule.name"
    ));
    let no_action = from_rule("x", vec![]);
    assert!(matches!(
        m.run(Command::SaveRule { id: None, rule: no_action }),
        Err(CoreError::InvalidArgument { field, .. }) if field == "rule.then"
    ));
    let empty_test = from_rule("", vec![RuleAction::Star]);
    assert!(matches!(
        m.run(Command::SaveRule { id: None, rule: empty_test }),
        Err(CoreError::InvalidArgument { field, .. }) if field == "rule.when"
    ));
    let ok = from_rule("x", vec![RuleAction::Star]);
    for cmd in [
        Command::SaveRule {
            id: Some(unknown),
            rule: ok,
        },
        Command::DeleteRule { id: unknown },
        Command::EnableRule {
            id: unknown,
            enabled: true,
        },
    ] {
        assert!(matches!(
            m.run(cmd),
            Err(CoreError::NotFound { entity, .. }) if entity == "rule"
        ));
    }
    assert!(matches!(
        m.run(Command::ApplyRules {
            mailbox: app_core::MailboxId::from_uuid(uuid::Uuid::from_u128(5)),
        }),
        Err(CoreError::NotFound { entity, .. }) if entity == "mailbox"
    ));
    // A deleted rule can be neither deleted nor toggled again.
    let id = save(&mut m, from_rule("y", vec![RuleAction::Star]));
    m.run(Command::DeleteRule { id }).unwrap();
    assert!(matches!(
        m.run(Command::DeleteRule { id }),
        Err(CoreError::NotFound { .. })
    ));
    assert!(
        m.engine
            .state()
            .synced
            .settings
            .live_rules()
            .next()
            .is_none()
    );
}

#[test]
fn rules_sort_new_mail_on_fetch_and_each_action_undoes() {
    let mut m = mailbox();
    save(
        &mut m,
        from_rule(
            "NEWS@",
            vec![
                RuleAction::MarkRead,
                RuleAction::Move {
                    to: "Reading".into(),
                },
            ],
        ),
    );
    let out = m.fetch(
        "INBOX",
        vec![news(1, "news@example.org"), news(2, "friend@example.org")],
    );
    let report = &out.receipt.data["rules"];
    assert_eq!(report["matched"], 1);
    assert_eq!(
        report["commands"],
        serde_json::json!(["mark_read", "move_threads"])
    );
    assert!(
        out.effects
            .iter()
            .any(|e| matches!(e, Effect::RunRemoteOp { .. })),
        "the actions reach the server as ordinary ops"
    );
    let sorted = m.message("INBOX", 1);
    let other = m.message("INBOX", 2);
    let reading = m.mailbox("Reading");
    let state = m.engine.state();
    assert_eq!(state.messages[&sorted].location(), reading);
    assert!(state.messages[&sorted].flags.seen);
    assert_eq!(state.messages[&other].location(), m.mailbox("INBOX"));
    assert!(!state.messages[&other].flags.seen);
    assert_eq!(
        m.engine.history().undo_label(),
        Some("Rule: Move (1 message)")
    );

    m.run(Command::Undo {}).unwrap();
    assert_eq!(
        m.engine.state().messages[&sorted].location(),
        m.mailbox("INBOX")
    );
    m.run(Command::Undo {}).unwrap();
    assert!(!m.engine.state().messages[&sorted].flags.seen);
    m.run(Command::Redo {}).unwrap();
    m.run(Command::Redo {}).unwrap();
    assert_eq!(m.engine.state().messages[&sorted].location(), reading);
}

#[test]
fn no_rules_run_on_a_first_fetch_or_outside_the_inbox() {
    let mut m = app_core_mail();
    save(&mut m, from_rule("news@", vec![RuleAction::Star]));
    // First fetch of INBOX: the whole mailbox, not new mail.
    let out = m.fetch("INBOX", vec![news(1, "news@example.org")]);
    assert!(out.receipt.data.get("rules").is_none());
    assert!(
        !m.engine.state().messages[&m.message("INBOX", 1)]
            .flags
            .flagged
    );
    // Reading: new mail, but a rule without a mailbox test runs on the Inbox only.
    m.fetch("Reading", vec![]);
    m.fetch("Reading", vec![news(5, "news@example.org")]);
    assert!(
        !m.engine.state().messages[&m.message("Reading", 5)]
            .flags
            .flagged
    );
    // With a mailbox test it runs there.
    save(
        &mut m,
        Rule {
            name: "Star in Reading".into(),
            enabled: true,
            when: Condition::Mailbox {
                path: "Reading".into(),
            },
            then: vec![RuleAction::Star],
        },
    );
    m.fetch("Reading", vec![news(6, "x@example.org")]);
    assert!(
        m.engine.state().messages[&m.message("Reading", 6)]
            .flags
            .flagged
    );
    // A disabled rule never runs.
    let id = live(&m)
        .into_iter()
        .find(|(_, r)| r.name == "Star in Reading")
        .unwrap()
        .0;
    m.run(Command::EnableRule { id, enabled: false }).unwrap();
    m.fetch("Reading", vec![news(7, "x@example.org")]);
    assert!(
        !m.engine.state().messages[&m.message("Reading", 7)]
            .flags
            .flagged
    );
}

#[test]
fn a_moved_message_is_not_sorted_again() {
    let mut m = mailbox();
    // Two rules that would send mail back and forth if moved mail were sorted again.
    save(
        &mut m,
        Rule {
            name: "Inbox to Reading".into(),
            enabled: true,
            when: Condition::Mailbox {
                path: "INBOX".into(),
            },
            then: vec![RuleAction::Move {
                to: "Reading".into(),
            }],
        },
    );
    save(
        &mut m,
        Rule {
            name: "Reading to Inbox".into(),
            enabled: true,
            when: Condition::Mailbox {
                path: "Reading".into(),
            },
            then: vec![RuleAction::Move { to: "INBOX".into() }],
        },
    );
    let out = m.fetch("INBOX", vec![news(1, "a@example.org")]);
    let Some(Effect::RunRemoteOp { op, .. }) = out
        .effects
        .iter()
        .find(|e| matches!(e, Effect::RunRemoteOp { .. }))
    else {
        panic!("no move op")
    };
    let op = *op;
    m.feed(Event::RemoteOpSucceeded { op });
    // The fetch of Reading finds the moved message: it is located, not added.
    let mut moved = news(9, "a@example.org");
    moved.message_id_hdr = Some("1@x".into());
    let out = m.fetch("Reading", vec![moved]);
    assert_eq!(out.receipt.data["located"], 1);
    assert!(out.receipt.data.get("rules").is_none());
    assert_eq!(m.engine.state().messages.len(), 1);
}

#[test]
fn apply_rules_is_one_undo_step_and_a_second_run_does_nothing() {
    let mut m = mailbox();
    m.fetch(
        "INBOX",
        vec![
            news(1, "news@example.org"),
            news(2, "news@example.org"),
            news(3, "friend@example.org"),
        ],
    );
    save(
        &mut m,
        from_rule("news@", vec![RuleAction::Star, RuleAction::Archive]),
    );
    let inbox = m.mailbox("INBOX");
    let before = m.engine.state().messages.clone();
    let out = m.run(Command::ApplyRules { mailbox: inbox }).unwrap();
    assert_eq!(out.receipt.data["matched"], 2);
    assert_eq!(
        out.receipt.data["commands"],
        serde_json::json!(["star", "archive_threads"])
    );
    let after = m.engine.state().messages.clone();
    let archive = m.mailbox("Archive");
    assert_eq!(
        after.values().filter(|e| e.location() == archive).count(),
        2
    );

    // Archived messages are no longer in INBOX; in Archive the rule wants nothing more.
    let again = m.run(Command::ApplyRules { mailbox: archive }).unwrap();
    assert_eq!(again.receipt.data["commands"], serde_json::json!([]));
    assert!(again.receipt.undo_label.is_none());

    m.run(Command::Undo {}).unwrap();
    let undone = m.engine.state().messages.clone();
    for (id, e) in &before {
        assert_eq!(undone[id].location(), e.location());
        assert_eq!(undone[id].flags, e.flags);
    }
    m.run(Command::Redo {}).unwrap();
    for (id, e) in &after {
        assert_eq!(m.engine.state().messages[id].location(), e.location());
        assert_eq!(m.engine.state().messages[id].flags, e.flags);
    }
}

#[test]
fn two_devices_with_the_same_rule_converge() {
    let run = |order: &[u32]| {
        let mut m = mailbox();
        save(
            &mut m,
            from_rule(
                "news@",
                vec![
                    RuleAction::MarkRead,
                    RuleAction::Move {
                        to: "Reading".into(),
                    },
                    // A later move loses: one place per message.
                    RuleAction::Archive,
                ],
            ),
        );
        for uid in order {
            m.fetch("INBOX", vec![news(*uid, "news@example.org")]);
        }
        let reading = m.mailbox("Reading");
        let mut sorted: Vec<(String, bool)> = m
            .engine
            .state()
            .messages
            .values()
            .map(|e| (e.subject.clone(), e.location() == reading && e.flags.seen))
            .collect();
        sorted.sort();
        sorted
    };
    let a = run(&[1, 2]);
    assert_eq!(a, run(&[2, 1]));
    assert!(a.iter().all(|(_, sorted)| *sorted));
}

#[test]
fn rules_run_only_the_rule_actions() {
    let mut m = mailbox();
    m.fetch("INBOX", vec![news(1, "news@example.org")]);
    let message = m.message("INBOX", 1);
    let refused = [
        Command::Forward {
            message,
            body: None,
        },
        Command::Reply {
            message,
            body: None,
        },
        Command::CreateDraft {
            account: m.account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
        },
        Command::SendDraft {
            id: app_core::DraftId::from_uuid(uuid::Uuid::from_u128(3)),
        },
        Command::PushSettings {},
        Command::SaveRule {
            id: None,
            rule: from_rule("x", vec![RuleAction::Star]),
        },
        Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![message],
        },
    ];
    for cmd in refused {
        let name = cmd.name();
        assert!(
            matches!(
                m.engine.dispatch(cmd, &rule_ctx(40)),
                Err(CoreError::OriginNotAllowed { origin, .. }) if origin == "rule"
            ),
            "{name}"
        );
    }
    m.engine
        .dispatch(
            Command::Star {
                ids: Vec::new(),
                messages: vec![message],
            },
            &rule_ctx(41),
        )
        .unwrap();
    // The format has no forward or reply action at all.
    for action in [
        r#"{"action":"forward","to":"x@example.org"}"#,
        r#"{"action":"reply"}"#,
    ] {
        assert!(serde_json::from_str::<RuleAction>(action).is_err());
    }
    assert_eq!(serde_json::to_value(Origin::Rule).unwrap(), "rule");
    assert!(!Origin::Rule.is_user() && !Origin::Rule.is_agent());
}

#[test]
fn agents_may_list_save_and_apply_rules() {
    for name in [
        CommandName::ListRules,
        CommandName::SaveRule,
        CommandName::DeleteRule,
        CommandName::EnableRule,
        CommandName::ApplyRules,
    ] {
        assert_eq!(name.agent_access(), app_core::AgentAccess::Allowed);
    }
    let mut m = mailbox();
    let mcp = Ctx {
        now_ms: T0,
        id_seed: 60 << 64,
        origin: Origin::Mcp,
    };
    m.engine
        .dispatch(
            Command::SaveRule {
                id: None,
                rule: from_rule("x", vec![RuleAction::Star]),
            },
            &mcp,
        )
        .unwrap();
    assert_eq!(live(&m).len(), 1);
}

#[derive(Debug, Clone)]
enum Edit {
    Save(u8),
    Toggle(u8),
    Delete(u8),
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (0_u8..3).prop_map(Edit::Save),
        (0_u8..3).prop_map(Edit::Toggle),
        (0_u8..3).prop_map(Edit::Delete),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any sequence of rule edits undoes back to no rules and redoes to the same rules.
    #[test]
    fn rule_edits_undo_and_redo_exactly(edits in prop::collection::vec(edit(), 1..12)) {
        let mut m = mailbox();
        let mut ids: Vec<RuleId> = Vec::new();
        let mut steps = 0;
        for e in edits {
            let cmd = match e {
                Edit::Save(n) => Command::SaveRule {
                    id: ids.get(usize::from(n)).copied(),
                    rule: from_rule(&format!("s{n}"), vec![RuleAction::Star]),
                },
                Edit::Toggle(n) => match ids.get(usize::from(n)) {
                    Some(&id) => Command::EnableRule { id, enabled: steps % 2 == 0 },
                    None => continue,
                },
                Edit::Delete(n) => match ids.get(usize::from(n)) {
                    Some(&id) => Command::DeleteRule { id },
                    None => continue,
                },
            };
            if let Ok(out) = m.run(cmd) {
                if let Some(id) = out.receipt.data.get("id").and_then(|v| serde_json::from_value(v.clone()).ok())
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
                if out.receipt.undo_label.is_some() {
                    steps += 1;
                }
            }
        }
        let done = live(&m);
        for _ in 0..steps {
            m.run(Command::Undo {}).unwrap();
        }
        prop_assert!(live(&m).is_empty());
        for _ in 0..steps {
            m.run(Command::Redo {}).unwrap();
        }
        prop_assert_eq!(live(&m), done);
    }
}

#[test]
fn a_rule_run_on_fetch_keeps_the_users_redo() {
    let mut m = mailbox();
    save(&mut m, from_rule("news@", vec![RuleAction::Star]));
    m.fetch("INBOX", vec![news(1, "friend@example.org")]);
    let first = m.message("INBOX", 1);
    m.run(Command::MarkRead {
        ids: Vec::new(),
        messages: vec![first],
    })
    .unwrap();
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.history().redo_len(), 1);
    m.fetch("INBOX", vec![news(2, "news@example.org")]);
    assert!(
        m.engine.state().messages[&m.message("INBOX", 2)]
            .flags
            .flagged
    );
    assert_eq!(
        m.engine.history().redo_len(),
        1,
        "a background rule run keeps redo"
    );
    assert_eq!(
        m.engine.history().undo_label(),
        Some("Rule: Star (1 message)")
    );
    m.run(Command::Undo {}).unwrap(); // the rule's star
    m.run(Command::Redo {}).unwrap();
    assert!(
        m.engine.state().messages[&m.message("INBOX", 2)]
            .flags
            .flagged
    );
}

#[test]
fn a_uidvalidity_reset_runs_no_rules() {
    let mut m = mailbox();
    save(&mut m, from_rule("news@", vec![RuleAction::Star]));
    let inbox = m.mailbox("INBOX");
    let out = m.feed(Event::EnvelopesFetched {
        account: m.account,
        mailbox: inbox,
        uid_validity: 8, // was 7
        uid_next: Some(2),
        total: 1,
        unread: 1,
        envelopes: vec![news(1, "news@example.org")],
    });
    assert_eq!(out.receipt.data["uid_validity_reset"], true);
    assert!(out.receipt.data.get("rules").is_none());
    assert!(
        !m.engine.state().messages[&m.message("INBOX", 1)]
            .flags
            .flagged
    );
}

#[test]
fn save_rule_refuses_more_than_the_limit() {
    let mut m = mailbox();
    for n in 0..app_core::synced::MAX_RULES {
        save(&mut m, from_rule(&format!("s{n}"), vec![RuleAction::Star]));
    }
    assert!(matches!(
        m.run(Command::SaveRule {
            id: None,
            rule: from_rule("one more", vec![RuleAction::Star]),
        }),
        Err(CoreError::InvalidArgument { field, .. }) if field == "rule"
    ));
    // Replacing an existing rule is still fine.
    let (id, _) = live(&m)[0].clone();
    m.run(Command::SaveRule {
        id: Some(id),
        rule: from_rule("replaced", vec![RuleAction::Star]),
    })
    .unwrap();
}

#[test]
fn rule_edits_are_persisted_with_the_dirty_flag() {
    let mut m = mailbox();
    let out = m
        .run(Command::SaveRule {
            id: None,
            rule: from_rule("x", vec![RuleAction::Star]),
        })
        .unwrap();
    let persisted = out.effects.iter().find_map(|e| match e {
        Effect::PersistState {
            delta: app_core::PersistDelta::Synced(local),
        } => Some(local.clone()),
        _ => None,
    });
    let local = persisted.unwrap();
    assert!(local.dirty);
    assert_eq!(local.settings.live_rules().count(), 1);
}
