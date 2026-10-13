//! The name nvmise goes by. The library works its folders, settings and
//! shell function names out from it: `~/.local/share/nvmise`,
//! `NVMISE_DATA_DIR`, `__nvmise_handoff`.

use nvmimic::complete::{Command, Needs, command, flag};
use nvmimic::identity::{Identity, Packages};

/// What TAB suggests after `nvmise`.
pub static COMPLETION: Command = Command {
    name: "nvmise",
    about: "nvm's commands, with mise keeping the Node versions",
    needs: Needs::Nothing,
    flags: &[flag("--version", "print nvmise's version")],
    takes: &[],
    commands: &[command(
        "init",
        "print the shell code that loads nvmise",
        Needs::Nothing,
        &[],
        &[],
    )],
};

pub static NVMISE: Identity = Identity {
    name: "nvmise",
    version: env!("CARGO_PKG_VERSION"),
    completion: &COMPLETION,
    packages: Packages {
        homebrew: "nvmise",
        cargo: "nvmise",
    },
    // nvmise has no releases yet.
    release: None,
};
