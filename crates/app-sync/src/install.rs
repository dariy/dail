//! How the running dail was installed, and installing an update that way (Wave 7 S4, ADR 0011).
//!
//! | Install kind | Strategy |
//! |---|---|
//! | AppImage | the new AppImage is copied next to `$APPIMAGE` and renamed over it |
//! | plain binary | `self-replace` swaps the running executable |
//! | `.deb` | `pkexec apt-get install -y <file>`; without `pkexec`, the command to run |
//! | MSI | `msiexec /i <file>` is started; the user closes dail |
//! | Android, iOS | none: the apps open the release page |
//!
//! [`STRATEGIES`] is that table as data; [`install`] runs it.

use std::path::{Path, PathBuf};

use app_core::{Event, InstallKind, UpdateHost};

/// What [`install`] does for an install kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Copy next to the given file, then rename over it.
    ReplaceFile,
    /// Replace the running executable (`self-replace`).
    ReplaceSelf,
    /// Run a privileged package install through `pkexec`.
    Pkexec,
    /// Start the Windows installer.
    Msiexec,
    /// Only tell the user.
    NotifyOnly,
}

/// The strategy of every install kind.
pub const STRATEGIES: &[(InstallKind, Strategy)] = &[
    (InstallKind::Appimage, Strategy::ReplaceFile),
    (InstallKind::Binary, Strategy::ReplaceSelf),
    (InstallKind::Deb, Strategy::Pkexec),
    (InstallKind::Msi, Strategy::Msiexec),
    (InstallKind::Android, Strategy::NotifyOnly),
    (InstallKind::Ios, Strategy::NotifyOnly),
];

/// The strategy for `kind`.
pub fn strategy(kind: InstallKind) -> Strategy {
    STRATEGIES
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(Strategy::NotifyOnly, |(_, s)| *s)
}

/// The build's target triple as release artifacts name it.
pub fn target() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "windows" => format!("{arch}-pc-windows-msvc"),
        "macos" => format!("{arch}-apple-darwin"),
        "android" => "android".to_owned(),
        "ios" => "ios".to_owned(),
        _ => format!("{arch}-unknown-linux-gnu"),
    }
}

/// What the environment says about the install: `$APPIMAGE`, the running executable, and
/// whether dpkg lists the package's files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    /// `$APPIMAGE`.
    pub appimage: Option<PathBuf>,
    /// `std::env::current_exe()`.
    pub exe: Option<PathBuf>,
    /// `/var/lib/dpkg/info/dail.list` exists.
    pub dpkg_lists_dail: bool,
    /// `%ProgramFiles%`.
    pub program_files: Option<PathBuf>,
}

impl Probe {
    /// The probe of this process.
    pub fn here() -> Self {
        Self {
            appimage: std::env::var_os("APPIMAGE").map(PathBuf::from),
            exe: std::env::current_exe().ok(),
            dpkg_lists_dail: Path::new("/var/lib/dpkg/info/dail.list").exists(),
            program_files: std::env::var_os("ProgramFiles").map(PathBuf::from),
        }
    }

    /// The install kind it shows.
    pub fn kind(&self) -> InstallKind {
        if self.appimage.as_ref().is_some_and(|p| p.is_absolute()) {
            return InstallKind::Appimage;
        }
        let exe = self.exe.as_deref();
        if self.dpkg_lists_dail && exe == Some(Path::new("/usr/bin/dail")) {
            return InstallKind::Deb;
        }
        if let (Some(exe), Some(program_files)) = (exe, &self.program_files)
            && exe.starts_with(program_files.join("dail"))
        {
            return InstallKind::Msi;
        }
        InstallKind::Binary
    }
}

/// The running desktop build, for `Event::UpdateHostDetected`.
pub fn desktop_host() -> UpdateHost {
    UpdateHost {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        target: target(),
        install: Probe::here().kind(),
    }
}

/// `Effect::InstallUpdate`: installs `path` the way `kind` needs.
pub async fn install(version: String, kind: InstallKind, path: String) -> Event {
    let path = PathBuf::from(path);
    let result = match strategy(kind) {
        Strategy::ReplaceFile => match std::env::var_os("APPIMAGE") {
            Some(target) => replace_file(&path, Path::new(&target)).await,
            None => Err("$APPIMAGE is not set; start dail from its AppImage".to_owned()),
        },
        Strategy::ReplaceSelf => replace_self(path).await,
        Strategy::Pkexec => pkexec_apt(&path).await,
        Strategy::Msiexec => msiexec(&path),
        Strategy::NotifyOnly => Err("this install updates from the release page".to_owned()),
    };
    match result {
        Ok(()) => Event::UpdateInstalled { version },
        Err(reason) => Event::UpdateFailed { reason },
    }
}

/// Copies `new` to a temporary file beside `target`, then renames it over `target`, keeping
/// `target`'s permissions. The rename is atomic, so a running AppImage keeps its old inode.
pub async fn replace_file(new: &Path, target: &Path) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| format!("{} has no directory", target.display()))?;
    let name = target
        .file_name()
        .ok_or_else(|| format!("{} has no file name", target.display()))?;
    let temp = dir.join(format!(".{}.update", name.to_string_lossy()));
    tokio::fs::copy(new, &temp)
        .await
        .map_err(|err| format!("cannot write beside {}: {err}", target.display()))?;
    if let Ok(meta) = tokio::fs::metadata(target).await {
        let _ = tokio::fs::set_permissions(&temp, meta.permissions()).await;
    }
    if let Err(err) = tokio::fs::rename(&temp, target).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(format!("cannot replace {}: {err}", target.display()));
    }
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn replace_self(path: PathBuf) -> Result<(), String> {
    tokio::task::spawn_blocking(move || self_replace::self_replace(&path))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| format!("cannot replace the running dail: {err}"))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
async fn replace_self(_path: PathBuf) -> Result<(), String> {
    Err("this install updates from the release page".to_owned())
}

/// The directories of `$PATH` that hold `program`.
fn on_path(program: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// The command a user runs to install a `.deb` by hand.
pub fn apt_command(path: &Path) -> String {
    format!("sudo apt-get install {}", path.display())
}

async fn pkexec_apt(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{} is not an absolute path", path.display()));
    }
    let Some(pkexec) = on_path("pkexec") else {
        return Err(format!(
            "pkexec is not installed; run: {}",
            apt_command(path)
        ));
    };
    // `apt-get install` takes a path to a local file when it contains a `/`.
    let status = tokio::process::Command::new(pkexec)
        .args(["apt-get", "install", "-y"])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .status()
        .await
        .map_err(|err| format!("cannot start pkexec: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "the package install failed ({status}); run: {}",
            apt_command(path)
        ))
    }
}

fn msiexec(path: &Path) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("MSI updates install on Windows only".to_owned());
    }
    std::process::Command::new("msiexec")
        .arg("/i")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("cannot start msiexec: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_one_strategy() {
        for kind in [
            InstallKind::Appimage,
            InstallKind::Binary,
            InstallKind::Deb,
            InstallKind::Msi,
            InstallKind::Android,
            InstallKind::Ios,
        ] {
            assert_eq!(
                STRATEGIES.iter().filter(|(k, _)| *k == kind).count(),
                1,
                "{kind:?}"
            );
            assert_eq!(
                strategy(kind) == Strategy::NotifyOnly,
                !kind.self_updates(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn the_probe_finds_the_install_kind() {
        let probe = |appimage: Option<&str>, exe: &str, dpkg: bool, pf: Option<&str>| Probe {
            appimage: appimage.map(PathBuf::from),
            exe: Some(PathBuf::from(exe)),
            dpkg_lists_dail: dpkg,
            program_files: pf.map(PathBuf::from),
        };
        assert_eq!(
            probe(
                Some("/home/me/dail.AppImage"),
                "/tmp/.mount_x/usr/bin/dail",
                false,
                None
            )
            .kind(),
            InstallKind::Appimage
        );
        assert_eq!(
            probe(Some("rel"), "/usr/bin/dail", true, None).kind(),
            InstallKind::Deb
        );
        assert_eq!(
            probe(None, "/usr/bin/dail", false, None).kind(),
            InstallKind::Binary
        );
        assert_eq!(
            probe(None, "/home/me/bin/dail", true, None).kind(),
            InstallKind::Binary
        );
        if cfg!(windows) {
            assert_eq!(
                probe(
                    None,
                    r"C:\Program Files\dail\dail.exe",
                    false,
                    Some(r"C:\Program Files")
                )
                .kind(),
                InstallKind::Msi
            );
        }
        assert_eq!(
            probe(None, "/opt/dail/dail", false, Some("/opt")).kind(),
            InstallKind::Msi
        );
    }

    #[test]
    fn targets_name_release_artifacts() {
        let target = target();
        assert!(
            target.ends_with("-unknown-linux-gnu")
                || target.ends_with("-pc-windows-msvc")
                || target.ends_with("-apple-darwin")
                || (cfg!(any(target_os = "android", target_os = "ios"))
                    && target == std::env::consts::OS),
            "{target}"
        );
    }

    #[tokio::test]
    async fn a_file_is_replaced_atomically() {
        let dir = std::env::temp_dir().join(format!("dail-install-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("dail.AppImage");
        let new = dir.join("new");
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        replace_file(&new, &target).await.unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(!dir.join(".dail.AppImage.update").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn mobile_kinds_do_not_install() {
        let event = install("0.2.0".to_owned(), InstallKind::Android, "/x".to_owned()).await;
        assert!(matches!(event, Event::UpdateFailed { .. }), "{event:?}");
    }
}
