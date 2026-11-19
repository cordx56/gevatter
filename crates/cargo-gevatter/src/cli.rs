use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, bin_name = "cargo gevatter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Pass given arguments to `cargo run` with fixed sources
    Run {
        #[arg(last = true, required = false)]
        args: Vec<String>,
    },
    /// Check lifetime annotations
    Check {
        /// Allow all fixable errors
        #[arg(long)]
        allow_fixable: bool,
        /// Allow warnings (including non-permissive annotations)
        #[arg(long)]
        allow_warnings: bool,
        /// Show all diagnostics by other than `rustc`
        #[arg(long)]
        show_all_diag: bool,
        /// Do not report generalizable (non-permissive) annotations
        #[arg(short, long)]
        no_generalize: bool,
        /// Preserve existing bindings
        #[arg(long)]
        preserve_binding: bool,
        /// Arguments passed to `cargo check`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Auto fix lifetime annotations
    Fix {
        /// Do not fix generalizable (non-permissive) annotations
        #[arg(short, long)]
        no_generalize: bool,
        /// Preserve existing bindings
        #[arg(long)]
        preserve_binding: bool,
        /// Passed to `cargo fix`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Toolchain related actions
    #[command(subcommand)]
    Toolchain(ToolchainCommand),
}

#[derive(Subcommand)]
pub enum ToolchainCommand {
    Install,
    Remove,
}
