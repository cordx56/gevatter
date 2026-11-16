use std::env;
use std::process::{Command, Stdio};

include!("./src/parse.rs");

fn main() {
    if !env::var("RUSTC_VERSION").is_ok_and(|v| !v.is_empty()) {
        let rustc_version = obtain_rustc_version();
        println!("cargo::rustc-env=RUSTC_VERSION={rustc_version}");
    }
    let host_tuple = rustc(&["--print=host-tuple"]).trim().to_owned();
    println!("cargo::rustc-env=RUSTC_HOST_TUPLE={host_tuple}");
}

fn rustc(args: &[&str]) -> String {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let mut cmd = Command::new(rustc);
    cmd.args(args).stdout(Stdio::piped());
    let out = cmd
        .spawn()
        .expect("failed to spawn rustc")
        .wait_with_output()
        .expect("failed to execute rustc")
        .stdout;
    String::from_utf8(out).expect("rustc's output is not UTF-8")
}

fn obtain_rustc_version() -> String {
    let out = rustc(&["-vV"]);
    let release = parse_rustc_version(&out).expect("failed to get version number");
    release.to_owned()
}
