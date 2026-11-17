use std::env;
use std::os::unix::process::CommandExt;
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
    let toolchain_root = toolchain::ensure_rust_toolchain(
        env!("RUSTC_VERSION"),
        RUST_COMPONENTS.iter().map(|v| v.as_str()),
    )
    .expect("failed to ensure Rust toolchain");

    let mut args = env::args_os().skip(1);
    let command = args.next().expect("no extra args");

    let error = toolchain::command(&toolchain_root, command)
        .expect("failed to set $PATH")
        .args(args)
        .env("RUSTC_BOOTSTRAP", "1")
        .exec();
    panic!("{error}");
}
