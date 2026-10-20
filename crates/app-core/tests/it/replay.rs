//! Replay harness: every `tests/replay/*.jsonl` log is replayed and its receipts and final state
//! are snapshot-tested. Replaying twice must give identical results.

use std::path::Path;

use app_core::{CoreError, Engine, replay};
use serde_json::json;

#[test]
fn replay_fixtures_match_snapshots() {
    let replay_file = |path: &Path| {
        let text = std::fs::read_to_string(path).unwrap();
        let records = replay::parse_log(&text).unwrap();
        let mut engine = Engine::default();
        let steps = replay::run(&mut engine, records);
        json!({ "steps": steps, "final_state": engine.state() })
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/replay");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures in {}", dir.display());
    for path in paths {
        let first = replay_file(&path);
        assert_eq!(
            first,
            replay_file(&path),
            "{} is not deterministic",
            path.display()
        );
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        insta::assert_json_snapshot!(name, first);
    }
}

#[test]
fn records_round_trip_through_json() {
    let line = r#"{"t":5,"seed":9,"origin":"mcp","input":{"command":{"cmd":"undo","args":{}}}}"#;
    let records = replay::parse_log(line).unwrap();
    let (number, record) = &records[0];
    assert_eq!(*number, 1);
    assert_eq!(record.ctx(*number).id_seed, 9);
    assert_eq!(serde_json::to_string(record).unwrap(), line);
}

#[test]
fn default_seed_depends_on_line() {
    let text = "# comment\n\n{\"t\":1,\"origin\":\"cli\",\"input\":{\"command\":{\"cmd\":\"undo\",\"args\":{}}}}";
    let records = replay::parse_log(text).unwrap();
    let (line, record) = &records[0];
    assert_eq!(*line, 3);
    assert_eq!(record.ctx(*line).id_seed, 3_u128 << 64);
}

#[test]
fn invalid_line_reports_its_number() {
    let err = replay::parse_log("# ok\n{not json}").unwrap_err();
    assert!(matches!(err, CoreError::InvalidReplay { line: 2, .. }));
}

#[test]
fn audit_records_hide_secrets_and_message_text() {
    use app_core::{Command, Ctx, Origin, ProviderPreset, ReplayInput, ReplayRecord, Secret};

    let ctx = Ctx {
        now_ms: 1_700_000_000_000,
        id_seed: 42,
        origin: Origin::Mcp,
    };
    let add = ReplayRecord::new(
        &ctx,
        ReplayInput::Command(Command::AddAccount {
            preset: ProviderPreset::Gmail,
            email: "me@gmail.com".into(),
            display_name: None,
            password: Some(Secret::new("hunter2")),
            imap: None,
            smtp: None,
        }),
    );
    let text = add.redacted_line();
    assert!(!text.contains("hunter2"));
    let line: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(line["t"], 1_700_000_000_000_i64);
    assert_eq!(line["seed"], 42);
    assert_eq!(line["origin"], "mcp");
    assert_eq!(line["input"]["command"]["args"]["password"], "<redacted>");

    let draft = ReplayRecord::new(
        &ctx,
        ReplayInput::Command(Command::Reply {
            message: app_core::MessageId::from_uuid(uuid::Uuid::from_u128(1)),
            body: Some("my private reply".into()),
        }),
    );
    let text = draft.redacted_line();
    assert!(!text.contains("private"));
    // A redacted line is still a valid record, seeds beyond 64 bits included.
    let parsed: ReplayRecord = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed.origin, Origin::Mcp);
    let big = ReplayRecord::new(
        &Ctx {
            id_seed: 7 << 64,
            ..ctx
        },
        parsed.input.clone(),
    );
    let reparsed: ReplayRecord = serde_json::from_str(&big.redacted_line()).unwrap();
    assert_eq!(reparsed.seed, Some(7 << 64));
    assert!(
        matches!(
            reparsed.input,
            ReplayInput::Command(Command::Reply { body: Some(ref body), .. }) if body == "<redacted>"
        ),
        "{:?}",
        reparsed.input
    );
}
