use gevatter_lib::driver;
use std::{env, process::ExitCode};

fn option_values<'a>(args: &'a [String], key: &'a str) -> impl Iterator<Item = &'a str> {
    args.iter().enumerate().filter_map(move |(index, arg)| {
        if arg == key {
            args.get(index + 1).map(String::as_str)
        } else {
            arg.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix('='))
        }
    })
}

fn is_passthrough(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "-vV" || arg == "--version" || arg.starts_with("--print"))
        // FIXME: we would like to avoid this;
        || option_values(args, "--crate-name").any(|name| name.starts_with("build_script_"))
        || option_values(args, "--crate-type")
            .flat_map(|types| types.split(','))
            .any(|crate_type| crate_type == "proc-macro")
}

pub fn run_compiler() -> ExitCode {
    let original_args: Vec<_> = env::args().collect();

    // this is intended to check that the current compilation is a crate in a workspace
    // that is a current target
    if original_args.first() != original_args.get(1) {
        return driver::DefaultRustc::new(original_args).run();
    }

    let args: Vec<String> = original_args.iter().skip(1).cloned().collect();
    if is_passthrough(&args) {
        return driver::DefaultRustc::new(args).run();
    }

    driver::GevatterDriver::new(original_args).run(&args)
}
