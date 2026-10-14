//! `bothy` command-line tool. Early development: only `--version` and `--help` work so far.

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn help() -> String {
    format!(
        "bothy {VERSION}\n\
         Serverless, local-first sync over your own Google Drive.\n\
         \n\
         This is an early development release. Planned commands:\n\
         \x20 init, join, put, get, list, del, sync, status\n\
         \n\
         Usage: bothy [--version | --help]\n\
         \n\
         Project: https://github.com/bothy-sync/bothy\n"
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("-V" | "--version") => {
            println!("bothy {VERSION} (format v{})", bothy::FORMAT_VERSION);
            ExitCode::SUCCESS
        }
        None | Some("-h" | "--help" | "help") => {
            print!("{}", help());
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("bothy: '{other}' is not available yet in this early release.\n");
            eprint!("{}", help());
            ExitCode::from(2)
        }
    }
}
