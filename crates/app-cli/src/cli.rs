//! Command-line definition.

use std::fmt::Write as _;

use app_core::{Command, DeviceRole, ProviderPreset};
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
    /// Run the dail server (ADR 0017): own the profile headless (sync, rules, retries, the
    /// outbox), let `dail exec`, `dail tui` and `dail mcp` attach through the profile socket, and
    /// serve paired devices over TLS until Ctrl+C or SIGTERM. Without `--listen` it listens on
    /// 127.0.0.1:7443 only. `dail serve pair` pairs a device.
    #[command(args_conflicts_with_subcommands = true)]
    Serve {
        #[command(flatten)]
        args: ServeArgs,
        #[command(subcommand)]
        command: Option<ServeCmd>,
    },
    /// Link this profile to a dail server as a device (ADR 0017).
    Remote {
        #[command(subcommand)]
        command: RemoteCmd,
    },
    /// Updates of dail (aliases for the update commands, ADR 0011).
    Update {
        #[command(subcommand)]
        command: UpdateCmd,
    },
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

/// `dail update …`.
#[derive(Debug, Subcommand)]
pub enum UpdateCmd {
    /// Check for a new version now (`check_for_updates`) and print the outcome (`update`).
    /// Exit code 0 if a newer version is available or none is, 1 if the check failed.
    Check,
    /// Check, download and install the newest version (`check_for_updates`, `download_update`,
    /// `install_update`). Without `--yes` it stops after the download and says what it would
    /// install.
    Install {
        /// Install without asking (the install still needs admin rights for `.deb` and MSI).
        #[arg(long)]
        yes: bool,
    },
}

/// `dail serve [--listen …] [--advertise …] [--allow-send]`.
#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Address and port to listen on (repeatable; port 0 picks a free port). Default
    /// 127.0.0.1:7443. Any address is allowed; one outside loopback and private networks
    /// (RFC 1918, Tailscale's 100.64.0.0/10, IPv6 ULA) gets a warning.
    #[arg(long, value_name = "ADDR:PORT")]
    pub listen: Vec<std::net::SocketAddr>,
    /// A name or address devices reach the server by, e.g. a Tailscale MagicDNS name
    /// (repeatable). Shown by `dail serve pair` and written into the certificate.
    #[arg(long, value_name = "HOST")]
    pub advertise: Vec<String>,
    /// Let agent devices send mail without the user's approval (origin `mcp_allow_send`), as
    /// `dail mcp --allow-send` does. By default an agent's `send_draft` creates an approval
    /// request.
    #[arg(long)]
    pub allow_send: bool,
}

/// `dail serve …`.
#[derive(Debug, Subcommand)]
pub enum ServeCmd {
    /// Open a single-use pairing code on the running `dail serve` (alias for `start_pairing`,
    /// valid 10 minutes). Prints the code, its expiry, the server's addresses and its
    /// certificate fingerprint; on the device run `dail remote add`.
    Pair {
        /// What the device may do: `user` (GUI, TUI, CLI; may approve sends) or `agent` (MCP
        /// only, under the agent policy).
        #[arg(long, value_enum)]
        role: RoleArg,
        /// The device's name, e.g. `Laptop` (1 to 64 characters).
        #[arg(long)]
        name: String,
    },
    /// List the paired devices (alias for `list_devices`).
    Devices,
    /// Revoke a paired device (alias for `revoke_device`): its token stops working and its
    /// connections close at once. Asks first unless `--yes` is given.
    Revoke {
        /// The device's ID (see `dail serve devices`).
        id: String,
        /// Revoke without asking.
        #[arg(long)]
        yes: bool,
    },
}

/// `dail remote …`.
#[derive(Debug, Subcommand)]
pub enum RemoteCmd {
    /// Pair this profile with a dail server (aliases for `connect_server` and `trust_server`).
    /// The profile must have no accounts. Shows the certificate fingerprint the server presents
    /// to compare with the one `dail serve` printed, then asks for the pairing code from
    /// `dail serve pair`; `--fingerprint` and `--code` answer both for scripts. Once paired it is
    /// a remote profile (`[remote]` in its `config.toml`): every front end on it connects to the
    /// server.
    Add {
        /// The server as `host[:port]` (default port 7443; `[::1]:7443` for IPv6).
        server: String,
        /// The pairing code from `dail serve pair`.
        #[arg(long)]
        code: Option<String>,
        /// The SHA-256 fingerprint `dail serve` printed (64 hex digits, colons allowed). The
        /// server's certificate must match it exactly.
        #[arg(long)]
        fingerprint: Option<String>,
    },
    /// Print the link to the dail server: status, server, pinned fingerprint, revision, queued
    /// commands and refused ones.
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RoleArg {
    User,
    Agent,
}

impl From<RoleArg> for DeviceRole {
    fn from(role: RoleArg) -> Self {
        match role {
            RoleArg::User => Self::User,
            RoleArg::Agent => Self::Agent,
        }
    }
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
