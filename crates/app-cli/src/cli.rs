//! Command-line definition.

use std::fmt::Write as _;

use app_core::{Command, ProviderPreset};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};

/// Agent-first email client.
#[derive(Debug, Parser)]
#[command(name = "dail", version, about)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Option<Sub>,
}

/// Options shared by every subcommand.
#[derive(Debug, Args)]
pub struct GlobalArgs {
    /// Where effects run: `live` (IMAP/SMTP, from M1) or `memory` (offline fake server).
    #[arg(long, value_enum, default_value_t = BackendKind::Live, global = true)]
    pub backend: BackendKind,
    /// Fake-server content for `--backend memory`: a built-in fixture name or a JSON file path.
    #[arg(long, global = true)]
    pub fixture: Option<String>,
    /// Pretty-print JSON output (one JSON document per result, multi-line).
    #[arg(long, global = true)]
    pub pretty: bool,
    /// Profile name for `--backend live` (under the platform data directory).
    #[arg(long, global = true, default_value = "default")]
    pub profile: String,
    /// Profile directory for `--backend live`; overrides `--profile`.
    #[arg(long, global = true)]
    pub profile_dir: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BackendKind {
    Live,
    Memory,
}

#[derive(Debug, Subcommand)]
pub enum Sub {
    /// Run one command, e.g. `dail exec set_setting --args '{"key":"poll_secs","value":300}'`.
    Exec {
        /// Command name (see the list below or `dail schema`).
        command: String,
        /// Command arguments as a JSON object (default `{}`).
        #[arg(long)]
        args: Option<String>,
        /// Report what the command would do (receipt, effects by name, changed fields) without
        /// doing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Run commands from a JSONL file (`-` for stdin): one `{"cmd":…,"args":…}` per line
    /// (`"dry_run": true` previews it instead), `#` starts a comment. Prints one JSON result
    /// per command.
    Batch {
        /// Path of the JSONL file, or `-` for stdin.
        input: String,
        /// Continue after a rejected command (exit code is still 1 if any failed).
        #[arg(long)]
        keep_going: bool,
    },
    /// Manage mail accounts (aliases for the account commands).
    Account {
        #[command(subcommand)]
        command: AccountCmd,
    },
    /// Serve MCP over stdio: one tool per registry command (origin `mcp`). Attaches to the
    /// running GUI of the profile if there is one.
    Mcp {
        /// Let the agent send mail without the user's approval (origin `mcp_allow_send`). By
        /// default `send_draft` from the agent creates an approval request.
        #[arg(long)]
        allow_send: bool,
        /// Serve Streamable HTTP on this loopback address (e.g. `127.0.0.1:8765`, path `/mcp`)
        /// instead of stdio, until Ctrl+C. Clients send `Authorization: Bearer <token>`, the
        /// token in `<profile>/mcp-token` (ADR 0007).
        #[arg(long, value_name = "ADDR")]
        http: Option<std::net::SocketAddr>,
    },
    /// Open the desktop GUI (the default when no subcommand is given, unless on Linux without a
    /// display in a terminal: then the TUI).
    Gui,
    /// Open the terminal UI. Attaches to the running GUI or `dail mcp` of the profile if there is
    /// one; otherwise owns the profile and lets `dail exec` and `dail mcp` attach to it. Logs go to
    /// `<profile>/logs/tui.log`, because stderr is the screen.
    Tui,
    /// Replay a log (an audit log from `<profile>/logs/replay-YYYYMMDD.jsonl` or a hand-written
    /// one) through a fresh engine, offline: prints one JSON result per record. Nothing is sent
    /// or stored; the profile is not touched.
    Replay {
        /// Path of the JSONL log.
        log: std::path::PathBuf,
        /// Print the final state as a last line `{"state": …}`.
        #[arg(long)]
        state: bool,
    },
    /// Check the profile: config, store integrity, downloaded mail, index, backups, keyring
    /// entries (never their values) and TLS. Prints one JSON line per check
    /// (`{"check","status":"ok|warn|fail","detail"}`); exit code 1 if any failed. With dail
    /// running on the profile, only what the running process reports is checked.
    Doctor,
    /// Print the JSON Schema of commands and events, or the spec of one command.
    Schema {
        /// Print only this command's spec and argument schema.
        #[arg(long)]
        command: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AccountCmd {
    /// Add an account and test its login (alias for `add_account` + `test_account`). The password
    /// is read from a no-echo prompt, or from the first line of stdin with `--password-stdin`;
    /// it is never accepted as an argument.
    Add(AccountAddArgs),
    /// Replace an account's password and test the login (alias for `set_password`). Read like
    /// `add`'s: a no-echo prompt, or the first line of stdin with `--password-stdin`.
    Password(AccountPasswordArgs),
}

#[derive(Debug, Args)]
pub struct AccountPasswordArgs {
    /// The account's email address.
    #[arg(long)]
    pub email: String,
    /// Read the password from the first line of stdin instead of prompting.
    #[arg(long)]
    pub password_stdin: bool,
}

#[derive(Debug, Args)]
pub struct AccountAddArgs {
    /// Provider preset.
    #[arg(long, value_enum)]
    pub preset: PresetArg,
    /// Email address (also the login).
    #[arg(long)]
    pub email: String,
    /// Name shown in the UI (default: the email address).
    #[arg(long)]
    pub display_name: Option<String>,
    /// IMAP server as `host:port[:tls|starttls]` (required for `custom`). Without a security
    /// suffix, ports 143 and 587 use STARTTLS and every other port uses TLS.
    #[arg(long)]
    pub imap: Option<String>,
    /// SMTP server as `host:port[:tls|starttls]` (required for `custom`).
    #[arg(long)]
    pub smtp: Option<String>,
    /// Read the password from the first line of stdin instead of prompting.
    #[arg(long, conflicts_with = "no_password")]
    pub password_stdin: bool,
    /// Add the account without a password (and without testing it).
    #[arg(long)]
    pub no_password: bool,
    /// Do not test the login after adding the account.
    #[arg(long)]
    pub no_test: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PresetArg {
    Hostinger,
    Gmail,
    Custom,
}

impl From<PresetArg> for ProviderPreset {
    fn from(preset: PresetArg) -> Self {
        match preset {
            PresetArg::Hostinger => Self::Hostinger,
            PresetArg::Gmail => Self::Gmail,
            PresetArg::Custom => Self::Custom,
        }
    }
}

impl Cli {
    /// Parses `std::env::args`, with the `exec` help listing every registered command.
    pub fn parse_with_registry_help() -> Self {
        let mut listing = String::from("Commands:\n");
        for spec in Command::specs() {
            let _ = writeln!(
                listing,
                "  {:<16} {:<11} {}",
                spec.name,
                format!("{:?}", spec.kind).to_lowercase(),
                spec.title
            );
        }
        let command = Self::command().mut_subcommand("exec", |exec| exec.after_help(listing));
        let matches = command.get_matches();
        match Self::from_arg_matches(&matches) {
            Ok(cli) => cli,
            Err(err) => err.exit(),
        }
    }
}
