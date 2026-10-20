//! `dail exec` and `dail batch`.

use std::process::ExitCode;

use anyhow::Context;
use app_core::{Command, CommandName, CoreError, Origin};
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};

use crate::cli::GlobalArgs;
use crate::output::{emit, preview_json, result_json};
use crate::runtime::{Listen, connect};

/// Builds a command from a name and an optional JSON-object argument string.
pub fn parse_exec(name: &str, args: Option<&str>) -> Result<Command, CoreError> {
    if CommandName::from_name(name).is_none() {
        return Err(CoreError::InvalidCommand {
            reason: format!("unknown command `{name}`; run `dail exec --help` for the list"),
        });
    }
    let args: Value = match args {
        Some(text) => serde_json::from_str(text).map_err(|err| CoreError::InvalidCommand {
            reason: format!("--args is not valid JSON: {err}"),
        })?,
        None => Value::Object(serde_json::Map::new()),
    };
    Command::from_value(serde_json::json!({ "cmd": name, "args": args }))
}

/// Parses one batch line (`{"cmd":…,"args":…}`).
#[cfg(test)]
pub fn parse_line(line: &str) -> Result<Command, CoreError> {
    parse_batch_line(line).map(|(cmd, _)| cmd)
}

/// Parses one batch line, with its `"dry_run": true` flag if it has one.
pub fn parse_batch_line(line: &str) -> Result<(Command, bool), CoreError> {
    let mut value: Value = serde_json::from_str(line).map_err(|err| CoreError::InvalidCommand {
        reason: format!("not valid JSON: {err}"),
    })?;
    let dry_run = value
        .as_object_mut()
        .and_then(|o| o.remove("dry_run"))
        .is_some_and(|v| v == Value::Bool(true));
    Command::from_value(value).map(|cmd| (cmd, dry_run))
}

/// Submits `cmd`, or with `dry_run` previews it; the JSON to print and whether it succeeded.
async fn run(
    conn: &app_sync::Connection,
    cmd: Command,
    origin: Origin,
    dry_run: bool,
    line: Option<usize>,
) -> anyhow::Result<(Value, bool)> {
    let name = cmd.name();
    if dry_run {
        let result = conn.preview(cmd, origin).await?;
        return Ok((preview_json(name, line, &result), result.is_ok()));
    }
    let result = conn.submit(cmd, origin).await?;
    Ok((result_json(Some(name), line, &result), result.is_ok()))
}

pub async fn exec(
    global: &GlobalArgs,
    name: &str,
    args: Option<&str>,
    dry_run: bool,
) -> anyhow::Result<ExitCode> {
    let front = connect(global, Listen::No).await?;
    let (value, ok) = match parse_exec(name, args) {
        Ok(cmd) => run(&front.conn, cmd, Origin::Cli, dry_run, None).await?,
        Err(err) => (result_json(Some(name), None, &Err(err)), false),
    };
    emit(global.pretty, &value)?;
    front.close().await?;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub async fn batch(global: &GlobalArgs, input: &str, keep_going: bool) -> anyhow::Result<ExitCode> {
    let reader: Box<dyn AsyncBufRead + Unpin + Send> = if input == "-" {
        Box::new(BufReader::new(tokio::io::stdin()))
    } else {
        let file = tokio::fs::File::open(input)
            .await
            .with_context(|| format!("cannot open {input}"))?;
        Box::new(BufReader::new(file))
    };
    let front = connect(global, Listen::No).await?;
    let session = &front.conn;
    let mut lines = reader.lines();
    let mut number = 0_usize;
    let mut failed = false;
    while let Some(line) = lines.next_line().await? {
        number += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (value, ok) = match parse_batch_line(trimmed) {
            Ok((cmd, dry_run)) => {
                let done = run(session, cmd, Origin::Batch, dry_run, Some(number)).await?;
                // Later lines see the results of earlier ones (e.g. a sync before a listing).
                session.flush().await?;
                done
            }
            Err(err) => (result_json(None, Some(number), &Err(err)), false),
        };
        emit(global.pretty, &value)?;
        if !ok {
            failed = true;
            if !keep_going {
                break;
            }
        }
    }
    front.close().await?;
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parity: every registry command round-trips through `dail exec` argument parsing.
    #[test]
    fn every_registry_command_parses_via_exec() {
        for &name in CommandName::ALL {
            let example = name.example();
            let json = serde_json::to_value(&example).unwrap();
            let args = json["args"].to_string();
            assert_eq!(parse_exec(name.as_str(), Some(&args)).unwrap(), example);
            assert_eq!(parse_line(&json.to_string()).unwrap(), example);
        }
    }

    #[test]
    fn missing_args_default_to_empty_object() {
        assert_eq!(parse_exec("undo", None).unwrap(), Command::Undo {});
        assert_eq!(parse_line(r#"{"cmd":"redo"}"#).unwrap(), Command::Redo {});
    }

    #[test]
    fn unknown_command_and_bad_json_are_invalid_command() {
        assert!(matches!(
            parse_exec("nope", None),
            Err(CoreError::InvalidCommand { .. })
        ));
        assert!(matches!(
            parse_exec("undo", Some("{")),
            Err(CoreError::InvalidCommand { .. })
        ));
        assert!(matches!(
            parse_line("not json"),
            Err(CoreError::InvalidCommand { .. })
        ));
    }
}
