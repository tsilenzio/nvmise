//! Project tasks: `cargo xtask <task>`.

mod release;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <task>

tasks:
  release [--unsigned]   build the macOS program for a release into target/dist/
  help                   show this list";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["help"] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["release", rest @ ..] => release::run(rest),
        _ => {
            eprintln!(
                "xtask: there is no task {}. Run cargo xtask help.",
                args.join(" ")
            );
            ExitCode::from(2)
        }
    }
}

/// The folder the project is in: the one above this package's.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the project folder")
        .to_path_buf()
}
