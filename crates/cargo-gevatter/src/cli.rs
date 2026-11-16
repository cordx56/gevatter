use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, bin_name = "cargo gevatter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Run {
        #[arg(last = true, required = true)]
        args: Vec<String>,
    },
    Lint {
        #[arg(long)]
        allow_permissive: bool,
        #[arg(long)]
        show_all_diag: bool,
        #[arg(long)]
        no_preserve_binding: bool,
    },
    Fix {
        #[arg(short, long)]
        generalize: bool,
        #[arg(long)]
        no_preserve_binding: bool,
        #[arg(last = true)]
        args: Vec<String>,
    },
    #[command(subcommand)]
    Toolchain(ToolchainCommand),
}

#[derive(Subcommand)]
pub enum ToolchainCommand {
    Install,
    Remove,
}
