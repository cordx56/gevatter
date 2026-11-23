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
        #[arg(long)]
        no_generalize: bool,
        /// How strictly to keep lifetime parameters early-bound or late-bound
        ///
        /// 0: do not preserve; allow any change
        /// 1: skip only changes that turn the function's own lifetime parameters
        ///    from late-bound into early-bound
        /// 2: fully preserve; skip every change between early-bound and late-bound
        #[arg(
            short,
            long,
            default_value_t = 1,
            verbatim_doc_comment,
            value_parser = clap::value_parser!(u8).range(0..=2)
        )]
        preserve_binding: u8,
        /// Arguments passed to `cargo check`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Automatically fix lifetime annotations
    Fix {
        /// Do not fix generalizable (non-permissive) annotations
        #[arg(long)]
        no_generalize: bool,
        /// How strictly to keep lifetime parameters early-bound or late-bound
        ///
        /// 0: do not preserve; allow any change
        /// 1: skip only changes that turn the function's own lifetime parameters
        ///    from late-bound into early-bound
        /// 2: fully preserve; skip every change between early-bound and late-bound
        #[arg(
            short,
            long,
            default_value_t = 1,
            verbatim_doc_comment,
            value_parser = clap::value_parser!(u8).range(0..=2)
        )]
        preserve_binding: u8,
        /// Arguments passed to `cargo fix`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Manage the Gevatter toolchain
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
            preserve_binding: 1,
            args: Vec::new(),
        }
    }
}

/// Environment variables that tell the driver the `--preserve-binding` level
pub fn preserve_binding_level(level: u8) -> [(&'static str, &'static str); 2] {
    [
        (
            "GEVATTER_PRESERVE_BINDING",
            if 0 < level { "1" } else { "0" },
        ),
        (
            "GEVATTER_ALLOW_EARLY_TO_LATE",
            if level < 2 { "1" } else { "0" },
        ),
    ]
}

#[derive(Subcommand)]
pub enum ToolchainCommand {
    /// Download and install the Gevatter driver and its Rust toolchain
    Install,
    /// Remove the installed Rust toolchains
    Remove,
}
