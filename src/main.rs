//! The `nvmise` program. The `nvm` shell function is a few lines of shell
//! code that start this program and do what it prints, which is how `nvm
//! use` can change the shell it was typed in.
//!
//! The program has two jobs so far:
//!
//! 1. `nvmise init` prints the shell code a new shell loads.
//! 2. The shell functions start it with [`HANDOFF`] as the first argument
//!    to run one nvm command.
//!
//! The nvmimic library does both. This file only says who is asking
//! ([`NVMISE`]) and where Node versions live ([`MiseBackend`]).

use nvmimic::host::{HANDOFF, Host};
use nvmimic::init::Named;
use nvmise::{MiseBackend, NVMISE};
use std::env;
use std::process::ExitCode;

const USAGE: &str = "usage: nvmise init [bash|zsh|sh|dash] | nvmise --version";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();

    match words.as_slice() {
        [first, function, rest @ ..] if *first == HANDOFF => {
            let Some(backend) = MiseBackend::from_process() else {
                return fail(NO_FOLDERS);
            };
            let rest: Vec<String> = rest.iter().map(|arg| arg.to_string()).collect();
            ExitCode::from(host(&backend).handoff(function, &rest))
        }
        ["--version"] => {
            println!("{}", NVMISE.version);
            ExitCode::SUCCESS
        }
        ["--help"] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["init"] => init(None),
        ["init", shell] => match Named::parse(shell) {
            Some(named) => init(Some(named)),
            None => fail(&format!("init doesn't know the shell {shell}.")),
        },
        _ => fail(USAGE),
    }
}

const NO_FOLDERS: &str =
    "could not work out where mise keeps Node. Set HOME, or MISE_DATA_DIR and NVMISE_DATA_DIR.";

fn host(backend: &MiseBackend) -> Host<'_> {
    Host {
        identity: &NVMISE,
        backend,
        init_extra: "",
    }
}

fn init(named: Option<Named>) -> ExitCode {
    let Some(backend) = MiseBackend::from_process() else {
        return fail(NO_FOLDERS);
    };
    // What `init` prints ends by switching the new shell to the default
    // version. Nobody asked for that switch, so mise isn't told about it.
    let backend = backend.while_loading();
    match host(&backend).init(named) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => fail(&message),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("nvmise: {message}");
    ExitCode::FAILURE
}
