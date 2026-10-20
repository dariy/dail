//! The injected context of every `dispatch` / `ingest` call.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::IdGen;

/// Everything non-deterministic that a call needs, supplied by the runtime. `app-core` never reads
/// clocks or RNGs itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ctx {
    /// Current time in Unix milliseconds.
    pub now_ms: i64,
    /// Seed for new IDs created by this call.
    pub id_seed: u128,
    /// The front-end the input came from.
    pub origin: Origin,
}

impl Ctx {
    /// A fresh ID generator for this call.
    pub const fn ids(&self) -> IdGen {
        IdGen::new(self.id_seed)
    }
}

/// The front-end (or runtime component) an input came from. Policies such as send approval
/// depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// The desktop GUI, including the command palette.
    Gui,
    /// The terminal UI (`dail tui`), including its palette. A user origin like the GUI.
    Tui,
    /// `dail exec`.
    Cli,
    /// `dail batch`.
    Batch,
    /// The MCP server (`dail mcp`). Its sends become approval requests.
    Mcp,
    /// The MCP server of a session the user started with `dail mcp --allow-send`: its sends
    /// leave without approval.
    McpAllowSend,
    /// The runtime itself, e.g. effect results and timer ticks.
    Runtime,
}

impl Origin {
    /// Whether the input came from an agent (MCP), with or without `--allow-send`.
    pub const fn is_agent(self) -> bool {
        matches!(self, Self::Mcp | Self::McpAllowSend)
    }

    /// Whether the input came from the user directly: the GUI, the TUI, `dail exec` or
    /// `dail batch`. Only these may decide approval requests.
    pub const fn is_user(self) -> bool {
        matches!(self, Self::Gui | Self::Tui | Self::Cli | Self::Batch)
    }

    /// The `snake_case` name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gui => "gui",
            Self::Tui => "tui",
            Self::Cli => "cli",
            Self::Batch => "batch",
            Self::Mcp => "mcp",
            Self::McpAllowSend => "mcp_allow_send",
            Self::Runtime => "runtime",
        }
    }
}
