//! End-to-end tests of the `dail` binary on the memory backend (no network).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

fn dail(args: &[&str], stdin: Option<&str>) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Keep `--backend live` away from the real user profile.
    let data = std::env::temp_dir().join(format!("dail-cli-{}-data", std::process::id()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_dail"))
        .args(args)
        .current_dir(root)
        .env("XDG_DATA_HOME", &data)
        .env("APPDATA", &data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    if let Some(input) = stdin {
        pipe.write_all(input.as_bytes()).unwrap();
    }
    drop(pipe);
    child.wait_with_output().unwrap()
}

fn json_lines(output: &Output) -> Vec<Value> {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn smoke_batch_prints_one_receipt_per_command() {
    let output = dail(
        &["batch", "--backend", "memory", "tests/smoke/basic.jsonl"],
        None,
    );
    assert!(output.status.success(), "{output:?}");
    let lines = json_lines(&output);
    let names: Vec<&str> = lines.iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "get_settings",
            "add_account",
            "set_setting",
            "undo",
            "redo",
            "open_palette",
            "close_palette",
            "get_settings",
            "sync_now",
            "list_mailboxes",
            "check_for_updates"
        ]
    );
    assert!(lines.iter().all(|line| line["ok"] == true));
    assert_eq!(lines[7]["data"]["poll_secs"], 300);
    assert_eq!(lines[1]["data"]["email"], "me@gmail.com");
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(!stdout.contains("app-password"), "secret echoed");
}

#[test]
fn exec_runs_one_command() {
    let output = dail(
        &[
            "exec",
            "set_setting",
            "--args",
            r#"{"key":"history_cap","value":50}"#,
            "--backend",
            "memory",
        ],
        None,
    );
    assert!(output.status.success(), "{output:?}");
    let lines = json_lines(&output);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["undo_label"], "Set history_cap");
}

#[test]
fn exec_rejection_exits_1_with_structured_error() {
    let output = dail(&["exec", "undo", "--backend", "memory"], None);
    assert_eq!(output.status.code(), Some(1));
    let lines = json_lines(&output);
    assert_eq!(lines[0]["ok"], false);
    assert_eq!(lines[0]["error"]["error"], "nothing_to_undo");
}

#[test]
fn batch_stops_at_first_failure_unless_keep_going() {
    let script = "{\"cmd\":\"undo\"}\n{\"cmd\":\"open_palette\"}\n";
    let output = dail(&["batch", "-", "--backend", "memory"], Some(script));
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json_lines(&output).len(), 1);

    let output = dail(
        &["batch", "-", "--backend", "memory", "--keep-going"],
        Some(script),
    );
    assert_eq!(output.status.code(), Some(1));
    let lines = json_lines(&output);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["line"], 1);
    assert_eq!(lines[1]["ok"], true);
}

#[test]
fn batch_reports_invalid_lines() {
    let output = dail(&["batch", "-", "--backend", "memory"], Some("{oops\n"));
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json_lines(&output)[0]["error"]["error"], "invalid_command");
}

#[test]
fn live_backend_uses_the_named_profile_under_the_data_dir() {
    let output = dail(&["exec", "get_settings", "--profile", "ci_test"], None);
    assert!(output.status.success(), "{output:?}");
    let output = dail(&["exec", "get_settings", "--profile", "../escape"], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid profile name"));
    let output = dail(&["exec", "get_settings", "--fixture", "basic"], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--backend memory"));
}

#[test]
fn memory_backend_accepts_builtin_fixture_and_rejects_unknown() {
    let ok = dail(
        &[
            "exec",
            "get_settings",
            "--backend",
            "memory",
            "--fixture",
            "basic",
        ],
        None,
    );
    assert!(ok.status.success(), "{ok:?}");
    let bad = dail(
        &[
            "exec",
            "get_settings",
            "--backend",
            "memory",
            "--fixture",
            "nope",
        ],
        None,
    );
    assert_eq!(bad.status.code(), Some(2));
}

#[test]
fn schema_lists_commands_and_describes_one() {
    let output = dail(&["schema"], None);
    assert!(output.status.success());
    let schema = &json_lines(&output)[0];
    assert!(schema["command"]["oneOf"].is_array());
    assert!(schema["event"].is_object());

    let output = dail(&["schema", "--command", "set_setting"], None);
    let spec = &json_lines(&output)[0];
    assert_eq!(spec["kind"], "mutation");
    assert_eq!(spec["args_schema"]["type"], "object");
    assert!(!spec["description"].as_str().unwrap().is_empty());
}

#[test]
fn exec_help_lists_registry_commands() {
    let output = dail(&["exec", "--help"], None);
    let help = String::from_utf8(output.stdout).unwrap();
    for name in [
        "get_settings",
        "set_setting",
        "undo",
        "redo",
        "open_palette",
    ] {
        assert!(help.contains(name), "{name} missing from help");
    }
}

#[test]
fn mcp_stdio_lists_and_calls_tools() {
    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "test", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "set_setting", "arguments": {"key": "poll_secs", "value": 600}}}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
            "name": "redo", "arguments": {}}}),
    ];
    let input: String = requests.iter().map(|r| format!("{r}\n")).collect();
    let output = dail(&["mcp", "--backend", "memory"], Some(&input));
    assert!(output.status.success(), "{output:?}");
    let responses = json_lines(&output);
    let by_id = |id: i64| responses.iter().find(|r| r["id"] == id).unwrap();
    assert_eq!(by_id(1)["result"]["serverInfo"]["name"], "dail");
    let tools = by_id(2)["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"set_setting") && names.contains(&"undo"));
    let get_settings = tools.iter().find(|t| t["name"] == "get_settings").unwrap();
    assert_eq!(get_settings["annotations"]["readOnlyHint"], true);
    let set = &by_id(3)["result"];
    assert_eq!(set["isError"], false);
    assert_eq!(set["structuredContent"]["undo_label"], "Set poll_secs");
    let redo = &by_id(4)["result"];
    assert_eq!(redo["isError"], true);
    assert_eq!(
        redo["structuredContent"]["error"]["error"],
        "nothing_to_redo"
    );
}

#[test]
fn account_add_reads_password_from_stdin_and_verifies() {
    let output = dail(
        &[
            "--backend",
            "memory",
            "--fixture",
            "basic",
            "account",
            "add",
            "--preset",
            "hostinger",
            "--email",
            "alice@example.com",
            "--password-stdin",
        ],
        Some("s3cret-pass\n"),
    );
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(!stdout.contains("s3cret-pass"), "secret echoed: {stdout}");
    let lines = json_lines(&output);
    let names: Vec<&str> = lines.iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["add_account", "test_account", "account_status"]);
    assert_eq!(lines[2]["status"]["state"], "verified");
    assert_eq!(lines[0]["data"]["imap"]["host"], "imap.hostinger.com");
}

#[test]
fn account_add_reports_failed_verification_with_exit_code_1() {
    let output = dail(
        &[
            "--backend",
            "memory",
            "account",
            "add",
            "--preset",
            "custom",
            "--email",
            "bob@example.org",
            "--imap",
            "mail.example.org:143",
            "--smtp",
            "mail.example.org:587",
            "--password-stdin",
        ],
        Some("pw\n"),
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let lines = json_lines(&output);
    assert_eq!(lines[0]["data"]["imap"]["security"], "starttls");
    assert_eq!(lines[2]["ok"], false);
    assert_eq!(lines[2]["status"]["state"], "failed");
}

#[test]
fn account_add_never_takes_a_password_argument() {
    let output = dail(
        &[
            "--backend",
            "memory",
            "account",
            "add",
            "--preset",
            "gmail",
            "--email",
            "me@gmail.com",
            "--password",
            "hunter2",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    // Without a terminal and without --password-stdin, the alias refuses instead of hanging.
    let output = dail(
        &[
            "--backend",
            "memory",
            "account",
            "add",
            "--preset",
            "gmail",
            "--email",
            "me@gmail.com",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("--password-stdin"));
}

#[test]
fn account_add_without_password_skips_the_test() {
    let output = dail(
        &[
            "--backend",
            "memory",
            "account",
            "add",
            "--preset",
            "gmail",
            "--email",
            "me@gmail.com",
            "--no-password",
        ],
        None,
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(json_lines(&output).len(), 1);
}

#[test]
fn live_backend_persists_accounts_in_the_profile_config() {
    let dir = std::env::temp_dir().join(format!("dail-cli-{}-live", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profile = dir.to_str().unwrap();
    let output = dail(
        &[
            "--profile-dir",
            profile,
            "exec",
            "add_account",
            "--args",
            r#"{"preset":"gmail","email":"me@gmail.com"}"#,
        ],
        None,
    );
    assert!(output.status.success(), "{output:?}");
    let config = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert!(config.contains("email = \"me@gmail.com\""), "{config}");

    // A second process sees the persisted account.
    let output = dail(
        &[
            "--profile-dir",
            profile,
            "exec",
            "add_account",
            "--args",
            r#"{"preset":"gmail","email":"me@gmail.com"}"#,
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json_lines(&output)[0]["error"]["error"], "already_exists");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn batch_syncs_a_fixture_and_lists_threads_line_by_line() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new(env!("CARGO_BIN_EXE_dail"))
        .args(["batch", "--backend", "memory", "--fixture", "basic", "-"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut send = |line: &str| -> Value {
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
        let mut reply = String::new();
        stdout.read_line(&mut reply).unwrap();
        serde_json::from_str(&reply).unwrap()
    };
    let added = send(
        r#"{"cmd":"add_account","args":{"preset":"hostinger","email":"alice@example.com","password":"pw"}}"#,
    );
    assert_eq!(added["ok"], true, "{added}");
    let synced = send(r#"{"cmd":"sync_now"}"#);
    assert_eq!(synced["data"]["started"].as_array().unwrap().len(), 1);
    // Batch waits for the sync before reading the next line.
    let mailboxes = send(r#"{"cmd":"list_mailboxes"}"#);
    let inbox = &mailboxes["data"]["mailboxes"][0];
    assert_eq!(inbox["role"], "inbox");
    assert_eq!(inbox["total"], 2);
    let threads =
        send(&json!({"cmd": "list_threads", "args": {"mailbox": inbox["id"]}}).to_string());
    let subjects: Vec<&str> = threads["data"]["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["untrusted_content"]["subject"].as_str().unwrap())
        .collect();
    assert_eq!(subjects, ["Lunch?", "Q3 numbers"]);
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn exec_attaches_to_a_running_mcp_server_of_the_same_profile() {
    let dir = std::env::temp_dir().join(format!("dail-cli-{}-attach", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profile = dir.to_str().unwrap();
    let mut host = Command::new(env!("CARGO_BIN_EXE_dail"))
        .args(["mcp", "--backend", "memory", "--profile-dir", profile])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = host.stdin.take().unwrap();
    let mut output = BufReader::new(host.stdout.take().unwrap());
    let mut request = |value: Value, notify: Option<Value>| {
        if let Some(notification) = notify {
            writeln!(input, "{notification}").unwrap();
        }
        writeln!(input, "{value}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };
    // The host listens before it serves MCP, so it accepts clients once it has answered.
    let init = request(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "test", "version": "0"}}}),
        None,
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "dail");

    let common = ["--backend", "memory", "--profile-dir", profile];
    let set = dail(
        &[
            &common[..],
            &[
                "exec",
                "set_setting",
                "--args",
                r#"{"key":"poll_secs","value":300}"#,
            ],
        ]
        .concat(),
        None,
    );
    assert!(set.status.success(), "{set:?}");
    let get = dail(&[&common[..], &["exec", "get_settings"]].concat(), None);
    assert_eq!(json_lines(&get)[0]["data"]["poll_secs"], 300, "{get:?}");

    // The host sees the change the attached `exec` made.
    let settings = request(
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "get_settings", "arguments": {}}}),
        Some(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})),
    );
    assert_eq!(
        settings["result"]["structuredContent"]["data"]["poll_secs"],
        300
    );
    drop(input);
    assert!(host.wait().unwrap().success());

    // With the host gone, `exec` opens a fresh memory session of its own.
    let get = dail(&[&common[..], &["exec", "get_settings"]].concat(), None);
    assert_eq!(json_lines(&get)[0]["data"]["poll_secs"], 120, "{get:?}");
}

#[test]
fn sessions_on_a_profile_keep_an_audit_log_that_replays() {
    let dir = std::env::temp_dir().join(format!("dail-cli-{}-audit", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profile = dir.to_str().unwrap();
    let common = ["--backend", "memory", "--profile-dir", profile];
    let input = concat!(
        r#"{"cmd":"add_account","args":{"preset":"gmail","email":"me@gmail.com","password":"hunter2"}}"#,
        "\n",
        r#"{"cmd":"set_setting","args":{"key":"poll_secs","value":300}}"#,
        "\n",
    );
    let batch = dail(&[&common[..], &["batch", "-"]].concat(), Some(input));
    assert!(batch.status.success(), "{batch:?}");

    let logs: Vec<_> = std::fs::read_dir(dir.join("logs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(logs.len(), 1, "{logs:?}");
    let log = &logs[0];
    let name = log.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("replay-") && name.ends_with(".jsonl"),
        "{name}"
    );
    let text = std::fs::read_to_string(log).unwrap();
    assert!(!text.contains("hunter2"), "the password is redacted");
    assert!(text.contains(r#""origin":"batch""#));

    let replayed = dail(&["replay", log.to_str().unwrap(), "--state"], None);
    assert!(replayed.status.success(), "{replayed:?}");
    let lines = json_lines(&replayed);
    assert!(
        lines
            .iter()
            .any(|l| l["name"] == "add_account" && l["ok"] == true)
    );
    let state = &lines.last().unwrap()["state"];
    assert_eq!(state["settings"]["poll_secs"], 300);
    assert_eq!(state["accounts"].as_object().unwrap().len(), 1);
}

#[test]
fn replay_runs_hand_written_fixtures_and_rejects_bad_logs() {
    let output = dail(
        &["replay", "crates/app-core/tests/replay/approvals.jsonl"],
        None,
    );
    assert!(output.status.success(), "{output:?}");
    let lines = json_lines(&output);
    assert!(lines.iter().all(|l| l["line"].is_u64()));
    let refused = lines
        .iter()
        .find(|l| l["name"] == "approve_request")
        .unwrap();
    assert_eq!(refused["error"]["error"], "origin_not_allowed");

    let bad = std::env::temp_dir().join(format!("dail-cli-{}-bad.jsonl", std::process::id()));
    std::fs::write(&bad, "# fine\nnot json\n").unwrap();
    let output = dail(&["replay", bad.to_str().unwrap()], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("line 2"));
}

#[test]
fn tui_needs_a_terminal() {
    let output = dail(&["tui", "--backend", "memory"], None);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("needs a terminal"), "{stderr}");
}
