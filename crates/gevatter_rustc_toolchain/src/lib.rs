use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::LazyLock;

pub mod archive;
pub mod parse;

pub const RUSTC_VERSION: &str = env!("RUSTC_VERSION");
pub const HOST_TUPLE: &str = env!("RUSTC_HOST_TUPLE");
pub const RUNTIME_COMPONENTS: [&str; 3] = ["rustc", "rust-std", "cargo"];
pub const BUILD_COMPONENTS: [&str; 5] = ["rustc", "rust-std", "cargo", "rustc-dev", "llvm-tools"];

const DIST_SERVER: &str = "https://static.rust-lang.org/dist";
pub const DYLIB_PATH: &str = if cfg!(target_os = "macos") {
    "DYLD_FALLBACK_LIBRARY_PATH"
} else {
    "LD_LIBRARY_PATH"
};

pub static INSTALL_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    PathBuf::from(env::var("HOME").expect("failed to get $HOME")).join(".gevatter")
});
pub static TOOLCHAIN_BASE_DIR: LazyLock<PathBuf> = LazyLock::new(|| INSTALL_DIR.join("toolchain"));

pub fn toolchain_dir(channel: &str) -> PathBuf {
    TOOLCHAIN_BASE_DIR.join(format!("{channel}-{HOST_TUPLE}"))
}

fn component_url(channel: &str, component: &str) -> String {
    if component == "rust-src" {
        format!("{DIST_SERVER}/{component}-{channel}.tar.xz")
    } else {
        format!("{DIST_SERVER}/{component}-{channel}-{HOST_TUPLE}.tar.xz")
    }
}

fn install_component(channel: &str, component: &str) -> io::Result<()> {
    let toolchain_root = toolchain_dir(channel);
    let url = component_url(channel, component);
    tracing::info!("install {} from {url}", component);
    archive::download_read(&url, |mut entry| {
        let path = entry.path()?.into_owned();
        let path: Vec<_> = path.components().skip(2).collect();
        if path.is_empty() {
            return Ok(());
        }
        let path = path.into_iter().fold(PathBuf::new(), |acc, x| acc.join(x));
        let dest = toolchain_root.join(&path);
        fs::create_dir_all(dest.parent().unwrap())?;
        entry.unpack(&dest)?;
        tracing::debug!("{} extracted to {}", path.display(), dest.display());
        Ok(())
    })
}

/// install Rust toolchain and returns its path
pub fn install_rust_toolchain<'a>(
    channel: &str,
    components: impl IntoIterator<Item = &'a str>,
) -> io::Result<PathBuf> {
    components
        .into_iter()
        .try_for_each(|component| install_component(channel, component))?;
    Ok(toolchain_dir(channel))
}

// ensure that Rust toolchain is installed
pub fn ensure_rust_toolchain<'a>(
    channel: &str,
    components: impl IntoIterator<Item = &'a str>,
) -> io::Result<PathBuf> {
    let dir = toolchain_dir(channel);
    if !dir.is_dir() {
        install_rust_toolchain(channel, components)?;
    }
    Ok(dir)
}

pub fn remove_rust_toolchains() -> io::Result<()> {
    match fs::remove_dir_all(&*TOOLCHAIN_BASE_DIR) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

pub fn prepend_paths(
    variable: &str,
    front: impl IntoIterator<Item = PathBuf>,
) -> io::Result<OsString> {
    let paths = env::var_os(variable).unwrap_or_default();
    let paths = env::split_paths(&paths).filter(|path| !path.as_os_str().is_empty());
    env::join_paths(front.into_iter().chain(paths)).map_err(io::Error::other)
}
