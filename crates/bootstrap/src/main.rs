use std::env;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use toolchain::BUILD_COMPONENTS;

static RUST_COMPONENTS: LazyLock<Vec<String>> = LazyLock::new(|| {
    env::var("RUST_COMPONENTS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|v| !v.is_empty())
        .chain(BUILD_COMPONENTS.iter().map(|s| s.to_string()))
        .collect()
});

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(true)
        .with_level(true)
        .init();

    let toolchain_root = toolchain::ensure_rust_toolchain(
        env!("RUSTC_VERSION"),
        RUST_COMPONENTS.iter().map(|v| v.as_str()),
    )
    .expect("failed to ensure Rust toolchain");

    let mut args = env::args_os().skip(1);
    let mut options = Vec::new();
    let command = loop {
        match args.next() {
            Some(arg) => {
                if !arg.to_string_lossy().starts_with("--") {
                    break arg;
                } else {
                    options.push(arg);
                }
            }
            _ => {
                println!("Options:");
                println!("    --with-driver=[driver]");
                return;
            }
        }
    };

    let mut with_driver = None;
    for option in options {
        if option.to_string_lossy().starts_with("--with-driver=") {
            let split = option.to_string_lossy().find("=").unwrap();
            let path = &option.as_bytes()[(split + 1)..];
            with_driver = Some(
                PathBuf::from(OsString::from_vec(path.to_vec()))
                    .canonicalize()
                    .expect("failed to canonicalize driver path"),
            );
        }
    }

    let envs = [
        (
            "PATH",
            toolchain::prepend_paths("PATH", [toolchain_root.join("bin")])
                .expect("failed to make PATH"),
        ),
        (
            toolchain::DYLIB_PATH,
            toolchain::prepend_paths(toolchain::DYLIB_PATH, [toolchain_root.join("lib")])
                .expect("failed to make DYLIB_PATH"),
        ),
    ]
    .into_iter()
    .chain(
        with_driver
            .into_iter()
            .map(|v| ("GEVATTER_DRIVER", v.into_os_string())),
    );

    let error = Command::new(command)
        .envs(envs)
        .args(args)
        .env("RUSTC_BOOTSTRAP", "1")
        .exec();
    panic!("{error}");
}
