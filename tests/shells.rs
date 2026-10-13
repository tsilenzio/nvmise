//! The `nvmise` program under real shells. Each shell loads nvm from what
//! `nvmise init` prints and runs nvm's commands, in a throwaway home with a
//! stand-in for the mise program (see `common/mod.rs`).

mod common;

use common::Home;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const NVMISE: &str = env!("CARGO_BIN_EXE_nvmise");

/// The shells on this machine, each with the name `nvmise init` takes for
/// it and the flags that keep it from reading anyone's startup files.
fn shells() -> Vec<(&'static str, &'static str, &'static [&'static str])> {
    let all: [(&str, &str, &[&str]); 4] = [
        ("/bin/bash", "bash", &[]),
        ("/bin/zsh", "zsh", &["-f"]),
        ("/bin/sh", "sh", &[]),
        ("/bin/dash", "dash", &[]),
    ];
    all.into_iter()
        .filter(|(program, _, _)| Path::new(program).is_file())
        .collect()
}

fn run_shell(home: &Home, shell: &str, flags: &[&str], script: &str, mise_on: bool) -> Output {
    let mut command = Command::new(shell);
    command
        .args(flags)
        .args(["-c", script])
        .env_clear()
        .envs(home.vars())
        .env("NVMISE", NVMISE)
        .env("PATH", format!("{}:/usr/bin:/bin", home.bin().display()))
        // bash sets these two when nobody else did. A shell someone
        // opened has them already, and `init` goes by what it was given.
        .env("SHELL", shell)
        .env("TERM", "dumb")
        // The shell sets `PWD` to the folder it starts in, and `init` goes
        // by the `PWD` it was given. They have to agree, as they do in a
        // shell someone opened.
        .current_dir(&home.root)
        .stdin(Stdio::null());
    if mise_on {
        // What `mise activate` leaves in a shell.
        command.env("__MISE_DIFF", "x");
    }
    command.output().unwrap()
}

const SCRIPT: &str = r#"
eval "$("$NVMISE" init SHELL)" || exit 90
echo "loaded:$(nvm current) mise:${MISE_NODE_VERSION-}"
nvm use 22.1.0 || exit 91
echo "node:$(command -v node) mise:${MISE_NODE_VERSION-}"
nvm alias default 22 >/dev/null && echo "alias:$(cat "$HOME/.local/share/nvmise/alias/default")"
nvm uninstall 20.1.0 >/dev/null || exit 92
echo "left:$(ls "$MISE_DATA_DIR/installs/node")"
nvm deactivate >/dev/null 2>&1
echo "after:${NVM_BIN-} mise:${MISE_NODE_VERSION-}"
case "$(command -v __nvmise_handoff)" in __nvmise_handoff) echo own:yes ;; esac
command -v __nvmimic_handoff >/dev/null 2>&1 && echo "nvmimic's function is here"
nvm unload
command -v nvm >/dev/null 2>&1 && echo "nvm still loaded"
echo done
"#;

#[test]
fn every_shell_runs_nvm_through_nvmise() {
    for (shell, name, flags) in shells() {
        for mise_on in [false, true] {
            let home = Home::new("shell");
            home.with(&["v20.1.0", "v22.1.0"]);
            fs::create_dir_all(home.aliases()).unwrap();
            fs::write(home.aliases().join("default"), "20\n").unwrap();

            let out = run_shell(&home, shell, flags, &SCRIPT.replace("SHELL", name), mise_on);
            let said = String::from_utf8_lossy(&out.stdout);
            let errors = String::from_utf8_lossy(&out.stderr);
            // mise hears about `nvm use` only where it is switched on, and
            // never about the switch a loading shell makes by itself.
            let told = if mise_on { "22.1.0" } else { "" };
            assert_eq!(
                said,
                format!(
                    "loaded:v20.1.0 mise:\nNow using node v22.1.0 (npm v10.7.0)\n\
                     node:{}/bin/node mise:{told}\nalias:22\nleft:22.1.0\nafter: mise:\n\
                     own:yes\ndone\n",
                    home.path_of("v22.1.0")
                ),
                "{shell}, mise on: {mise_on}: {errors}"
            );
            assert_eq!(errors, "", "{shell}");
            assert!(
                home.mise_log().starts_with("uninstall node@20.1.0 "),
                "{shell}: {}",
                home.mise_log()
            );
            // Nothing went into nvm's folder, and nothing appeared under
            // nvmimic's name.
            assert_eq!(fs::read_dir(home.nvm_dir()).unwrap().count(), 0, "{shell}");
            assert!(!home.root.join(".local/state/nvmimic").exists());
            assert!(!home.root.join(".local/share/nvmimic").exists());
        }
    }
}

#[test]
fn the_program_says_who_it_is() {
    let run = |args: &[&str]| {
        let home = Home::new("program");
        Command::new(NVMISE)
            .args(args)
            .env_clear()
            .envs(home.vars())
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let out = run(&["--version"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("{}\n", env!("CARGO_PKG_VERSION"))
    );

    let out = run(&["init", "fish"]);
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "nvmise: init doesn't know the shell fish.\n"
    );

    // The shell code carries nvmise's names and none of nvmimic's.
    let out = run(&["init", "bash"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("__nvmise_handoff() {"), "{text}");
    assert!(!text.contains("__nvmimic_handoff"), "{text}");
    assert!(!text.contains("__NVMIMIC_"), "{text}");
}
