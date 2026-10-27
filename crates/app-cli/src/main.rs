//! `dail` — the single binary that hosts every front-end (see `ARCHITECTURE.md §3`).
//!
//! Exit codes: `0` success, `1` a command was rejected, `2` usage or runtime error.

mod account;
mod cli;
mod doctor;
mod exec;
mod mcp_http;
mod output;
mod replay;
mod runtime;
mod schema;
mod tui;
mod update;

use std::fs::File;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use crate::cli::{AccountCmd, Cli, GlobalArgs, Sub};

fn main() -> ExitCode {
    let cli = Cli::parse_with_registry_help();
    let terminal_ui = match cli.command {
        Some(Sub::Tui) => true,
        None => tui::prefers_tui(
            cfg!(target_os = "linux"),
            std::env::var_os("DISPLAY").as_deref(),
            std::env::var_os("WAYLAND_DISPLAY").as_deref(),
            std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        ),
        Some(_) => false,
    };
    if terminal_ui {
        init_tracing_to_file(&cli.global);
    } else {
        init_tracing();
    }
    match run(cli, terminal_ui) {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(std::io::stderr().lock(), "error: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_env("DAIL_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(filter())
        .with_writer(std::io::stderr)
        .init();
}

/// In the TUI stderr is the screen: logs go to `<profile>/logs/tui.log` (nowhere for a private
/// memory session).
fn init_tracing_to_file(global: &GlobalArgs) {
    let file = runtime::profile_root(global)
        .ok()
        .flatten()
        .and_then(|root| open_log(&app_io::ProfileDirs::at(&root).logs_dir()));
    match file {
        Some(file) => tracing_subscriber::fmt()
            .with_env_filter(filter())
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .init(),
        None => tracing_subscriber::fmt()
            .with_env_filter(filter())
            .with_writer(std::io::sink)
            .init(),
    }
}

fn open_log(dir: &std::path::Path) -> Option<File> {
    std::fs::create_dir_all(dir).ok()?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        options.mode(0o600);
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    options.open(dir.join("tui.log")).ok()
}

fn run(cli: Cli, terminal_ui: bool) -> anyhow::Result<ExitCode> {
    if terminal_ui {
        return tui::tui(&cli.global);
    }
    match cli.command {
        Some(Sub::Exec {
            command,
            args,
            dry_run,
        }) => runtime::block_on(exec::exec(&cli.global, &command, args.as_deref(), dry_run)),
        Some(Sub::Batch { input, keep_going }) => {
            runtime::block_on(exec::batch(&cli.global, &input, keep_going))
        }
        Some(Sub::Account {
            command: AccountCmd::Add(args),
        }) => runtime::block_on(account::add(&cli.global, &args)),
        Some(Sub::Account {
            command: AccountCmd::Password(args),
        }) => runtime::block_on(account::password(&cli.global, &args)),
        Some(Sub::Update { command }) => runtime::block_on(update::run(&cli.global, &command)),
        Some(Sub::Mcp {
            allow_send,
            http: None,
        }) => runtime::block_on(async {
            let front = runtime::connect(&cli.global, runtime::Listen::Yes).await?;
            app_mcp::serve_stdio(front.conn.clone(), allow_send).await?;
            front.close().await?;
            Ok(ExitCode::SUCCESS)
        }),
        Some(Sub::Mcp {
            allow_send,
            http: Some(addr),
        }) => runtime::block_on(mcp_http::serve(&cli.global, allow_send, addr)),
        Some(Sub::Replay { log, state }) => replay::replay(&cli.global, &log, state),
        Some(Sub::Doctor) => runtime::block_on(doctor::doctor(&cli.global)),
        Some(Sub::Schema { command }) => schema::schema(&cli.global, command.as_deref()),
        Some(Sub::Tui) => tui::tui(&cli.global),
        Some(Sub::Gui) | None => {
            let runtime = runtime::build()?;
            let owned = runtime.block_on(runtime::own(&cli.global))?;
            // The listener keeps serving agents while the window is open; `run` shuts the
            // session down when the window closes.
            let runtime::Owned {
                session,
                server,
                lock,
            } = owned;
            app_gui::run(&runtime, session)?;
            drop(server);
            drop(lock);
            Ok(ExitCode::SUCCESS)
        }
    }
}
