//! The synced settings document (ADR 0015, SET-1): validation, tombstones and the merge laws.

use std::collections::BTreeMap;

use app_core::synced::{MAX_CONDITION_DEPTH, MAX_TEXT_CHARS, TOMBSTONE_TTL_MS};
use app_core::{
    Condition, Rule, RuleAction, RuleEntry, RuleId, SETTINGS_MAX_BYTES, SettingsError, Stamped,
    SyncedSettings, SyncedTheme,
};
use proptest::prelude::*;

fn rid(n: u128) -> RuleId {
    RuleId::from_uuid(uuid::Uuid::from_u128(n))
}

fn rule(name: &str) -> Rule {
    Rule {
        name: name.into(),
        enabled: true,
        when: Condition::From {
            contains: "news".into(),
        },
        then: vec![RuleAction::MarkRead],
    }
}

fn doc_with(rule_entry: RuleEntry) -> SyncedSettings {
    let mut doc = SyncedSettings::new();
    doc.rules.insert(rid(1), rule_entry);
    doc
}

#[test]
fn newer_field_and_rule_win() {
    let mut a = SyncedSettings::new();
    a.prefs.insert(
        "list_density".into(),
        Stamped {
            updated_ms: 10,
            value: 1.into(),
        },
    );
    a.prefs.insert(
        "after_triage".into(),
        Stamped {
            updated_ms: 30,
            value: 2.into(),
        },
    );
    a.rules.insert(
        rid(1),
        RuleEntry {
            updated_ms: 5,
            rule: Some(rule("old")),
        },
    );
    let mut b = SyncedSettings::new();
    b.prefs.insert(
        "list_density".into(),
        Stamped {
            updated_ms: 20,
            value: 0.into(),
        },
    );
    b.prefs.insert(
        "after_triage".into(),
        Stamped {
            updated_ms: 25,
            value: 0.into(),
        },
    );
    b.rules.insert(
        rid(1),
        RuleEntry {
            updated_ms: 6,
            rule: Some(rule("new")),
        },
    );
    b.theme = Some(Stamped {
        updated_ms: 1,
        value: SyncedTheme {
            mode: 2,
            light: "dail-light".into(),
            dark: "dail-dark".into(),
        },
    });
    let m = a.merge(&b);
    assert_eq!(m.prefs["list_density"].value, 0, "b is newer");
    assert_eq!(m.prefs["after_triage"].value, 2, "a is newer");
    assert_eq!(m.rules[&rid(1)].rule.as_ref().unwrap().name, "new");
    assert_eq!(m.theme.unwrap().value.mode, 2, "only b has a theme");
}

#[test]
fn a_tombstone_wins_a_tie_and_a_newer_save_revives() {
    let live = doc_with(RuleEntry {
        updated_ms: 7,
        rule: Some(rule("r")),
    });
    let dead = doc_with(RuleEntry {
        updated_ms: 7,
        rule: None,
    });
    assert!(live.merge(&dead).rules[&rid(1)].rule.is_none());
    assert!(dead.merge(&live).rules[&rid(1)].rule.is_none());
    let saved = doc_with(RuleEntry {
        updated_ms: 8,
        rule: Some(rule("r")),
    });
    assert!(dead.merge(&saved).rules[&rid(1)].rule.is_some());
}

#[test]
fn prune_drops_only_old_tombstones() {
    let mut doc = SyncedSettings::new();
    doc.rules.insert(
        rid(1),
        RuleEntry {
            updated_ms: 0,
            rule: None,
        },
    );
    doc.rules.insert(
        rid(2),
        RuleEntry {
            updated_ms: 100,
            rule: None,
        },
    );
    doc.rules.insert(
        rid(3),
        RuleEntry {
            updated_ms: 0,
            rule: Some(rule("kept")),
        },
    );
    doc.prune(TOMBSTONE_TTL_MS + 50);
    assert_eq!(
        doc.rules.keys().copied().collect::<Vec<_>>(),
        vec![rid(2), rid(3)]
    );
}

#[test]
fn validation_rejects_hostile_documents() {
    let too_big = vec![b' '; SETTINGS_MAX_BYTES + 1];
    assert!(matches!(
        SyncedSettings::from_json(&too_big),
        Err(SettingsError::TooLarge { .. })
    ));
    let err = SyncedSettings::from_json(b"{\"rules\": 3}").unwrap_err();
    assert!(matches!(err, SettingsError::Malformed { .. }));
    assert!(err.to_string().starts_with("ERR021: "));

    let mut empty_name = rule(" ");
    let err = doc_with(RuleEntry {
        updated_ms: 1,
        rule: Some(empty_name.clone()),
    })
    .validate()
    .unwrap_err();
    assert!(err.to_string().starts_with("ERR022: "), "{err}");
    empty_name.name = "x".repeat(MAX_TEXT_CHARS + 1);
    assert!(
        doc_with(RuleEntry {
            updated_ms: 1,
            rule: Some(empty_name)
        })
        .validate()
        .is_err()
    );

    let mut no_action = rule("n");
    no_action.then.clear();
    assert!(
        doc_with(RuleEntry {
            updated_ms: 1,
            rule: Some(no_action)
        })
        .validate()
        .is_err()
    );

    let mut deep = Condition::HasAttachment;
    for _ in 0..MAX_CONDITION_DEPTH {
        deep = Condition::Not { of: Box::new(deep) };
    }
    let mut nested = rule("deep");
    nested.when = deep;
    assert!(
        doc_with(RuleEntry {
            updated_ms: 1,
            rule: Some(nested)
        })
        .validate()
        .is_err()
    );

    let mut empty_move = rule("m");
    empty_move.then = vec![RuleAction::Move { to: String::new() }];
    assert!(
        doc_with(RuleEntry {
            updated_ms: 1,
            rule: Some(empty_move)
        })
        .validate()
        .is_err()
    );

    let mut zero = SyncedSettings::new();
    zero.schema = 0;
    assert!(zero.validate().is_err());
}

#[test]
fn settings_document_schema() {
    insta::assert_json_snapshot!(schemars::schema_for!(SyncedSettings));
}

fn arb_rule() -> impl Strategy<Value = Option<Rule>> {
    prop_oneof![
        Just(None),
        "[a-c]{1,2}".prop_map(|name| Some(rule(&name))),
        "[a-c]{1,2}".prop_map(|name| {
            let mut r = rule(&name);
            r.enabled = false;
            Some(r)
        }),
    ]
}

fn arb_doc() -> impl Strategy<Value = SyncedSettings> {
    let prefs = prop::collection::btree_map(
        prop::sample::select(vec!["list_density", "after_triage", "language"]),
        (0..4i64, 0..3u64),
        0..3,
    );
    let rules = prop::collection::btree_map(1..4u128, (0..4i64, arb_rule()), 0..3);
    let theme = prop::option::of((0..4i64, 0..3u64));
    (0..10i64, prefs, rules, theme).prop_map(|(updated_ms, prefs, rules, theme)| SyncedSettings {
        schema: 1,
        updated_ms,
        theme: theme.map(|(at, mode)| Stamped {
            updated_ms: at,
            value: SyncedTheme {
                mode,
                light: "l".into(),
                dark: format!("d{mode}"),
            },
        }),
        prefs: prefs
            .into_iter()
            .map(|(k, (at, v))| {
                (
                    k.to_owned(),
                    Stamped {
                        updated_ms: at,
                        value: v.into(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
        rules: rules
            .into_iter()
            .map(|(n, (at, rule))| {
                (
                    rid(n),
                    RuleEntry {
                        updated_ms: at,
                        rule,
                    },
                )
            })
            .collect(),
    })
}

proptest! {
    #[test]
    fn merge_is_commutative(a in arb_doc(), b in arb_doc()) {
        prop_assert_eq!(a.merge(&b), b.merge(&a));
    }

    #[test]
    fn merge_is_associative(a in arb_doc(), b in arb_doc(), c in arb_doc()) {
        prop_assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
    }

    #[test]
    fn merge_is_idempotent(a in arb_doc(), b in arb_doc()) {
        prop_assert_eq!(a.merge(&a), a.clone());
        let m = a.merge(&b);
        prop_assert_eq!(m.merge(&b), m);
    }

    #[test]
    fn a_written_document_reads_back(a in arb_doc()) {
        prop_assert_eq!(SyncedSettings::from_json(&a.to_json().unwrap()).unwrap(), a);
    }
}
