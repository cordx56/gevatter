#![feature(rustc_private)]

mod run;

fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("GEVATTER_LOG"))
        .with_target(true)
        .with_level(true)
        .init();

    run::run_compiler()
}
