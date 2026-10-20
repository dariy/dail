//! The agent policy (`CommandSpec::agents`, `ARCHITECTURE.md §9`, ADR 0002): commands an agent
//! could use to move data or credentials off the machine without the user are user-only.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{self, Mail};
use app_core::{
    AgentAccess, AttachmentRef, AttachmentState, Command, CommandName, CoreError, Ctx, DraftId,
    Effect, Endpoint, Origin, Outcome, ProviderPreset, Security,
};

const AGENTS: [Origin; 2] = [Origin::Mcp, Origin::McpAllowSend];

fn run(m: &mut Mail, cmd: Command, origin: Origin) -> Result<Outcome, CoreError> {
    m.seed += 1;
    let ctx = Ctx {
        origin,
        ..support::ctx(m.seed)
    };
    m.engine.dispatch(cmd, &ctx)
}

fn refused(name: &str, origin: Origin) -> CoreError {
    CoreError::OriginNotAllowed {
        command: name.to_owned(),
        origin: origin.as_str().to_owned(),
    }
}

fn attacker() -> Endpoint {
    Endpoint {
        host: "imap.attacker.example".into(),
        port: 993,
        security: Security::Tls,
    }
}

#[test]
fn every_user_only_command_is_refused_from_agents_and_changes_nothing() {
    for &name in CommandName::ALL {
        if name.agent_access() != AgentAccess::UserOnly {
            continue;
        }
        for origin in AGENTS {
            let mut m = Mail::hostinger();
            let before = m.engine.state().clone();
            let undo = m.engine.history().undo_len();
            let err = run(&mut m, name.example(), origin).unwrap_err();
            assert_eq!(err, refused(name.as_str(), origin));
            assert_eq!(m.engine.state(), &before, "{name:?}");
            assert_eq!(m.engine.history().undo_len(), undo, "{name:?}");
        }
        // The user's origins get past the check (the example may fail for its own reasons).
        for origin in [Origin::Gui, Origin::Tui, Origin::Cli, Origin::Batch] {
            let mut m = Mail::hostinger();
            let result = run(&mut m, name.example(), origin);
            assert!(
                !matches!(result, Err(CoreError::OriginNotAllowed { .. })),
                "{name:?} from {origin:?}: {result:?}"
            );
        }
    }
}

#[test]
fn agents_cannot_change_which_servers_get_the_password() {
    let mut m = Mail::hostinger();
    let id = m.account;
    for origin in AGENTS {
        let err = run(
            &mut m,
            Command::UpdateAccount {
                id,
                display_name: None,
                imap: Some(attacker()),
                smtp: None,
            },
            origin,
        )
        .unwrap_err();
        assert_eq!(err, refused("update_account", origin));
        let err = run(
            &mut m,
            Command::AddAccount {
                preset: ProviderPreset::Custom,
                email: "drop@attacker.example".into(),
                display_name: None,
                password: None,
                imap: Some(attacker()),
                smtp: Some(attacker()),
            },
            origin,
        )
        .unwrap_err();
        assert_eq!(err, refused("add_account", origin));
    }
    assert_eq!(m.engine.state().accounts.len(), 1);
    // The password still goes only to the account's own servers.
    let test = run(&mut m, Command::TestAccount { id }, Origin::Mcp).unwrap();
    let hosts: Vec<(&str, &str)> = test
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::VerifyAccount { imap, smtp, .. } => {
                Some((imap.host.as_str(), smtp.host.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(hosts, [("imap.example.com", "smtp.example.com")]);
}

#[test]
fn agents_cannot_restore_a_draft_from_arguments() {
    let mut m = Mail::hostinger();
    let Command::RestoreDraft { mut draft } = CommandName::RestoreDraft.example() else {
        unreachable!()
    };
    draft.account = m.account;
    draft.attachments = vec![AttachmentRef {
        path: "/home/me/.ssh/id_ed25519".into(),
        name: "id_ed25519".into(),
        state: AttachmentState::Ready {
            size: 1,
            content_type: "text/plain".into(),
        },
    }];
    for origin in AGENTS {
        let err = run(
            &mut m,
            Command::RestoreDraft {
                draft: draft.clone(),
            },
            origin,
        )
        .unwrap_err();
        assert_eq!(err, refused("restore_draft", origin));
    }
    assert!(m.engine.state().drafts.is_empty());
}

#[test]
fn agents_still_undo_and_redo_through_user_only_inverses() {
    let mut m = Mail::hostinger();
    let account = m.account;
    // The user discards a draft; the agent's undo restores it (`restore_draft` from history).
    let created = run(
        &mut m,
        Command::CreateDraft {
            account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Lunch?".into(),
            body: String::new(),
        },
        Origin::Gui,
    )
    .unwrap();
    let id: DraftId = serde_json::from_value(created.receipt.data["id"].clone()).unwrap();
    run(&mut m, Command::DiscardDraft { id }, Origin::Gui).unwrap();
    run(&mut m, Command::Undo {}, Origin::Mcp).unwrap();
    assert!(m.engine.state().drafts.contains_key(&id));

    // The user moves the account to a new server; the agent's undo and redo only switch
    // between the user's own two settings.
    let new = Endpoint {
        host: "imap.new.example".into(),
        port: 993,
        security: Security::Tls,
    };
    run(
        &mut m,
        Command::UpdateAccount {
            id: account,
            display_name: None,
            imap: Some(new.clone()),
            smtp: None,
        },
        Origin::Cli,
    )
    .unwrap();
    let host = |m: &Mail| m.engine.state().accounts[&account].imap.host.clone();
    run(&mut m, Command::Undo {}, Origin::Mcp).unwrap();
    assert_eq!(host(&m), "imap.example.com");
    run(&mut m, Command::Redo {}, Origin::McpAllowSend).unwrap();
    assert_eq!(host(&m), new.host);
}
