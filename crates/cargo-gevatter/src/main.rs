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
    options: &[(&str, bool)],
) -> io::Result<()> {
    let mut command = toolchain::command(program)?;
    command.args(args).envs(
        options
            .iter()
            .map(|(name, enabled)| (name, if *enabled { "1" } else { "0" })),
    );
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
        allow_permissive: false,
        show_all_diag: false,
        preserve_binding: false,
    }) {
        Command::Run { args } => {
            let args_iter = std::iter::once("run");
            let args: Vec<_> = if args.is_empty() {
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
                &[("GEVATTER_LINT", false), ("GEVATTER_GENERALIZE", true)],
            )
        }
        Command::Check {
            allow_fixable,
            allow_permissive,
            show_all_diag,
            preserve_binding,
        } => exec(
            "cargo",
            &["check"],
            &[
                ("GEVATTER_LINT", true),
                ("GEVATTER_LINT_ONLY", !show_all_diag),
                ("GEVATTER_GENERALIZE", true),
                ("GEVATTER_ALLOW_FIXABLE", allow_fixable),
                ("GEVATTER_ALLOW_PERMISSIVE", allow_permissive),
                ("GEVATTER_PRESERVE_BINDING", preserve_binding),
            ],
        ),
        Command::Fix {
            no_generalize,
            preserve_binding,
            args,
        } => exec(
            "cargo",
            &[vec!["fix".to_owned()], args].concat(),
            &[
                ("GEVATTER_LINT", true),
                ("GEVATTER_GENERALIZE", !no_generalize),
                ("GEVATTER_PRESERVE_BINDING", preserve_binding),
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
