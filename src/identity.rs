//! The name nvmise goes by. The library works its folders, settings and
//! shell function names out from it: `~/.local/share/nvmise`,
//! `NVMISE_DATA_DIR`, `__nvmise_handoff`.

use nvmimic::complete::{Command, Flag, Needs, command, flag};
use nvmimic::identity::{Identity, Packages, Release, Words};

/// The flags every `nvmise self` command takes.
const SELF_FLAGS: &[Flag] = &[
    flag("--force", "don't ask, go ahead"),
    flag("--dry-run", "show what would happen and change nothing"),
    flag("--help", "what this command does"),
];

/// What TAB suggests after `nvmise`. Keep in step with the commands in
/// `main.rs`.
pub static COMPLETION: Command = Command {
    name: "nvmise",
    about: "nvm's commands, with mise keeping the Node versions",
    needs: Needs::Nothing,
    flags: &[
        flag("--help", "show nvmise's help"),
        flag("--version", "print nvmise's version"),
    ],
    takes: &[],
    commands: &[
        Command {
            name: "self",
            about: "set nvmise up, switch it off and on, or remove it",
            needs: Needs::Nothing,
            flags: &[flag("--help", "what each of these does")],
            takes: &[],
            commands: &[
                command(
                    "install",
                    "set nvmise up for your nvm folder",
                    Needs::Nothing,
                    SELF_FLAGS,
                    &[],
                ),
                command(
                    "disable",
                    "hand `nvm` to nvmimic or official nvm, keep nvmise",
                    Needs::Nothing,
                    SELF_FLAGS,
                    &[],
                ),
                command("enable", "take `nvm` back", Needs::Nothing, SELF_FLAGS, &[]),
                command(
                    "uninstall",
                    "remove nvmise",
                    Needs::Nothing,
                    SELF_FLAGS,
                    &[],
                ),
                command(
                    "update",
                    "get the newest nvmise",
                    Needs::Nothing,
                    SELF_FLAGS,
                    &[],
                ),
                command(
                    "rollback",
                    "go back to the nvmise from before the last update",
                    Needs::Nothing,
                    SELF_FLAGS,
                    &[],
                ),
            ],
        },
        Command {
            name: "init",
            about: "print the shell code that loads nvmise",
            needs: Needs::Nothing,
            flags: &[flag("--help", "what init is for")],
            takes: &[],
            // Keep in step with `nvmimic::init::Named`.
            commands: &[
                command("bash", "for bash", Needs::Nothing, &[], &[]),
                command("zsh", "for zsh", Needs::Nothing, &[], &[]),
                command("sh", "for sh", Needs::Nothing, &[], &[]),
                command("dash", "for dash", Needs::Nothing, &[], &[]),
            ],
        },
        command("help", "show nvmise's help", Needs::Nothing, &[], &[]),
    ],
};

pub static NVMISE: Identity = Identity {
    name: "nvmise",
    version: env!("CARGO_PKG_VERSION"),
    completion: &COMPLETION,
    packages: Packages {
        homebrew: "nvmise",
        cargo: "nvmise",
    },
    release: Some(Release {
        url: "https://github.com/tsilenzio/nvmise/releases",
        // Only a release build knows the team that signs releases, and
        // `self update` accepts only a download signed by that team.
        team_id: option_env!("NVMISE_TEAM_ID"),
    }),
    // The installer's sentences about the nvm folder. nvmise uses none of
    // the Node versions or aliases kept there.
    words: Words {
        takes_over: &[
            "{app} only swaps out nvm's scripts. The Node versions and aliases already in",
            "this folder stay where they are and go unused: {app}'s Node versions are",
            "the ones mise keeps, and its aliases are its own.",
        ],
        stays: Some("The Node versions mise keeps, and the nvm folder, stay as they are."),
        takes_over_again: "{app} self enable",
        goes_back: Some("{app} self rollback"),
    },
};
