use clap::Parser;
use std::env;
use std::ffi::OsStr;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::ExitCode;

mod cli;
mod toolchain;

use cli::{Cli, Command, ToolchainCommand};

fn exec(
    program: impl AsRef<OsStr>,
    args: &[impl AsRef<OsStr>],
    options: &[(&str, &str)],
) -> io::Result<()> {
    let mut command = toolchain::command(program)?;
    command
        .args(args)
        .envs(options.iter().map(|(name, value)| (name, value)));
    Err(command.exec())
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("GEVATTER_LOG"))
        .with_target(true)
        .with_level(true)
        .init();

    let args = env::args_os()
        .enumerate()
        .filter(|(index, arg)| *index != 1 || arg != "gevatter")
        .map(|(_, arg)| arg);
    let cli = Cli::parse_from(args);

    let result = match cli.command.unwrap_or(Command::Check {
        allow_fixable: false,
        allow_warnings: false,
        show_all_diag: false,
        no_generalize: false,
        preserve_binding: false,
        args: Vec::new(),
    }) {
        Command::Run { args } => {
            let args_iter = std::iter::once("run");
            let args: Vec<_> = if !args.is_empty() {
                args_iter
                    .chain(std::iter::once("--"))
                    .chain(args.iter().map(|v| v.as_str()))
                    .collect()
            } else {
                args_iter.collect()
            };
            exec(
                "cargo",
                &args,
                &[("GEVATTER_LINT", "0"), ("GEVATTER_GENERALIZE", "1")],
            )
        }
        Command::Check {
            allow_fixable,
            allow_warnings,
            show_all_diag,
            no_generalize,
            preserve_binding,
            args,
        } => {
            let args: Vec<_> = std::iter::once("check")
                .chain(args.iter().map(|v| v.as_str()))
                .filter(|v| *v != "--keep-going")
                .chain(std::iter::once("--keep-going"))
                .collect();
            exec(
                "cargo",
                &args,
                &[
                    ("GEVATTER_LINT", "1"),
                    ("GEVATTER_LINT_ONLY", if !show_all_diag { "1" } else { "0" }),
                    (
                        "GEVATTER_GENERALIZE",
                        if !no_generalize { "1" } else { "0" },
                    ),
                    (
                        "GEVATTER_ALLOW_FIXABLE",
                        if allow_fixable { "1" } else { "0" },
                    ),
                    (
                        "CARGO_BUILD_WARNINGS",
                        if allow_warnings || allow_fixable {
                            "warn"
                        } else {
                            "deny"
                        },
                    ),
                    (
                        "GEVATTER_PRESERVE_BINDING",
                        if preserve_binding { "1" } else { "0" },
                    ),
                ],
            )
        }
        Command::Fix {
            no_generalize,
            preserve_binding,
            args,
        } => exec(
            "cargo",
            &[vec!["fix".to_owned()], args].concat(),
            &[
                ("GEVATTER_LINT", "1"),
                (
                    "GEVATTER_GENERALIZE",
                    if !no_generalize { "1" } else { "0" },
                ),
                (
                    "GEVATTER_PRESERVE_BINDING",
                    if preserve_binding { "1" } else { "0" },
                ),
            ],
        ),
        Command::Toolchain(ToolchainCommand::Install) => {
            toolchain::install_toolchain().map(|path| println!("{}", path.display()))
        }
        Command::Toolchain(ToolchainCommand::Remove) => toolchain::remove_toolchain(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
