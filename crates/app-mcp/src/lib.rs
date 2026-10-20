//! dail MCP server: tools and resources generated from the command registry.
//!
//! Every `CommandSpec` becomes one MCP tool: name = command name, input schema = the command's
//! `args` JSON Schema, annotations derived from its `CommandKind`. Calls are submitted to the
//! `Session` with `Origin::Mcp`, exactly like `dail exec` does with `Origin::Cli`.

pub mod http;
pub mod prompts;
pub mod resources;
pub mod server;
pub mod tools;

pub use server::{DailServer, INSTRUCTIONS, McpError, serve_stdio};
pub use tools::{annotations, call, call_as, tools};
