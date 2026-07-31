use std::process::ExitCode;

use lazytools::{cli, tui};
use lazytools_core::registry::Registry;

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let registry = Registry::new();

    if args.len() > 1 {
        return cli::run(&registry, args);
    }

    match tui::run(registry) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
