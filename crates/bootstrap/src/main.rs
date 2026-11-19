use std::env;
use std::os::unix::process::CommandExt;
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
    let command = args.next().expect("no extra args");

    let error = Command::new(command)
        .env(
            "PATH",
            toolchain::prepend_paths("PATH", [toolchain_root.join("bin")])
                .expect("failed to make PATH"),
        )
        .env(
            toolchain::DYLIB_PATH,
            toolchain::prepend_paths(toolchain::DYLIB_PATH, [toolchain_root.join("lib")])
                .expect("failed to make DYLIB_PATH"),
        )
        .args(args)
        .env("RUSTC_BOOTSTRAP", "1")
        .exec();
    panic!("{error}");
}
