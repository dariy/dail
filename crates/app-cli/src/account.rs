//! `dail account add`: an ergonomic alias for `add_account` followed by `test_account`;
//! `dail account password`: `set_password`, then the login's result.

use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;

use anyhow::{Context, bail};
use app_core::{AccountId, AccountStatus, Command, Endpoint, Origin, Secret};
use serde_json::json;

use crate::cli::{AccountAddArgs, AccountPasswordArgs, GlobalArgs};
use crate::output::{emit, result_json};
use crate::runtime::{Listen, connect};

/// Parses `host:port[:tls|starttls]` ([`Endpoint::parse`]).
pub fn parse_endpoint(text: &str) -> anyhow::Result<Endpoint> {
    Endpoint::parse(text).map_err(anyhow::Error::msg)
}

/// Strips the line ending of a password line; rejects empty passwords.
pub fn password_from_line(line: &str) -> anyhow::Result<Secret> {
    let password = line.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        bail!("empty password");
    }
    Ok(Secret::new(password))
}

fn read_password(args: &AccountAddArgs) -> anyhow::Result<Option<Secret>> {
    if args.no_password {
        return Ok(None);
    }
    prompt_password(&args.email, args.password_stdin).map(Some)
}

/// The password from stdin's first line, or from a no-echo prompt on a terminal.
fn prompt_password(email: &str, from_stdin: bool) -> anyhow::Result<Secret> {
    if from_stdin {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .context("cannot read the password from stdin")?;
        return password_from_line(&line);
    }
    if !std::io::stdin().is_terminal() {
        bail!("stdin is not a terminal: use --password-stdin");
    }
    let prompt = format!("Password for {email}: ");
    let line = rpassword::prompt_password(prompt).context("cannot read the password")?;
    password_from_line(&line)
}

pub async fn password(global: &GlobalArgs, args: &AccountPasswordArgs) -> anyhow::Result<ExitCode> {
    let front = connect(global, Listen::No).await?;
    let session = &front.conn;
    let id = session
        .state()
        .await?
        .accounts
        .values()
        .find(|a| a.email.eq_ignore_ascii_case(args.email.trim()))
        .map(|a| a.id);
    let Some(id) = id else {
        front.close().await?;
        bail!("no account {}", args.email);
    };
    let password = prompt_password(&args.email, args.password_stdin)?;
    let set = session
        .submit(Command::SetPassword { id, password }, Origin::Cli)
        .await?;
    emit(
        global.pretty,
        &result_json(Some("set_password"), None, &set),
    )?;
    let mut ok = set.is_ok();
    if ok {
        session.flush().await?;
        let status = session
            .state()
            .await?
            .accounts
            .get(&id)
            .map(|account| account.status.clone())
            .unwrap_or_default();
        ok = matches!(status, AccountStatus::Verified { .. });
        emit(
            global.pretty,
            &json!({ "ok": ok, "name": "account_status", "id": id, "status": status }),
        )?;
    }
    front.close().await?;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub async fn add(global: &GlobalArgs, args: &AccountAddArgs) -> anyhow::Result<ExitCode> {
    let imap = args.imap.as_deref().map(parse_endpoint).transpose()?;
    let smtp = args.smtp.as_deref().map(parse_endpoint).transpose()?;
    let password = read_password(args)?;
    let test = !args.no_test && password.is_some();
    let cmd = Command::AddAccount {
        preset: args.preset.into(),
        email: args.email.clone(),
        display_name: args.display_name.clone(),
        password,
        imap,
        smtp,
    };

    let front = connect(global, Listen::No).await?;
    let session = &front.conn;
    let added = session.submit(cmd, Origin::Cli).await?;
    emit(
        global.pretty,
        &result_json(Some("add_account"), None, &added),
    )?;
    let Ok(receipt) = added else {
        front.close().await?;
        return Ok(ExitCode::FAILURE);
    };
    let id: AccountId = serde_json::from_value(receipt.data["id"].clone())
        .context("add_account receipt has no account id")?;

    let mut ok = true;
    if test {
        let tested = session
            .submit(Command::TestAccount { id }, Origin::Cli)
            .await?;
        emit(
            global.pretty,
            &result_json(Some("test_account"), None, &tested),
        )?;
        session.flush().await?;
        let status = session
            .state()
            .await?
            .accounts
            .get(&id)
            .map(|account| account.status.clone())
            .unwrap_or_default();
        ok = tested.is_ok() && matches!(status, AccountStatus::Verified { .. });
        emit(
            global.pretty,
            &json!({ "ok": ok, "name": "account_status", "id": id, "status": status }),
        )?;
    }
    front.close().await?;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::Security;

    #[test]
    fn endpoints_parse_with_and_without_security() {
        let tls = parse_endpoint("mail.example.org:993").unwrap();
        assert_eq!(
            (tls.host.as_str(), tls.port, tls.security),
            ("mail.example.org", 993, Security::Tls)
        );
        assert_eq!(
            parse_endpoint("mail.example.org:587").unwrap().security,
            Security::StartTls
        );
        assert_eq!(
            parse_endpoint("mail.example.org:2525:starttls")
                .unwrap()
                .security,
            Security::StartTls
        );
        assert_eq!(
            parse_endpoint("mail.example.org:143:tls").unwrap().security,
            Security::Tls
        );
        assert!(parse_endpoint("mail.example.org").is_err());
        assert!(parse_endpoint("mail.example.org:http").is_err());
        assert!(parse_endpoint("mail.example.org:99999").is_err());
    }

    #[test]
    fn password_lines_lose_only_the_line_ending() {
        assert_eq!(password_from_line("p w \r\n").unwrap().expose(), "p w ");
        assert_eq!(password_from_line("pw").unwrap().expose(), "pw");
        assert!(password_from_line("\n").is_err());
    }
}
