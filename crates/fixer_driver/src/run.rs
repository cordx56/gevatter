use fixer_lib::driver;
use std::{env, process::ExitCode};

pub fn run_compiler() -> ExitCode {
    let original_args: Vec<_> = env::args().collect();
    let args: Vec<String>;

    if original_args.first() == original_args.get(1) {
        args = original_args.iter().skip(1).map(|v| v.to_owned()).collect();
    } else {
        return driver::run(&original_args, &mut driver::DefaultRustc);
    }

    for arg in &args {
        if arg == "-vV" || arg == "--version" || arg.starts_with("--print") {
            return driver::run(&args, &mut driver::DefaultRustc);
        }
    }

    driver::run(&args, &mut driver::FixerRustc::new(&original_args))
}
