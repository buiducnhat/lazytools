mod cli;

use std::process::ExitCode;

use lazytools_core::registry::Registry;

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let registry = Registry::new();

    if args.len() > 1 {
        cli::run(&registry, args)
    } else {
        // P2 thay bằng TUI thật.
        eprintln!("TUI chưa có, xem `lazytools --help`");
        ExitCode::from(2)
    }
}
