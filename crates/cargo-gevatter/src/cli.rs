use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, bin_name = "cargo gevatter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run `cargo run` with lifetime annotations fixed in memory
    Run {
        /// Arguments passed to the program
        #[arg(last = true, required = false)]
        args: Vec<String>,
    },
    /// Check lifetime annotations
    Check {
        /// Report fixable errors as warnings (implies `--allow-warnings`)
        #[arg(long)]
        allow_fixable: bool,
        /// Exit successfully even with warnings (including non-permissive annotations)
        #[arg(long)]
        allow_warnings: bool,
        /// Also show diagnostics emitted by `rustc` itself
        #[arg(long)]
        show_all_diag: bool,
        /// Do not report generalizable (non-permissive) annotations
        #[arg(short, long)]
        no_generalize: bool,
        /// Skip changes that switch lifetime parameters between early-bound and late-bound
        #[arg(long)]
        preserve_binding: bool,
        /// Skip changes that switch lifetime parameters only from late-bound to early-bound
        #[arg(long)]
        allow_early_to_late: bool,
        /// Arguments passed to `cargo check`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Auto fix lifetime annotations
    Fix {
        /// Do not fix generalizable (non-permissive) annotations
        #[arg(short, long)]
        no_generalize: bool,
        /// Skip changes that switch lifetime parameters between early-bound and late-bound
        #[arg(long)]
        preserve_binding: bool,
        /// Skip changes that switch lifetime parameters only from late-bound to early-bound
        #[arg(long)]
        allow_early_to_late: bool,
        /// Passed to `cargo fix`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Toolchain related actions
    #[command(subcommand)]
    Toolchain(ToolchainCommand),
}
impl Default for Command {
    fn default() -> Self {
        Self::Check {
            allow_fixable: false,
            allow_warnings: false,
            show_all_diag: false,
            no_generalize: false,
            preserve_binding: false,
            allow_early_to_late: false,
            args: Vec::new(),
        }
    }
}

#[derive(Subcommand)]
pub enum ToolchainCommand {
    Install,
    Remove,
}
