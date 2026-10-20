//! End-to-end: an agent attached to `dail mcp --backend memory --fixture basic` over stdio
//! triages the inbox and drafts a reply (`ROADMAP.md` M7). Runs in CI with `cargo test`.
//!
//! The test talks MCP like an agent would: it discovers the tools and resources, reads the
//! inbox, marks one thread read, archives another, replies, and asks to send. The send must stay
//! an approval request: nothing leaves without the user. Setting up the account is the user's
//! job too: `dail account add` attaches to the running server over the profile's socket.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Agent {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl Agent {
    /// `dail mcp` on a memory session that listens on `profile`'s socket.
    fn start(profile: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dail"))
            .args(["mcp", "--backend", "memory", "--fixture", "basic"])
            .arg("--profile-dir")
            .arg(profile)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut agent = Self {
            child,
            input,
            output,
            next_id: 1,
        };
        let init = agent.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "e2e-agent", "version": "0" } }),
        );
        assert_eq!(init["serverInfo"]["name"], "dail");
        assert!(init["capabilities"]["resources"].is_object());
        assert!(
            init["instructions"]
                .as_str()
                .unwrap()
                .contains("untrusted_content")
        );
        agent.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        agent
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
    }

    /// Sends a request and returns its `result` (skipping notifications).
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "server closed"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                assert!(message.get("error").is_none(), "{method}: {message}");
                return message["result"].clone();
            }
        }
    }

    /// Calls a tool and returns its structured content; panics on a tool error.
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let result = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        assert_eq!(result["isError"], false, "{name}: {result}");
        result["structuredContent"].clone()
    }

    fn resource(&mut self, uri: &str) -> Value {
        let result = self.request("resources/read", json!({ "uri": uri }));
        let text = result["contents"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    fn finish(mut self) {
        drop(self.input);
        assert!(self.child.wait().unwrap().success());
    }
}

/// The user adds the fake server's account with `dail account add`, attached to the running
/// `dail mcp` (the server listens before it answers `initialize`).
fn user_adds_the_account(profile: &Path) {
    let mut user = Command::new(env!("CARGO_BIN_EXE_dail"))
        .args(["--backend", "memory", "--profile-dir"])
        .arg(profile)
        .args([
            "account",
            "add",
            "--preset",
            "hostinger",
            "--email",
            "alice@example.com",
            "--password-stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    writeln!(user.stdin.take().unwrap(), "pw").unwrap();
    let output = user.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let last: Value = serde_json::from_str(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(last["status"]["state"], "verified", "{last}");
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-e2e-agent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn an_agent_triages_the_inbox_and_drafts_a_reply_that_waits_for_approval() {
    let profile = scratch();
    let mut agent = Agent::start(&profile);

    // Discover the surface.
    let tools = agent.request("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for needed in [
        "sync_now",
        "mark_read",
        "archive_threads",
        "reply",
        "send_draft",
    ] {
        assert!(names.contains(&needed), "{needed}");
    }
    let templates = agent.request("resources/templates/list", json!({}));
    assert_eq!(templates["resourceTemplates"].as_array().unwrap().len(), 2);

    // Account setup is the user's: an agent can't add one.
    let refused = agent.request(
        "tools/call",
        json!({ "name": "add_account",
                "arguments": { "preset": "hostinger", "email": "alice@example.com" } }),
    );
    assert_eq!(
        refused["structuredContent"]["error"]["error"],
        "origin_not_allowed"
    );
    user_adds_the_account(&profile);
    agent.tool("sync_now", json!({}));

    // Read the inbox through resources.
    let mailboxes = agent.resource("dail://mailboxes");
    let boxes = mailboxes["mailboxes"].as_array().unwrap();
    let inbox = boxes.iter().find(|m| m["role"] == "inbox").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let archive = boxes.iter().find(|m| m["role"] == "archive").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let threads = agent.resource(&format!("dail://mailbox/{inbox}/threads"));
    let threads = threads["threads"].as_array().unwrap().clone();
    assert_eq!(threads.len(), 2, "{threads:?}");
    let by_subject = |subject: &str| {
        threads
            .iter()
            .find(|t| t["untrusted_content"]["subject"] == subject)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let lunch = by_subject("Lunch?");
    let q3 = by_subject("Q3 numbers");

    // Triage: read the unread one, archive the other.
    let marked = agent.tool("mark_read", json!({ "ids": [lunch] }));
    assert!(marked["undo_label"].is_string(), "{marked}");
    agent.tool("archive_threads", json!({ "ids": [q3] }));
    let archived = agent.resource(&format!("dail://mailbox/{archive}/threads"));
    assert!(
        archived["threads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["untrusted_content"]["subject"] == "Q3 numbers")
    );
    let inbox_now = agent.tool("list_threads", json!({ "mailbox": inbox }));
    assert_eq!(inbox_now["data"]["total"], 1);
    assert_eq!(inbox_now["data"]["threads"][0]["unread"], 0);

    // Read the thread with its body, then draft a reply.
    let thread = agent.resource(&format!("dail://thread/{lunch}"));
    let message = thread["messages"][0]["id"].as_str().unwrap().to_owned();
    let reply = agent.tool(
        "reply",
        json!({ "message": message, "body": "Noon works for me." }),
    );
    let draft = reply["data"]["id"].as_str().unwrap().to_owned();
    assert_eq!(reply["data"]["untrusted_content"]["subject"], "Re: Lunch?");
    agent.tool(
        "update_draft",
        json!({ "id": draft, "body": "Noon works for me. See you there!" }),
    );

    // Asking to send creates an approval request; approving it is not up to the agent.
    let asked = agent.tool("send_draft", json!({ "id": draft }));
    assert_eq!(asked["data"]["state"], "awaiting_approval");
    let approval = asked["data"]["approval"]["id"].as_str().unwrap().to_owned();
    let refused = agent.request(
        "tools/call",
        json!({ "name": "approve_request", "arguments": { "id": approval } }),
    );
    assert_eq!(refused["isError"], true);
    assert_eq!(
        refused["structuredContent"]["error"]["error"],
        "origin_not_allowed"
    );
    let pending = agent.tool("list_approvals", json!({}));
    assert_eq!(pending["data"]["approvals"][0]["draft"], draft);
    assert!(
        pending["data"]["approvals"][0]["message"]["send_at"].is_null(),
        "not scheduled"
    );
    let thread = agent.tool("get_thread", json!({ "id": lunch }));
    assert_eq!(
        thread["data"]["messages"].as_array().unwrap().len(),
        1,
        "nothing was sent"
    );

    // What the user sees.
    let view = agent.resource("dail://state/view");
    assert!(view["focus"].is_string());
    agent.finish();
    let _ = std::fs::remove_dir_all(profile);
}
