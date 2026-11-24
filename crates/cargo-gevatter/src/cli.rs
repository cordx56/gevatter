use clap::{
    ColorChoice, Parser, Subcommand, ValueEnum,
    builder::{Styles, styling::AnsiColor},
};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::BrightGreen.on_default().bold())
    .usage(AnsiColor::BrightGreen.on_default().bold())
    .literal(AnsiColor::BrightCyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::BrightRed.on_default().bold());

#[derive(Parser)]
#[command(version, about, bin_name = "cargo gevatter", color = ColorChoice::Always, styles = STYLES)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run `cargo run` with lifetime annotations fixed in memory
    Run {
        /// Arguments passed to the program
        #[arg(allow_hyphen_values = true)]
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
        #[arg(
            short,
            long,
            value_enum,
            default_value_t = PreserveBinding::default(),
        )]
        preserve_binding: PreserveBinding,
        /// Arguments passed to `cargo check`
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Automatically fix lifetime annotations
    #[command(allow_missing_positional = true)]
    Fix {
        /// Do not fix generalizable (non-permissive) annotations
        #[arg(long)]
        no_generalize: bool,
        /// How strictly to keep lifetime parameters early-bound or late-bound
        #[arg(
            short,
            long,
            value_enum,
            default_value_t = PreserveBinding::default(),
        )]
        preserve_binding: PreserveBinding,
        /// Arguments passed to `cargo fix`
        #[arg(allow_hyphen_values = true)]
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
            preserve_binding: PreserveBinding::default(),
            args: Vec::new(),
        }
    }
}

#[derive(ValueEnum, Default, Clone, Copy, Debug)]
pub enum PreserveBinding {
    /// Do not preserve; allow any change
    Off = 0,
    /// Skip changes that turn the function's own lifetime parameters from late-bound into
    /// early-bound
    #[default]
    Owned = 1,
    /// Skip changes that make any late-bound lifetime early-bound, even when it only starts to
    /// refer to a lifetime parameter of the enclosing `impl`
    ToEarly = 2,
    /// Fully preserve; skip every change between early-bound and late-bound
    Full = 3,
}

#[derive(Subcommand)]
pub enum ToolchainCommand {
    /// Download and install the Gevatter driver and its Rust toolchain
    Install,
    /// Remove the installed Rust toolchains
    Remove,
}
