//! The golden settings documents (ADR 0015): every document a released build wrote to a
//! `dail/Settings` folder must still be read intact. A golden document is never edited.

use std::path::{Path, PathBuf};

use app_core::{Condition, RuleAction, RuleId, SyncedSettings};

fn settings_root() -> PathBuf {
    // The run-time manifest dir first, for test runs on a device (AGENTS.md §6).
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    Path::new(&manifest).join("testdata/golden/settings")
}

fn rule_id(n: u128) -> RuleId {
    RuleId::from_uuid(uuid::Uuid::from_u128(0xa000 + n))
}

#[test]
fn every_golden_settings_document_reads_intact() {
    let mut names: Vec<_> = std::fs::read_dir(settings_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no golden settings document");
    for path in names {
        let bytes = std::fs::read(&path).unwrap();
        let doc = SyncedSettings::from_json(&bytes);
        assert!(doc.is_ok(), "{}: {doc:?}", path.display());
        let doc = doc.unwrap();
        let theme = doc.theme.as_ref().unwrap();
        assert_eq!(theme.value.mode, 2);
        assert_eq!(theme.value.dark, "dail-dark");
        assert_eq!(doc.prefs["list_density"].value, 1);
        let newsletters = doc.rules[&rule_id(1)].rule.as_ref().unwrap();
        assert!(newsletters.enabled);
        assert!(matches!(newsletters.when, Condition::Any { .. }));
        assert_eq!(
            newsletters.then[0],
            RuleAction::Move {
                to: "Reading".into()
            }
        );
        let invoices = doc.rules[&rule_id(2)].rule.as_ref().unwrap();
        assert!(!invoices.enabled);
        assert_eq!(invoices.then.len(), 3);
        assert!(doc.rules[&rule_id(3)].rule.is_none(), "a tombstone");
        assert_eq!(doc.live_rules().count(), 2);
        // What this build writes reads back the same.
        let again = SyncedSettings::from_json(&doc.to_json().unwrap()).unwrap();
        assert_eq!(again, doc);
    }
}

#[test]
fn unknown_fields_of_a_newer_schema_are_ignored() {
    let doc = SyncedSettings::from_json(
        br#"{"schema": 7, "updated_ms": 5, "future": {"x": 1},
            "rules": {"00000000-0000-0000-0000-00000000a001":
              {"updated_ms": 5, "rule": {"name": "n", "when": {"test": "has_attachment"},
               "then": [{"action": "star"}], "colour": "red"}}}}"#,
    )
    .unwrap();
    assert_eq!(doc.schema, 7);
    assert!(doc.rules[&rule_id(1)].rule.as_ref().unwrap().enabled);
}
