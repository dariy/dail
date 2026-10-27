//! `dail update check` and `dail update install`: aliases for the update commands (ADR 0011).
//! Each step's receipt is printed as a JSON line, then `{"name":"update","update":…}` with the
//! state after the runtime answered.

use std::process::ExitCode;

use app_core::{Command, Origin, UpdateStatus};
use serde_json::json;

use crate::cli::{GlobalArgs, UpdateCmd};
use crate::output::{emit, result_json};
use crate::runtime::{Front, Listen, connect};

/// Submits `cmd`, prints its result, waits for its effects and returns the update status, or
/// `None` if the command was refused.
async fn step(
    global: &GlobalArgs,
    front: &Front,
    cmd: Command,
) -> anyhow::Result<Option<UpdateStatus>> {
    let name = cmd.name();
    let result = front.conn.submit(cmd, Origin::Cli).await?;
    emit(global.pretty, &result_json(Some(name), None, &result))?;
    if result.is_err() {
        return Ok(None);
    }
    front.conn.flush().await?;
    Ok(Some(front.conn.state().await?.update.status.clone()))
}

pub async fn run(global: &GlobalArgs, command: &UpdateCmd) -> anyhow::Result<ExitCode> {
    let front = connect(global, Listen::No).await?;
    let ok = steps(global, &front, command).await;
    let update = front.conn.state().await.map(|s| s.update.clone());
    front.close().await?;
    let ok = ok?;
    emit(
        global.pretty,
        &json!({ "ok": ok, "name": "update", "update": update? }),
    )?;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

async fn steps(global: &GlobalArgs, front: &Front, command: &UpdateCmd) -> anyhow::Result<bool> {
    let checked = step(global, front, Command::CheckForUpdates {}).await?;
    let install = match command {
        UpdateCmd::Check => {
            return Ok(matches!(
                checked,
                Some(UpdateStatus::Idle | UpdateStatus::Available { .. })
            ));
        }
        UpdateCmd::Install { yes } => *yes,
    };
    match checked {
        Some(UpdateStatus::Available { .. }) => {}
        // Nothing newer: done.
        Some(UpdateStatus::Idle) => return Ok(true),
        _ => return Ok(false),
    }
    let downloaded = step(global, front, Command::DownloadUpdate {}).await?;
    if !matches!(downloaded, Some(UpdateStatus::Ready { .. })) {
        return Ok(false);
    }
    if !install {
        // The download is kept; `--yes` installs it.
        return Ok(true);
    }
    let installed = step(global, front, Command::InstallUpdate { confirm: true }).await?;
    Ok(matches!(installed, Some(UpdateStatus::Installed { .. })))
}
