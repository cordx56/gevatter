use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;
use toolchain::{HOST_TUPLE, TOOLCHAIN_BASE_DIR, archive, parse::parse_rustc_version};

const DRIVER: &str = "gevatter_driver";
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn release_url() -> String {
    format!(
        "https://github.com/cordx56/gevatter/releases/download/v{}",
        PKG_VERSION,
    )
}

fn gevatter_bin_dir() -> PathBuf {
    toolchain::INSTALL_DIR.join("bin")
}
fn driver_cmd() -> String {
    env::var("GEVATTER_DRIVER").unwrap_or(DRIVER.to_string())
}

fn link_driver(driver_path: &Path) -> io::Result<()> {
    let link_path = gevatter_bin_dir().join(DRIVER);
    if fs::read_link(&link_path).is_ok_and(|linked| linked == driver_path) {
        return Ok(());
    }
    fs::remove_file(&link_path).ok();
    fs::create_dir_all(link_path.parent().unwrap())?;
    symlink(driver_path, link_path)
}

fn driver_command() -> Command {
    let mut cmd = Command::new(driver_cmd());
    cmd.env(
        "PATH",
        toolchain::prepend_paths("PATH", [gevatter_bin_dir()]).expect("failed to make PATH"),
    );
    cmd
}

pub fn toolchain_dir() -> Option<PathBuf> {
    let rustc_out = String::from_utf8(
        driver_command()
            .args(["-vV"])
            .stdout(Stdio::piped())
            .spawn()
            .ok()?
            .wait_with_output()
            .ok()?
            .stdout,
    )
    .expect("rustc output is not UTF-8");
    let rustc_version = parse_rustc_version(&rustc_out)?;
    Some(TOOLCHAIN_BASE_DIR.join(format!("{rustc_version}-{HOST_TUPLE}")))
}
pub fn ensure_toolchain() -> io::Result<PathBuf> {
    if let Ok(child) = driver_command()
        .args(["--gevatter-version"])
        .stdout(Stdio::piped())
        .spawn()
        .inspect_err(|_| tracing::warn!("failed to get gevatter_driver version; install toolchain"))
    {
        let ver = String::from_utf8(child.wait_with_output()?.stdout)
            .expect("gevatter_driver's output is not UTF-8");
        let ver = ver.trim();
        tracing::info!("gevatter_driver found: v{ver}");
        if ver == PKG_VERSION {
            return Ok(toolchain_dir().expect("toolchain not found"));
        }
    }
    println!("Installing Gevatter toolchain...");
    install_toolchain()
}

/// Returns Rust toolchain path
pub fn install_toolchain() -> io::Result<PathBuf> {
    let mut rust_version = String::new();
    let mut driver_data = Vec::new();
    archive::download_read(
        &format!("{}/{DRIVER}-{HOST_TUPLE}.tar.xz", release_url()),
        |mut entry| {
            let path = entry.path()?;
            let file_name = path.file_name();
            if file_name.is_some_and(|v| v == "stable-version") {
                entry.read_to_string(&mut rust_version).map(|_| ())
            } else if file_name.is_some_and(|v| v == DRIVER) {
                entry.read_to_end(&mut driver_data).map(|_| ())
            } else {
                Ok(())
            }
        },
    )
    .inspect_err(|e| tracing::error!("failed to download {DRIVER}: {e:?}"))?;
    let toolchain_path =
        toolchain::ensure_rust_toolchain(rust_version.trim(), toolchain::RUNTIME_COMPONENTS)
            .inspect_err(|e| {
                tracing::error!("failed to install Rust toolchain for {DRIVER}: {e:?}")
            })?;

    let driver_path = toolchain_path.join("bin").join(DRIVER);
    let mut driver_file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o755)
        .open(&driver_path)
        .inspect_err(|e| tracing::error!("failed to open {driver_path:?}: {e:?}"))?;
    driver_file
        .write_all(&driver_data)
        .inspect_err(|e| tracing::error!("failed to write {driver_path:?}: {e:?}"))?;
    drop(driver_file);
    link_driver(&driver_path)?;
    Ok(toolchain_path)
}

pub fn remove_toolchain() -> io::Result<()> {
    toolchain::remove_rust_toolchains()
}

pub fn command(program: impl AsRef<OsStr>) -> io::Result<Command> {
    let toolchain_path = ensure_toolchain()?;
    let driver_cmd = driver_cmd();
    let mut command = Command::new(program);
    command
        .env(
            "PATH",
            toolchain::prepend_paths("PATH", [toolchain_path.join("bin")])
                .expect("failed to make PATH"),
        )
        .env("RUSTC", &driver_cmd)
        .env("RUSTC_WORKSPACE_WRAPPER", &driver_cmd)
        .env("RUSTC_BOOTSTRAP", "1");
    Ok(command)
}
