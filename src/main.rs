//! The `nvmise` program. People run it to set nvmise up, switch it off and
//! on, and remove it. `$NVM_DIR/nvm.sh` runs `nvmise init` to get the shell
//! functions, and the functions start it with [`HANDOFF`] as the first
//! argument to run one nvm command. That is how `nvm use` can change the
//! shell it was typed in.
//!
//! The nvmimic library does the work behind every one of these. This file
//! says who is asking ([`NVMISE`]), where Node versions live
//! ([`MiseBackend`]), and what nvmise adds to an install ([`MiseSteps`]).

use nvmimic::host::{HANDOFF, Host, is_tty};
use nvmimic::init::Named;
use nvmimic::install::{self, Env, Options, TerminalUi};
use nvmimic::update::{self, Source};
use nvmise::{MiseBackend, MiseSteps, NVMISE};
use std::env;
use std::process::ExitCode;

const USAGE: &str = "\
usage: nvmise <command>

nvmise gives you the `nvm` command, with mise keeping the Node versions.
In your shell you keep typing `nvm`. This command sets nvmise up, switches
it off and on, and removes it again.

  self install     set nvmise up for your nvm folder ($NVM_DIR, usually ~/.nvm)
  self disable     hand `nvm` to nvmimic or official nvm, and keep nvmise
  self enable      take `nvm` back
  self uninstall   remove nvmise again
  self update      get the newest nvmise
  self rollback    go back to the nvmise from before the last update
  init [shell]     print the shell code that loads nvmise, for `eval`
  --version        print nvmise's own version

Run nvmise <command> --help for details.";

const INIT_USAGE: &str = "\
usage: nvmise init [bash|zsh|sh|dash]

Prints the shell code that makes `nvm` work, for a shell to run with `eval`.
The nvm.sh that `nvmise self install` writes does that for you. To do it
yourself, put this line in your startup file, with the name of your shell:

  eval \"$(nvmise init zsh)\"";

const SELF_USAGE: &str = "\
usage: nvmise self <command> [--force] [--dry-run]

  install     set nvmise up for your nvm folder ($NVM_DIR, usually ~/.nvm)
  disable     hand `nvm` to nvmimic or official nvm, and keep nvmise
  enable      take `nvm` back
  uninstall   remove nvmise again
  update      get the newest nvmise
  rollback    go back to the nvmise from before the last update

  --force     don't ask, go ahead
  --dry-run   show what would happen and change nothing

Run nvmise self <command> --help for details.";

const FLAGS: &str =
    "  --force     don't ask, go ahead\n  --dry-run   show what would happen and change nothing";

/// What `nvmise self <action> --help` says before the flags.
fn about(action: &str) -> &'static str {
    match action {
        "install" => {
            "\
Sets nvmise up, so that `nvm` in your shell is nvmise and Node versions are
the ones mise keeps. The three scripts in your nvm folder are swapped, and
official nvm's are kept so that `nvmise self uninstall` can put them back.
If nvmimic answers `nvm` now, nvmise takes over from it and nvmimic stays
installed."
        }
        "disable" => {
            "\
Hands `nvm` to nvmimic if nvmimic is set up, and to official nvm if it
isn't. nvmise stays installed and keeps its aliases. `nvmise self enable`
takes `nvm` back."
        }
        "enable" => {
            "\
Takes `nvm` back after `nvmise self disable`, from nvmimic or from official
nvm. Whoever had it stays installed."
        }
        "uninstall" => {
            "\
Removes nvmise and the aliases made with it. `nvm` goes to nvmimic if
nvmimic is set up and wasn't switched off. Otherwise official nvm's scripts
go back. The Node versions mise keeps and your nvm folder stay."
        }
        "update" => {
            "\
Gets the newest nvmise, checks the download and swaps it in. The nvmise you
have now is kept, and `nvmise self rollback` brings it back."
        }
        _ => {
            "\
Goes back to the nvmise you had before the last `nvmise self update`.
Running it twice gets you back where you started."
        }
    }
}

const ACTIONS: [&str; 6] = [
    "install",
    "disable",
    "enable",
    "uninstall",
    "update",
    "rollback",
];

fn is_help(word: &str) -> bool {
    matches!(word, "help" | "--help" | "-h")
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();

    match words.as_slice() {
        [first, function, rest @ ..] if *first == HANDOFF => {
            let Some(backend) = MiseBackend::from_process() else {
                return fail(NO_FOLDERS);
            };
            let rest: Vec<String> = rest.iter().map(|arg| arg.to_string()).collect();
            ExitCode::from(host(&backend).handoff(function, &rest))
        }
        [] => say(USAGE),
        [word] if is_help(word) => say(USAGE),
        ["--version"] => say(NVMISE.version),
        ["init"] => init(None),
        ["init", word] if is_help(word) => say(INIT_USAGE),
        ["init", shell] => match Named::parse(shell) {
            Some(named) => init(Some(named)),
            None => mistake(&format!(
                "init doesn't know the shell {shell}. It knows {}. Run nvmise init --help.",
                Named::NAMES.join(", ")
            )),
        },
        ["self"] => say(SELF_USAGE),
        ["self", word] if is_help(word) => say(SELF_USAGE),
        ["self", action, flags @ ..] if ACTIONS.contains(action) => self_command(action, flags),
        _ => {
            // Only `self` has commands of its own to point at.
            let help = match words.first() {
                Some(&"self") => "nvmise self --help",
                _ => "nvmise --help",
            };
            mistake(&format!(
                "there is no command {}. Run {help}.",
                words.join(" ")
            ))
        }
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
    match host(&backend).init(named) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => fail(&message),
    }
}

/// `nvmise self <action>`. The library does each of them for the name in
/// `Env`, on the record it shares with nvmimic of who answers `nvm`.
fn self_command(action: &str, flags: &[&str]) -> ExitCode {
    if flags.iter().any(|flag| is_help(flag)) {
        return say(&format!(
            "usage: nvmise self {action} [--force] [--dry-run]\n\n{}\n\n{FLAGS}",
            about(action)
        ));
    }
    if let Some(unknown) = flags
        .iter()
        .find(|flag| !matches!(**flag, "--force" | "--dry-run"))
    {
        return mistake(&format!(
            "self {action} has no option {unknown}. Run nvmise self {action} --help."
        ));
    }
    let options = Options {
        force: flags.contains(&"--force"),
        dry_run: flags.contains(&"--dry-run"),
        purge: false,
        terminal: is_tty(0),
    };
    // `disable` and `uninstall` say what they would have done before they
    // refuse, so they handle a missing terminal themselves.
    let leaving = matches!(action, "disable" | "uninstall");
    if !leaving && !options.force && !options.dry_run && !options.terminal {
        return mistake(
            "this asks questions, and there is no terminal to answer them. Run it again with --force.",
        );
    }
    let Some(backend) = MiseBackend::from_process() else {
        return fail(NO_FOLDERS);
    };
    let steps = MiseSteps(&backend);
    let env = Env::from_process_for(&NVMISE);
    let mut ui = TerminalUi;
    let own_file = || {
        env::current_exe().map_err(|error| format!("cannot find this program's own file: {error}"))
    };
    let result = match action {
        "install" => own_file().and_then(|program| {
            install::self_install_with(&env, &program, &options, &mut ui, &steps)
        }),
        "enable" => own_file().and_then(|program| {
            install::self_enable_with(&env, &program, &options, &mut ui, &steps)
        }),
        "disable" => install::self_disable_with(&env, &options, &mut ui, &steps),
        "uninstall" => install::self_uninstall_with(&env, &options, &mut ui, &steps),
        "update" => match Source::from_process_for(&NVMISE) {
            Some(source) => update::self_update(&env, &source, &options, &mut ui),
            None => Err("there are no nvmise releases yet.".to_string()),
        },
        _ => update::self_rollback(&env, &options, &mut ui),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) if message == install::NO_TERMINAL => mistake(&message),
        Err(message) => fail(&message),
    }
}

fn say(text: &str) -> ExitCode {
    println!("{text}");
    ExitCode::SUCCESS
}

/// Something went wrong while doing what was asked.
fn fail(message: &str) -> ExitCode {
    eprintln!("nvmise: {message}");
    ExitCode::FAILURE
}

/// The command itself was wrong: one line, and the exit status shells use
/// for that.
fn mistake(message: &str) -> ExitCode {
    eprintln!("nvmise: {message}");
    ExitCode::from(2)
}
