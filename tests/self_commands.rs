//! `nvmise self install`, `disable`, `enable` and `uninstall`, with the
//! real `nvmise` program in throwaway homes. Official nvm is three small
//! stand-in files, and mise is the stand-in script (see `common/mod.rs`).
//! The same commands with nvmimic in the folder too are in `sister.rs`.

mod common;

use common::{Home, fake_node, homebrew_nvm_is_here};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const NVMISE: &str = env!("CARGO_BIN_EXE_nvmise");

/// Runs `nvmise <args>` with no terminal. nvmise is told that the file
/// being run is where it is installed, so no copy of the program is made.
fn nvmise(home: &Home, args: &[&str]) -> Output {
    Command::new(NVMISE)
        .args(args)
        .env_clear()
        .envs(home.vars())
        .env("PATH", format!("{}:/usr/bin:/bin", home.bin().display()))
        .env("HOMEBREW_PREFIX", home.root.join("no-homebrew"))
        .env("NVMISE_INSTALL_PATH", NVMISE)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Runs a `self` command with `--force` and expects it to work.
fn ok(home: &Home, command: &str) -> String {
    let out = nvmise(home, &["self", command, "--force"]);
    assert!(
        out.status.success(),
        "self {command}: {}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    text(&out.stdout)
}

#[test]
fn install_takes_the_nvm_folder_over_and_uninstall_gives_it_back() {
    if homebrew_nvm_is_here() {
        return;
    }
    let home = Home::new("self");
    home.with_official_nvm();

    // Without --force it would ask, and there is nobody to ask.
    let out = nvmise(&home, &["self", "install"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stderr).lines().count(), 1);
    assert_eq!(home.answers(), "official");

    // A dry run says what would happen and changes nothing.
    let out = nvmise(&home, &["self", "install", "--dry-run"]);
    let plan = text(&out.stdout);
    assert!(plan.contains("Would install nvmise for"), "{plan}");
    assert!(
        plan.contains("Would make the folder for nvm's aliases at"),
        "{plan}"
    );
    assert_eq!(home.answers(), "official");
    assert!(!home.installed("nvmise") && !home.shared().exists());

    let said = ok(&home, "install");
    // What the installer says about the folder is true for nvmise: the
    // versions and aliases already there are not used.
    for line in [
        "nvmise only swaps out nvm's scripts. The Node versions and aliases already in",
        "this folder stay where they are and go unused: nvmise's Node versions are",
        "Node versions are the ones mise keeps in",
    ] {
        assert!(said.contains(line), "no {line:?} in {said}");
    }
    assert!(!said.contains("drop-in replacement"), "{said}");
    assert_eq!(home.answers(), "nvmise");
    assert!(home.installed("nvmise") && home.backup_is_whole());
    assert!(home.aliases().is_dir());
    // Only the three scripts in the nvm folder changed hands.
    let mut names: Vec<String> = fs::read_dir(home.nvm_dir())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["bash_completion", "nvm-exec", "nvm.sh"]);

    // The aliases made with nvmise are named before they are deleted.
    fs::create_dir_all(home.aliases().join("lts")).unwrap();
    fs::write(home.aliases().join("default"), "22\n").unwrap();
    fs::write(home.aliases().join("lts/iron"), "v20.11.1\n").unwrap();
    let plan = text(&nvmise(&home, &["self", "uninstall", "--dry-run"]).stdout);
    assert!(
        plan.contains("Would delete the aliases made with nvmise in")
            && plan.contains(": default, lts/iron."),
        "{plan}"
    );
    // The aliases go, so nothing says they stay.
    assert!(
        plan.contains("The Node versions mise keeps, and the nvm folder, stay as they are."),
        "{plan}"
    );
    assert!(!plan.contains("aliases stay"), "{plan}");
    assert!(home.aliases().join("default").is_file());

    // Without --force and without a terminal, nothing goes.
    let out = nvmise(&home, &["self", "uninstall"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(home.answers(), "nvmise");

    ok(&home, "uninstall");
    assert!(home.official_is_back());
    assert!(!home.aliases().exists());
    assert!(!home.state("nvmise").exists());
    assert!(!home.shared().exists());
    assert!(!home.root.join(".local/share/nvmise").exists());
}

#[test]
fn disable_goes_back_to_official_nvm_and_enable_takes_nvm_again() {
    if homebrew_nvm_is_here() {
        return;
    }
    let home = Home::new("self");
    home.with_official_nvm();

    // There is nothing to switch on before an install.
    let out = nvmise(&home, &["self", "enable", "--force"]);
    assert!(!out.status.success());
    assert_eq!(
        text(&out.stderr),
        "nvmise: nvmise is not installed yet. Run `nvmise self install` first.\n"
    );

    ok(&home, "install");
    fs::write(home.aliases().join("default"), "22\n").unwrap();

    let said = ok(&home, "disable");
    assert!(said.contains("Done. nvm is back in"), "{said}");
    assert!(
        said.contains("so `nvmise self enable` can take over again."),
        "{said}"
    );
    assert!(home.official_is_back());
    // Switched off, nvmise is still installed and keeps its aliases.
    assert!(home.installed("nvmise"));
    assert!(home.aliases().join("default").is_file());

    ok(&home, "enable");
    assert_eq!(home.answers(), "nvmise");
    assert!(home.backup_is_whole());

    // It already answers, so there is nothing to take back.
    let out = nvmise(&home, &["self", "enable", "--force"]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("nvmise already answers `nvm` in"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn nvmise_can_be_first_where_there_was_no_nvm() {
    if homebrew_nvm_is_here() {
        return;
    }
    let home = Home::new("self");
    ok(&home, "install");
    assert_eq!(home.answers(), "nvmise");
    ok(&home, "uninstall");
    // There was no official nvm to put back, and nvmise carries none.
    assert_eq!(home.answers(), "nobody");
    assert!(!home.state("nvmise").exists());
}

/// The shells on this machine, with the flags that keep each from reading
/// anyone's startup files, and how mise gets switched on.
fn shells() -> Vec<(&'static str, &'static [&'static str])> {
    let all: [(&str, &[&str]); 4] = [
        ("/bin/bash", &[]),
        ("/bin/zsh", &["-f"]),
        ("/bin/sh", &[]),
        ("/bin/dash", &[]),
    ];
    all.into_iter()
        .filter(|(program, _)| Path::new(program).is_file())
        .collect()
}

#[test]
fn an_installed_nvmise_loads_through_nvm_sh_in_every_shell() {
    if homebrew_nvm_is_here() {
        return;
    }
    let home = Home::new("self");
    home.with_official_nvm();
    ok(&home, "install");
    for version in ["v20.1.0", "v22.1.0"] {
        fake_node(&home.folder(version), version);
    }
    fs::write(home.aliases().join("default"), "20\n").unwrap();

    // This is the line tools and startup files use to load nvm.
    let script = r#"
before="$PATH"
. "$NVM_DIR/nvm.sh" || exit 90
[ "$before" = "$PATH" ] && echo "path:same" || echo "path:changed"
echo "loaded:$(nvm current) mise:${MISE_NODE_VERSION-}"
nvm use 22 >/dev/null || exit 91
echo "used:$(nvm current) mise:${MISE_NODE_VERSION-}"
case "$(command -v __nvmise_handoff)" in __nvmise_handoff) echo own:yes ;; esac
"#;
    for (shell, flags) in shells() {
        for mise_on in [false, true] {
            let mut command = Command::new(shell);
            command
                .args(flags)
                .args(["-c", script])
                .env_clear()
                .envs(home.vars())
                .env("PATH", format!("{}:/usr/bin:/bin", home.bin().display()))
                .env("SHELL", shell)
                .env("TERM", "dumb")
                .current_dir(&home.root)
                .stdin(Stdio::null());
            if mise_on {
                command.env("__MISE_DIFF", "x");
            }
            let out = command.output().unwrap();
            let (path, current, heard) = common::loaded(mise_on, "20.1.0");
            let told = if mise_on { "22.1.0" } else { "" };
            let expected = format!(
                "{path}\nloaded:{current} mise:{heard}\nused:v22.1.0 mise:{told}\nown:yes\n"
            );
            assert_eq!(
                text(&out.stdout),
                expected,
                "{shell}, mise on: {mise_on}: {}",
                text(&out.stderr)
            );
            assert_eq!(text(&out.stderr), "", "{shell}");
        }
    }
}

#[test]
fn every_help_screen_is_a_usage_line_a_few_sentences_and_the_flags() {
    let home = Home::new("help");
    for action in [
        "install",
        "disable",
        "enable",
        "uninstall",
        "update",
        "rollback",
    ] {
        for help in ["--help", "-h", "help"] {
            let out = nvmise(&home, &["self", action, help]);
            let said = text(&out.stdout);
            assert!(out.status.success(), "{action}");
            let lines: Vec<&str> = said.lines().collect();
            assert_eq!(
                lines[0],
                format!("usage: nvmise self {action} [--force] [--dry-run]")
            );
            assert_eq!(
                &lines[lines.len() - 2..],
                [
                    "  --force     don't ask, go ahead",
                    "  --dry-run   show what would happen and change nothing"
                ],
                "{action}"
            );
            // A blank line, the sentences, a blank line.
            assert!(lines.len() <= 10, "{action}: {said}");
            assert!(
                lines.iter().all(|line| line.len() <= 78),
                "{action}: {said}"
            );
        }
    }
    for args in [
        &[][..],
        &["--help"],
        &["self"],
        &["self", "--help"],
        &["init", "--help"],
    ] {
        let out = nvmise(&home, args);
        assert!(out.status.success(), "{args:?}");
        assert!(text(&out.stdout).starts_with("usage: nvmise"), "{args:?}");
        assert_eq!(text(&out.stderr), "");
    }
    // Asking for help changes nothing.
    assert!(!home.installed("nvmise"));
}

#[test]
fn a_mistake_prints_one_line() {
    let home = Home::new("help");
    for (args, line) in [
        (
            &["bogus"][..],
            "nvmise: there is no command bogus. Run nvmise --help.\n",
        ),
        (
            &["self", "eject"],
            "nvmise: there is no command self eject. Run nvmise self --help.\n",
        ),
        (
            &["self", "uninstall", "--purge"],
            "nvmise: self uninstall has no option --purge. Run nvmise self uninstall --help.\n",
        ),
        (
            &["init", "fish"],
            "nvmise: init doesn't know the shell fish. It knows bash, zsh, sh, dash. Run nvmise init --help.\n",
        ),
        (
            &["self", "install"],
            "nvmise: this asks questions, and there is no terminal to answer them. Run it again with --force.\n",
        ),
    ] {
        let out = nvmise(&home, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(text(&out.stderr), line);
        assert_eq!(text(&out.stdout), "");
    }
}
