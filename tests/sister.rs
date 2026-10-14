//! nvmise and nvmimic in one nvm folder. Both can be installed, and only
//! one answers `nvm` at a time. Every way of handing `nvm` from one to the
//! other is run here with the nvmimic program installed on this machine and
//! the `nvmise` program built from this code, in throwaway homes.
//!
//! The tests skip themselves where there is no nvmimic 0.1.1 or newer.
//! `NVMISE_TEST_NVMIMIC` names the nvmimic program if it isn't in
//! Homebrew's usual place.

mod common;

use common::{Home, SCRIPTS, homebrew_nvm_is_here};
use nvmimic::handover::Record;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const NVMISE: &str = "nvmise";
const NVMIMIC: &str = "nvmimic";
/// The two orders the programs can arrive in.
const ORDERS: [(&str, &str); 2] = [(NVMIMIC, NVMISE), (NVMISE, NVMIMIC)];

/// The nvmimic program to test against, if this machine has one that knows
/// the record the two share.
fn nvmimic_program() -> Option<PathBuf> {
    if homebrew_nvm_is_here() {
        eprintln!(
            "skipped: Homebrew's nvm is installed, and these tests run `self install --force`"
        );
        return None;
    }
    let program = std::env::var_os("NVMISE_TEST_NVMIMIC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/homebrew/bin/nvmimic"));
    let version = Command::new(&program).arg("--version").output().ok()?;
    let version = text(&version.stdout);
    let numbers: Vec<u32> = version
        .trim()
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    if numbers.len() != 3 || numbers < vec![0, 1, 1] {
        eprintln!("skipped: {} is nvmimic {version}", program.display());
        return None;
    }
    Some(program)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct Pair {
    home: Home,
    nvmimic: PathBuf,
}

impl Pair {
    /// A throwaway home with official nvm's three scripts in `~/.nvm`.
    fn new(nvmimic: &Path) -> Pair {
        let home = Home::new("sister");
        home.with_official_nvm();
        Pair {
            home,
            nvmimic: nvmimic.to_path_buf(),
        }
    }

    /// Runs `<name> self <args>` with no terminal. nvmise is told that the
    /// file being run is where it is installed, so no copy of it is made.
    /// nvmimic is Homebrew's, and stays where Homebrew put it.
    fn run(&self, name: &str, args: &[&str]) -> Output {
        let program = match name {
            NVMISE => Path::new(env!("CARGO_BIN_EXE_nvmise")),
            _ => &self.nvmimic,
        };
        Command::new(program)
            .arg("self")
            .args(args)
            .env_clear()
            .envs(self.home.vars())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.home.bin().display()),
            )
            .env("HOMEBREW_PREFIX", self.home.root.join("no-homebrew"))
            .env("NVMISE_INSTALL_PATH", env!("CARGO_BIN_EXE_nvmise"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// Runs a `self` command with `--force` and expects it to work.
    fn ok(&self, name: &str, command: &str) -> String {
        let out = self.run(name, &[command, "--force"]);
        assert!(
            out.status.success(),
            "{name} self {command}: {}{}",
            text(&out.stdout),
            text(&out.stderr)
        );
        text(&out.stdout)
    }

    fn record(&self) -> Record {
        let text = fs::read_to_string(self.home.shared().join("record")).unwrap_or_default();
        Record::parse(&text)
    }

    /// The name of the function behind `nvm` in a new shell that loads
    /// `$NVM_DIR/nvm.sh`: `__nvmise_handoff` or `__nvmimic_handoff`.
    fn loads(&self) -> String {
        let script = r#". "$NVM_DIR/nvm.sh" --no-use || exit 90
for name in nvmise nvmimic; do
  command -v "__${name}_handoff" >/dev/null 2>&1 && echo "$name"
done"#;
        let out = Command::new("/bin/bash")
            .args(["-c", script])
            .env_clear()
            .envs(self.home.vars())
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&self.home.root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        text(&out.stdout).trim().to_string()
    }
}

#[test]
fn each_takes_over_from_the_other_and_both_stay_installed() {
    let Some(nvmimic) = nvmimic_program() else {
        return;
    };
    for (first, second) in ORDERS {
        let pair = Pair::new(&nvmimic);
        let home = &pair.home;
        pair.ok(first, "install");
        assert_eq!(home.answers(), first);
        assert_eq!(pair.loads(), first);
        assert!(home.backup_is_whole());

        // Without --force it asks, and with no terminal nothing changes.
        let asked = pair.run(second, &["install"]);
        assert!(!asked.status.success(), "{second}");
        assert_eq!(home.answers(), first);
        assert!(!home.installed(second));

        // A dry run says what it would do.
        let dry = text(&pair.run(second, &["install", "--dry-run"]).stdout);
        assert!(
            dry.contains(&format!(
                "Would put {second}'s scripts in place of {first}'s and leave {first} installed."
            )),
            "{dry}"
        );
        assert_eq!(home.answers(), first);

        // Taking over says what changes, names the sister tool, says who
        // made both, and asks.
        let said = pair.ok(second, "install");
        for line in [
            format!("{first} answers `nvm` in"),
            format!("{second} and {first} are sister tools by the same"),
            format!("This puts {second}'s scripts in place of {first}'s."),
            format!("Let {second} answer `nvm` from now on?"),
        ] {
            assert!(said.contains(&line), "{second}: no {line:?} in {said}");
        }
        assert_eq!(home.answers(), second);
        assert_eq!(pair.loads(), second);
        assert!(home.installed(first) && home.installed(second));
        let record = pair.record();
        assert_eq!(record.answers.as_deref(), Some(second));
        assert_eq!(record.programs.len(), 2);
        // The sister tool's scripts were never taken for official nvm's.
        assert!(home.backup_is_whole());

        // And back again: whoever was told last is in control.
        pair.ok(first, "install");
        assert_eq!(home.answers(), first);
        assert_eq!(pair.record().answers.as_deref(), Some(first));
        assert!(home.installed(second));
        assert!(home.backup_is_whole());
    }
}

#[test]
fn uninstalling_the_one_in_control_hands_over_to_the_one_asleep() {
    let Some(nvmimic) = nvmimic_program() else {
        return;
    };
    for (first, second) in ORDERS {
        let pair = Pair::new(&nvmimic);
        let home = &pair.home;
        pair.ok(first, "install");
        pair.ok(second, "install");

        let plan = text(&pair.run(second, &["uninstall", "--dry-run"]).stdout);
        assert!(
            plan.contains(&format!("Would hand `nvm` over to {first} by running")),
            "{plan}"
        );
        assert!(
            plan.contains("are sister tools by the same author"),
            "{plan}"
        );
        assert_eq!(home.answers(), second);

        let said = pair.ok(second, "uninstall");
        assert!(
            said.contains(&format!("and {first} answers `nvm` now.")),
            "{said}"
        );
        assert_eq!(home.answers(), first);
        assert_eq!(pair.loads(), first);
        assert!(!home.state(second).exists());
        let record = pair.record();
        assert_eq!(record.answers.as_deref(), Some(first));
        assert_eq!(record.programs.keys().collect::<Vec<_>>(), [first]);
        assert!(home.backup_is_whole(), "the backup outlives {second}");

        // The last one out puts official nvm back.
        pair.ok(first, "uninstall");
        assert!(home.official_is_back());
        assert!(!home.shared().exists());
        assert!(!home.state(first).exists());
    }
}

#[test]
fn uninstalling_the_one_asleep_leaves_the_other_in_control() {
    let Some(nvmimic) = nvmimic_program() else {
        return;
    };
    for (first, second) in ORDERS {
        let pair = Pair::new(&nvmimic);
        let home = &pair.home;
        pair.ok(first, "install");
        pair.ok(second, "install");
        let scripts_before: Vec<String> = SCRIPTS
            .iter()
            .map(|name| fs::read_to_string(home.nvm_dir().join(name)).unwrap())
            .collect();

        let said = pair.ok(first, "uninstall");
        assert!(
            said.contains(&format!("{second} answers `nvm` in")),
            "{said}"
        );
        assert_eq!(home.answers(), second);
        assert!(!home.installed(first) && home.installed(second));
        let scripts_after: Vec<String> = SCRIPTS
            .iter()
            .map(|name| fs::read_to_string(home.nvm_dir().join(name)).unwrap())
            .collect();
        assert_eq!(scripts_before, scripts_after);
        assert!(home.backup_is_whole());
        assert_eq!(pair.record().programs.keys().collect::<Vec<_>>(), [second]);

        pair.ok(second, "uninstall");
        assert!(home.official_is_back());
        assert!(!home.shared().exists());
    }
}

#[test]
fn disable_hands_nvm_to_nvmimic_and_enable_takes_it_back() {
    let Some(nvmimic) = nvmimic_program() else {
        return;
    };
    for (first, second) in ORDERS {
        let pair = Pair::new(&nvmimic);
        let home = &pair.home;
        pair.ok(first, "install");
        pair.ok(second, "install");
        // Whoever came last, nvmise is the one in control for this.
        if second != NVMISE {
            pair.ok(NVMISE, "enable");
        }
        assert_eq!(home.answers(), NVMISE);

        let plan = text(&pair.run(NVMISE, &["disable", "--dry-run"]).stdout);
        assert!(
            plan.contains("Would hand `nvm` over to nvmimic by running"),
            "{plan}"
        );
        assert_eq!(home.answers(), NVMISE);

        let said = pair.ok(NVMISE, "disable");
        assert!(
            said.contains("nvmimic is set up for")
                && said.contains("nvmise and nvmimic are sister tools by the same author."),
            "{said}"
        );
        assert_eq!(home.answers(), NVMIMIC);
        assert_eq!(pair.loads(), NVMIMIC);
        assert!(home.installed(NVMISE));
        assert!(pair.record().programs[NVMISE].aside);

        // Switched off, nvmise is not handed `nvm` when nvmimic leaves.
        // Enable is what brings it back.
        let said = pair.ok(NVMISE, "enable");
        assert!(
            said.contains("Let nvmise answer `nvm` from now on?"),
            "{said}"
        );
        assert_eq!(home.answers(), NVMISE);
        assert_eq!(pair.loads(), NVMISE);
        assert!(!pair.record().programs[NVMISE].aside);
        assert!(home.installed(NVMIMIC) && home.backup_is_whole());
    }
}

#[test]
fn a_program_that_stepped_aside_is_not_handed_nvm() {
    let Some(nvmimic) = nvmimic_program() else {
        return;
    };
    // nvmimic was told to step aside (`self eject`). nvmise's disable and
    // uninstall then go to official nvm, and nvmimic stays asleep.
    for leaving in ["disable", "uninstall"] {
        let pair = Pair::new(&nvmimic);
        let home = &pair.home;
        pair.ok(NVMIMIC, "install");
        pair.ok(NVMIMIC, "eject");
        assert!(home.official_is_back());
        pair.ok(NVMISE, "install");
        assert_eq!(home.answers(), NVMISE);

        pair.ok(NVMISE, leaving);
        assert!(home.official_is_back(), "{leaving}");
        assert!(home.installed(NVMIMIC), "{leaving}");
        assert_eq!(home.installed(NVMISE), leaving == "disable");
    }

    // nvmise switched off, nvmimic uninstalled while in control: official
    // nvm goes back, and nvmise stays asleep until `self enable`.
    let pair = Pair::new(&nvmimic);
    let home = &pair.home;
    pair.ok(NVMISE, "install");
    pair.ok(NVMIMIC, "install");
    pair.ok(NVMIMIC, "install");
    pair.ok(NVMISE, "enable");
    pair.ok(NVMISE, "disable");
    assert_eq!(home.answers(), NVMIMIC);
    pair.ok(NVMIMIC, "uninstall");
    assert!(home.official_is_back());
    assert!(home.installed(NVMISE));
    pair.ok(NVMISE, "enable");
    assert_eq!(home.answers(), NVMISE);
}

#[test]
fn nvm_runs_through_whichever_one_is_in_control() {
    let Some(nvmimic) = nvmimic_program() else {
        return;
    };
    let pair = Pair::new(&nvmimic);
    let home = &pair.home;
    // One version in mise's folder and another in nvm's own.
    home.with(&["v22.1.0"]);
    common::fake_node(&home.nvm_dir().join("versions/node/v18.1.0"), "v18.1.0");
    let listed = |pair: &Pair| {
        let out = Command::new("/bin/bash")
            .args(["-c", ". \"$NVM_DIR/nvm.sh\" --no-use && nvm_ls"])
            .env_clear()
            .envs(pair.home.vars())
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&pair.home.root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        text(&out.stdout)
    };

    pair.ok(NVMISE, "install");
    assert_eq!(listed(&pair), "v22.1.0\n");
    pair.ok(NVMIMIC, "install");
    assert_eq!(listed(&pair), "v18.1.0\n");
    pair.ok(NVMISE, "enable");
    assert_eq!(listed(&pair), "v22.1.0\n");
    pair.ok(NVMISE, "disable");
    assert_eq!(listed(&pair), "v18.1.0\n");
}
