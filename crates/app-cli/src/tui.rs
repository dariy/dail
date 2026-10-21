//! `dail tui`: the terminal UI over the selected profile (`ARCHITECTURE.md §7.4`).

use std::io::{IsTerminal, Write as _};
use std::process::ExitCode;

use anyhow::bail;
use app_tui::{Exit, Target, Tui};

use crate::cli::{BackendKind, GlobalArgs};
use crate::runtime::{self, Claimed, Listen, Owned};

/// Runs the TUI: attaches to the profile's owner, or owns the profile and listens on its socket
/// (so `dail account add`, `dail exec` and `dail mcp` attach to it).
pub fn tui(global: &GlobalArgs) -> anyhow::Result<ExitCode> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("`dail tui` needs a terminal on stdin and stdout");
    }
    let runtime = runtime::build()?;
    let claimed = runtime.block_on(runtime::claim(global, Listen::Yes))?;
    let owns = matches!(claimed, Claimed::Owned(_));
    let tui = Tui::default()
        .with_lang_tag(std::env::var("DAIL_LANG").ok().as_deref())
        // https://no-color.org: set and not empty.
        .with_no_color(std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()))
        .with_setup_hint(setup_line(global))
        .owning_session(owns);
    let (target, held) = match claimed {
        Claimed::Attached(conn) => (Target::Attach(conn), None),
        Claimed::Owned(Owned {
            session,
            server,
            lock,
        }) => (Target::Own(session), Some((server, lock))),
    };
    let exit = app_tui::run(&runtime, target, tui)?;
    // The session has shut down: stop listening, then release the profile.
    drop(held);
    match exit {
        Exit::Quit => Ok(ExitCode::SUCCESS),
        Exit::SessionGone => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "dail: the dail process that owned this profile went away, so the TUI closed"
            );
            Ok(ExitCode::from(2))
        }
        Exit::Hangup(reason) => {
            tracing::info!(reason, "the TUI ended");
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The `dail account add` line for this profile, shown while there are no accounts.
pub fn setup_line(global: &GlobalArgs) -> String {
    let mut line = "dail account add --preset gmail --email you@gmail.com".to_owned();
    if global.backend == BackendKind::Memory {
        line.push_str(" --backend memory");
    }
    if let Some(dir) = &global.profile_dir {
        line.push_str(&format!(" --profile-dir {}", dir.display()));
    } else if global.profile != "default" {
        line.push_str(&format!(" --profile {}", global.profile));
    }
    line
}

/// Whether plain `dail` should start the TUI: on Linux without a display (no `DISPLAY` and no
/// `WAYLAND_DISPLAY`) when it runs in a terminal. Everywhere else it is the GUI.
pub fn prefers_tui(
    linux: bool,
    display: Option<&std::ffi::OsStr>,
    wayland: Option<&std::ffi::OsStr>,
    terminal: bool,
) -> bool {
    let set = |v: Option<&std::ffi::OsStr>| v.is_some_and(|v| !v.is_empty());
    linux && terminal && !set(display) && !set(wayland)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn plain_dail_starts_the_tui_only_without_a_display_in_a_terminal() {
        let x = Some(OsStr::new(":0"));
        let empty = Some(OsStr::new(""));
        assert!(prefers_tui(true, None, None, true));
        assert!(prefers_tui(true, empty, None, true));
        assert!(!prefers_tui(true, x, None, true));
        assert!(!prefers_tui(
            true,
            None,
            Some(OsStr::new("wayland-0")),
            true
        ));
        assert!(!prefers_tui(true, None, None, false));
        assert!(!prefers_tui(false, None, None, true));
    }
}
