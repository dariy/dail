//! `dail schema`.

use std::process::ExitCode;

use anyhow::Context;
use app_core::CommandName;
use app_core::registry::{command_schema, event_schema};
use serde_json::json;

use crate::cli::GlobalArgs;
use crate::output::emit;

pub fn schema(global: &GlobalArgs, command: Option<&str>) -> anyhow::Result<ExitCode> {
    let value = match command {
        None => json!({ "command": command_schema(), "event": event_schema() }),
        Some(name) => {
            let name = CommandName::from_name(name)
                .with_context(|| format!("unknown command `{name}`"))?;
            let spec = name.spec();
            json!({
                "name": spec.name,
                "title": spec.title,
                "category": spec.category,
                "kind": spec.kind,
                "agents": spec.agents,
                "default_keys": spec.default_keys,
                "description": spec.description(),
                "args_schema": name.args_schema(),
            })
        }
    };
    emit(global.pretty, &value)?;
    Ok(ExitCode::SUCCESS)
}
