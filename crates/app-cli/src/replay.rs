//! `dail replay <log>`: runs a replay log through a fresh `Engine` (pure, offline).

use std::path::Path;
use std::process::ExitCode;

use anyhow::Context;
use app_core::{Engine, StepResult, replay};
use serde_json::json;

use crate::cli::GlobalArgs;
use crate::output::{emit, result_json};

/// Prints one result per record (`{"line": n, …receipt}` or `{"line": n, "ok": false, …}`) and,
/// with `state`, the final state. Rejected records are part of the history being replayed, so
/// they don't change the exit code; an unreadable log exits 2.
pub fn replay(global: &GlobalArgs, log: &Path, state: bool) -> anyhow::Result<ExitCode> {
    let text =
        std::fs::read_to_string(log).with_context(|| format!("cannot read {}", log.display()))?;
    let records = replay::parse_log(&text)?;
    let names: Vec<Option<&'static str>> = records
        .iter()
        .map(|(_, record)| match &record.input {
            app_core::ReplayInput::Command(cmd) => Some(cmd.name()),
            app_core::ReplayInput::Event(event) => Some(event.name()),
        })
        .collect();
    let mut engine = Engine::default();
    for (step, name) in replay::run(&mut engine, records).into_iter().zip(names) {
        let result = match step.result {
            StepResult::Ok(outcome) => Ok(outcome.receipt),
            StepResult::Err(err) => Err(err),
        };
        emit(global.pretty, &result_json(name, Some(step.line), &result))?;
    }
    if state {
        emit(global.pretty, &json!({ "state": engine.state() }))?;
    }
    Ok(ExitCode::SUCCESS)
}
