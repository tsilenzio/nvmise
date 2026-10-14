//! nvmise against official nvm, through nvmimic's parity harness. The
//! harness runs the same small shell scripts ("cases") with official nvm
//! and with nvmise, each in its own throwaway home, and compares what they
//! print, return, set and write. Both install from the harness's stand-in
//! for nodejs.org, where every Node release is a tiny tree of scripts.
//!
//! nvmise installs through the mise program of this machine. Every folder
//! mise uses is pointed into the throwaway home's sandbox, so nothing of
//! the person running this is read or changed.
//!
//! It is left out of a normal `cargo test`: it needs mise, and the first
//! run records official nvm for every case, which takes a while. Run it
//! alone, one at a time:
//!
//! ```sh
//! cargo test --test parity -- --ignored --nocapture
//! ```
//!
//! `NVMIMIC_CASES=VER,LST-01` picks cases by the start of their names, and
//! `NVMISE_PARITY_ALL_SHELLS=1` runs every case under all four shells.
//! Differences that are there because Node lives in mise are listed in
//! `tests/parity/expected`. Everything else that differs is written to
//! `target/parity/report.txt`, and the test fails.

mod common;

use nvmimic_parity::cases::{self, Plan};
use nvmimic_parity::harness::Impl;
use nvmimic_parity::subject::{Expected, Subject};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

struct Nvmise;

/// The mise program of this machine: `NVMISE_REAL_MISE`, or the first
/// `mise` on `PATH`.
fn real_mise() -> &'static Path {
    static MISE: OnceLock<PathBuf> = OnceLock::new();
    MISE.get_or_init(|| {
        if let Some(named) = std::env::var_os("NVMISE_REAL_MISE") {
            return PathBuf::from(named);
        }
        std::env::var("PATH")
            .unwrap_or_default()
            .split(':')
            .map(|dir| Path::new(dir).join("mise"))
            .find(|program| program.is_file())
            .expect("this test needs the mise program on PATH")
    })
}

/// Where nvmise and mise keep their own files in a sandbox. All of it is
/// beside the home folder except mise's data, which holds the Node
/// versions: that is in the home folder, as it is for someone using mise.
fn own_folders(side: &Path) -> Vec<(String, String)> {
    let own = side.join("own");
    [
        ("NVMISE_INSTALL_PATH", own.join("bin/nvmise")),
        ("NVMISE_STATE_DIR", own.join("state")),
        ("NVMISE_CACHE_DIR", own.join("cache")),
        ("NVMISE_DATA_DIR", own.join("data")),
        ("NVMISE_MISE", real_mise().to_path_buf()),
        ("MISE_DATA_DIR", side.join("home").join(MISE_DATA)),
        ("MISE_CONFIG_DIR", own.join("mise/config")),
        ("MISE_CACHE_DIR", own.join("mise/cache")),
        ("MISE_STATE_DIR", own.join("mise/state")),
        ("MISE_SYSTEM_DIR", own.join("mise/system")),
    ]
    .into_iter()
    .map(|(name, path)| (name.to_string(), path.display().to_string()))
    .collect()
}

/// mise's data folder, below the sandbox's home.
const MISE_DATA: &str = ".mise-data";

impl Subject for Nvmise {
    fn name(&self) -> &str {
        "nvmise"
    }

    /// Runs nvmise's real installer. The program is a link to the one
    /// cargo built, and `NVMISE_INSTALL_PATH` names that link, so the
    /// installer keeps it where it is and copies nothing.
    fn install(&self, nvm_dir: &Path, side: &Path, _count_launches: bool) {
        let program = side.join("own/bin/nvmise");
        fs::create_dir_all(program.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_nvmise"), &program).unwrap();
        let installed = Command::new(&program)
            .args(["self", "install", "--force"])
            .env_clear()
            .env("HOME", side.join("home"))
            .env("NVM_DIR", nvm_dir)
            .env("PATH", "/usr/bin:/bin")
            .env("HOMEBREW_PREFIX", side.join("no-homebrew"))
            .envs(own_folders(side))
            // The record shared with nvmimic, which only the `self`
            // commands read and no setting of nvmise's own moves.
            .env("NVMIMIC_SHARED_DIR", side.join("own/shared"))
            .output()
            .unwrap();
        assert!(
            installed.status.success(),
            "nvmise self install: {}",
            String::from_utf8_lossy(&installed.stderr)
        );
    }

    /// nvmise finds its record of the install, and mise, through these.
    fn env(&self, side: &Path) -> Vec<(String, String)> {
        own_folders(side)
    }

    fn own_functions(&self) -> Vec<String> {
        ["__nvmise_handoff", "_nvmise_complete"]
            .map(str::to_string)
            .to_vec()
    }

    /// Three things in what a run printed are mise's doing and not a
    /// difference in nvm's behaviour, so they are taken out before the
    /// comparison:
    ///
    /// - mise names a version's folder without the `v`.
    /// - mise prints its own progress and notes while it installs, each
    ///   line starting with `mise `.
    /// - mise puts a small script of its own in place of `bin/npm`, which
    ///   starts npm through `node`. The harness's stand-in `node` is a
    ///   script that can't run npm and prints what it was asked instead.
    ///   Those lines are turned back into what the stand-in npm prints.
    fn normalize(&self, text: String) -> String {
        let store = format!("<home>/{MISE_DATA}/installs/node/");
        let mut out = String::with_capacity(text.len());
        for line in text.split_inclusive('\n') {
            if line.starts_with("mise ") || line.starts_with("  node@") {
                continue;
            }
            out.push_str(&npm_through_node(line).replace(&store, "<install>/v"));
        }
        out
    }

    /// What mise keeps in its own folder is mise's business.
    fn ignores_file(&self, relative: &str) -> bool {
        Path::new(relative).starts_with(MISE_DATA)
    }

    fn expected(&self) -> &Expected {
        static EXPECTED: OnceLock<Expected> = OnceLock::new();
        EXPECTED.get_or_init(|| Expected::parse(include_str!("parity/expected")))
    }
}

/// The version the harness's stand-in npm answers with.
const STAND_IN_NPM: &str = "10.9.0";

/// Rewrites `node v22.1.0: <path>/npm-cli.js <args>`, which is what the
/// stand-in `node` prints when it is handed npm to run, into what the
/// stand-in npm prints for the same arguments.
fn npm_through_node(line: &str) -> String {
    const NPM: &str = "/lib/node_modules/npm/bin/npm-cli.js";
    let Some(at) = line.find(NPM) else {
        return line.to_string();
    };
    let Some(start) = line[..at].rfind("node v") else {
        return line.to_string();
    };
    let rest = &line[at + NPM.len()..];
    let args = rest.strip_prefix(' ').unwrap_or(rest);
    let answer = match args.strip_prefix("--version") {
        Some(after) => format!("{STAND_IN_NPM}{after}"),
        None => match args.strip_prefix("-v") {
            Some(after) if after.trim().is_empty() => format!("{STAND_IN_NPM}{after}"),
            _ => format!("npm: {args}"),
        },
    };
    format!("{}{answer}", &line[..start])
}

#[test]
#[ignore = "needs the mise program, and records official nvm on its first run"]
fn nvmise_is_compared_with_official_nvm() {
    if common::homebrew_nvm_is_here() {
        eprintln!("skipped: Homebrew's nvm is installed, and this runs `self install --force`");
        return;
    }
    // The harness keeps its mirror, its recordings of official nvm and its
    // reports here, and not in its own folder.
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("../parity");
    fs::create_dir_all(&target).unwrap();
    let target = target.canonicalize().unwrap();
    // SAFETY: set before the harness starts any thread.
    unsafe { std::env::set_var("NVMIMIC_PARITY_TARGET", &target) };

    let all = cases::load_all();
    assert!(!all.is_empty(), "no cases");
    // The everyday run uses one shell for a case where official nvm did
    // the same under all four. `NVMISE_PARITY_ALL_SHELLS` runs every case
    // under every shell.
    let one_shell = std::env::var_os("NVMISE_PARITY_ALL_SHELLS").is_none();
    let plan = Plan::everyday_for(Impl::Subject(&Nvmise), one_shell);
    let results = cases::run_all(&all, plan).unwrap_or_else(|reason| {
        panic!("the run was stopped to protect the machine: {reason}");
    });

    cases::write_report(&results, &target.join("report.txt"));
    let differing: Vec<&cases::Result> = results
        .iter()
        .filter(|result| !result.differences.is_empty())
        .collect();
    // How many pairs differ in each group of cases, by the letters a
    // case's name starts with.
    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for result in &results {
        let group = result.id.split('-').next().unwrap_or("").to_string();
        let counts = groups.entry(group).or_default();
        counts.0 += 1;
        counts.1 += usize::from(!result.differences.is_empty());
    }
    println!(
        "{} of {} pairs match. {} differ:",
        results.len() - differing.len(),
        results.len(),
        differing.len()
    );
    for (group, (pairs, differ)) in &groups {
        println!("  {group}: {} of {pairs} match", pairs - differ);
    }
    for result in &differing {
        println!("  differs: {} ({})", result.id, result.shell);
    }
    println!("report: {}", target.join("report.txt").display());
    assert!(differing.is_empty(), "{} pairs differ", differing.len());
}
