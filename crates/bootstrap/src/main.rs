use std::env;
use std::os::unix::process::CommandExt;
use toolchain::BUILD_COMPONENTS;

fn main() {
    let toolchain_root = toolchain::ensure_rust_toolchain(env!("RUSTC_VERSION"), BUILD_COMPONENTS)
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
