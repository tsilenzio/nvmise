//! A smoke test with the mise program of this machine in place of the
//! stand-in. It still runs in a throwaway home: every folder mise uses is
//! pointed inside it, so the Node versions and settings of the person
//! running the test are never read or changed.
//!
//! It is left out of a normal `cargo test`, because it needs mise and the
//! network. Run it with:
//!
//! ```sh
//! cargo test --test real_mise -- --ignored --nocapture
//! ```
//!
//! One Node release is downloaded from nodejs.org, once, and kept under
//! `target/`. The test then serves it from this machine as nvm's mirror, so
//! every `nvm install` below is mise really fetching, checking and unpacking
//! a release, with nothing more coming from the internet. The test prints
//! what mise asked the mirror for and what mise said.
//!
//! Each shell goes through three steps, with mise switched off and on:
//! install and use a version, open a new shell and see what loading nvm
//! does there, and uninstall the version.

mod common;

use common::Home;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

const NVMISE: &str = env!("CARGO_BIN_EXE_nvmise");

/// The release to install, and the name of its file for this machine.
struct Release {
    version: &'static str,
    file: String,
}

fn release() -> Release {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let native = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    // Node 14 was never built for Apple's own chips, so there nvmise has to
    // tell mise to fetch the Intel build. That is worth seeing happen, and
    // it only runs where macOS can run Intel programs.
    let intel_runs = cfg!(all(target_os = "macos", target_arch = "aarch64"))
        && Command::new("/usr/bin/arch")
            .args(["-x86_64", "/usr/bin/true"])
            .status()
            .is_ok_and(|status| status.success());
    let (version, arch) = if intel_runs {
        ("14.21.3", "x64")
    } else {
        ("16.20.2", native)
    };
    Release {
        version,
        file: format!("node-v{version}-{os}-{arch}.tar.gz"),
    }
}

/// The mise program of this machine: `NVMISE_REAL_MISE`, or the first
/// `mise` on `PATH`.
fn real_mise() -> PathBuf {
    if let Some(named) = std::env::var_os("NVMISE_REAL_MISE") {
        return PathBuf::from(named);
    }
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|dir| Path::new(dir).join("mise"))
        .find(|program| program.is_file())
        .expect("this test needs the mise program on PATH")
}

/// The folder the mirror serves: the release, its checksums and two lists
/// of what exists. The release is fetched from nodejs.org the first time
/// and kept.
fn mirror_folder(release: &Release) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("real-mise-mirror");
    let version = release.version;
    let folder = root.join(format!("v{version}"));
    fs::create_dir_all(&folder).unwrap();
    for name in [release.file.as_str(), "SHASUMS256.txt"] {
        let kept = folder.join(name);
        if kept.is_file() {
            continue;
        }
        let part = folder.join(format!("{name}.part"));
        let fetched = Command::new("curl")
            .args(["-fsSL", "--retry", "2", "-o"])
            .arg(&part)
            .arg(format!("https://nodejs.org/dist/v{version}/{name}"))
            .status()
            .unwrap();
        assert!(fetched.success(), "fetching {name} from nodejs.org");
        fs::rename(part, kept).unwrap();
    }
    fs::write(
        root.join("index.tab"),
        format!(
            "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
             v{version}\t2023-02-16\tosx-x64-tar,osx-arm64-tar,linux-x64,linux-arm64\t6.14.18\t8.4\t1.40.0\t1.2.11\t1.1.1\t83\t-\t-\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("index.json"),
        format!(
            "[{{\"version\":\"v{version}\",\"date\":\"2023-02-16\",\
             \"files\":[\"osx-x64-tar\",\"osx-arm64-tar\",\"linux-x64\",\"linux-arm64\"],\
             \"npm\":\"6.14.18\",\"lts\":false,\"security\":false}}]\n"
        ),
    )
    .unwrap();
    root
}

/// Serves a folder over HTTP and writes down every request.
fn serve(root: PathBuf) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (root, log) = (root.clone(), Arc::clone(&log));
            std::thread::spawn(move || answer(stream, &root, &log));
        }
    });
    (address, seen)
}

fn answer(mut stream: TcpStream, root: &Path, log: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    let mut agent = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 && line.trim() != "" => {
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("user-agent:") {
                    agent = value.trim().split('/').next().unwrap_or("").to_string();
                }
            }
            _ => break,
        }
    }
    let mut words = first.split_whitespace();
    let (method, path) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    let file = root.join(path.trim_start_matches('/'));
    let found = !path.contains("..") && file.is_file();
    log.lock().unwrap().push(format!(
        "{method} {path} -> {} ({agent})",
        if found { 200 } else { 404 }
    ));
    if !found {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    }
    let bytes = fs::read(file).unwrap();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    if method != "HEAD" {
        let _ = stream.write_all(&bytes);
    }
}

/// How mise comes to be switched on in a shell.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mise {
    /// Not at all.
    Off,
    /// `mise activate`, as a startup file would run it.
    Activated(&'static str),
    /// mise has no `activate` for this shell. It is started from a bash
    /// that has mise switched on, the way a script is.
    Inherited,
}

/// Step one: install a version and use it.
const INSTALL: &str = r#"
@PRELUDE@
eval "$("$NVMISE" init @SHELL@)" || exit 90
nvm install @VERSION@ || exit 91
echo "current:$(nvm current)"
echo "which:$(nvm which @VERSION@)"
echo "node:$(node --version)"
nvm ls --no-colors | sed 's/^/ls:/'
nvm deactivate >/dev/null 2>&1
nvm use @VERSION@ || exit 92
echo "used:$(nvm current) told:${MISE_NODE_VERSION-}"
echo "mise-picks:$(command mise which node 2>/dev/null)"
@HOOK@
echo "after-prompt:$(command -v node)"
nvm deactivate >/dev/null 2>&1
echo "deactivated told:${MISE_NODE_VERSION-}"
echo done
"#;

/// Step two: a new shell loads nvm. `nvm install` made the version the
/// `default` alias, which official nvm switches every new shell to. The
/// lines starting with `load ` are the record of what the shell ends up
/// with, and the test prints them together at the end.
const LOAD: &str = r#"
@PRELUDE@
before="$PATH"
eval "$("$NVMISE" init @SHELL@)" || exit 90
[ "$before" = "$PATH" ] && echo "load PATH: unchanged" || echo "load PATH: changed, first entry ${PATH%%:*}"
echo "load nvm current: $(nvm current)"
echo "load NVM_BIN: ${NVM_BIN-}"
echo "load MISE_NODE_VERSION: ${MISE_NODE_VERSION-}"
echo "load node on PATH: $(command -v node) $(node --version 2>/dev/null)"
@HOOK@
echo "load node after the next prompt: $(command -v node) $(node --version 2>/dev/null)"
cd "$HOME/project" || exit 94
@HOOK@
echo "load node in a folder with its own mise.toml: $(command -v node) $(node --version 2>/dev/null)"
echo done
"#;

/// Step three: uninstall the version.
const UNINSTALL: &str = r#"
@PRELUDE@
eval "$("$NVMISE" init @SHELL@)" || exit 90
nvm deactivate >/dev/null 2>&1
nvm uninstall @VERSION@ || exit 93
echo "left:$(ls "$MISE_DATA_DIR/installs/node" 2>/dev/null | tr '\n' ' ')"
echo done
"#;

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .map(|read| {
            read.flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
#[ignore = "needs the mise program and one download from nodejs.org"]
fn nvm_works_through_the_real_mise_in_every_shell() {
    let release = release();
    let version = release.version;
    let (mirror, seen) = serve(mirror_folder(&release));

    // The Node versions of the person running this, to compare at the end.
    let own_store = std::env::var_os("HOME")
        .map(|home| Path::new(&home).join(".local/share/mise/installs/node"))
        .unwrap_or_default();
    let own_before = names_in(&own_store);

    let home = Home::new("real");
    let mise = home.bin().join("mise");
    fs::remove_file(&mise).unwrap();
    std::os::unix::fs::symlink(real_mise(), &mise).unwrap();
    let store = home.path_of(version);
    // A project folder that asks mise for the version by itself.
    let project = home.root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("mise.toml"),
        format!("[tools]\nnode = \"{version}\"\n"),
    )
    .unwrap();

    let shells: [(&str, &str, &[&str], Mise); 4] = [
        ("/bin/bash", "bash", &[], Mise::Activated("bash")),
        ("/bin/zsh", "zsh", &["-f"], Mise::Activated("zsh")),
        // `/bin/sh` is bash on macOS, and bash's activation runs in it.
        ("/bin/sh", "sh", &[], Mise::Activated("bash")),
        ("/bin/dash", "dash", &[], Mise::Inherited),
    ];
    let mut installs = 0;
    // What a new shell ended up with after loading nvm, per shell.
    let mut loads: Vec<(String, Vec<String>)> = Vec::new();
    for (shell, name, flags, switched_on) in shells {
        if !Path::new(shell).is_file() {
            continue;
        }
        for how in [Mise::Off, switched_on] {
            let label = format!("{shell}, mise {how:?}");
            let (prelude, hook) = match how {
                Mise::Activated(kind) => (
                    format!("eval \"$(mise activate {kind})\""),
                    // What the shell runs before it shows the next prompt.
                    "_mise_hook",
                ),
                _ => (String::new(), ":"),
            };
            // Runs one step in a shell of its own and returns what it said.
            let run = |step: &str, script: &str| -> String {
                let script = script
                    .replace("@PRELUDE@", &prelude)
                    .replace("@HOOK@", hook)
                    .replace("@SHELL@", name)
                    .replace("@VERSION@", version);
                let mut command = match how {
                    Mise::Inherited => {
                        let mut bash = Command::new("/bin/bash");
                        bash.args([
                            "-c",
                            "eval \"$(mise activate bash)\"; exec \"$0\" -c \"$1\"",
                            shell,
                            &script,
                        ]);
                        bash
                    }
                    _ => {
                        let mut plain = Command::new(shell);
                        plain.args(flags).args(["-c", &script]);
                        plain
                    }
                };
                let out = command
                    .env_clear()
                    .envs(home.vars())
                    .env("NVMISE", NVMISE)
                    .env("NVM_NODEJS_ORG_MIRROR", &mirror)
                    // mise reads a project's `mise.toml` only once the
                    // folder is trusted.
                    .env("MISE_TRUSTED_CONFIG_PATHS", &home.root)
                    .env("PATH", format!("{}:/usr/bin:/bin", home.bin().display()))
                    .env("SHELL", shell)
                    .env("TERM", "dumb")
                    .current_dir(&home.root)
                    .stdin(Stdio::null())
                    .output()
                    .unwrap();
                let said = String::from_utf8_lossy(&out.stdout).into_owned();
                let errors = String::from_utf8_lossy(&out.stderr);
                println!("=== {label}: {step}\n{said}--- stderr\n{errors}");
                assert!(out.status.success(), "{label}, {step}: {:?}", out.status);
                assert!(said.ends_with("done\n"), "{label}, {step}");
                said
            };
            let _ = fs::remove_dir_all(home.aliases());

            let said = run("install", INSTALL);
            // mise fetched the release from the mirror nvmise passed on, and
            // not from anywhere else.
            installs += 1;
            let fetched = seen
                .lock()
                .unwrap()
                .iter()
                .filter(|line| line.starts_with(&format!("GET /v{version}/{} ", release.file)))
                .count();
            assert_eq!(fetched, installs, "{label}");

            let told = if how == Mise::Off { "" } else { version };
            for line in [
                format!("Downloading and installing node v{version}...\n"),
                format!("Now using node v{version} (npm v"),
                format!("current:v{version}\n"),
                format!("which:{store}/bin/node\n"),
                format!("node:v{version}\n"),
                format!("used:v{version} told:{told}\n"),
                format!("after-prompt:{store}/bin/node\n"),
                "deactivated told:\n".to_string(),
            ] {
                assert!(said.contains(&line), "{label}: no {line:?}");
            }
            assert!(
                said.lines()
                    .any(|line| line.starts_with("ls:") && line.contains(&format!("v{version}"))),
                "{label}"
            );
            // Where mise is on, it agrees with `nvm use`.
            if how != Mise::Off {
                assert!(
                    said.contains(&format!("mise-picks:{store}/bin/node\n")),
                    "{label}"
                );
            }

            // A new shell. What it ends up with is written down for the
            // end of the run. Without mise, loading nvm puts the default
            // version on `PATH`, as official nvm does. With mise switched
            // on it follows the one choice made in `nvmise::backend`.
            let said = run("load", LOAD);
            let record: Vec<String> = said
                .lines()
                .filter_map(|line| line.strip_prefix("load "))
                .map(|line| line.replace(&home.root.display().to_string(), "<home>"))
                .collect();
            let (path, current, heard) = common::loaded(how != Mise::Off, version);
            let changed = match path {
                "path:same" => "PATH: unchanged".to_string(),
                _ => format!("PATH: changed, first entry {store}/bin"),
            };
            for line in [
                changed,
                format!("MISE_NODE_VERSION: {heard}"),
                format!(
                    "NVM_BIN: {}",
                    match path {
                        "path:same" => String::new(),
                        _ => format!("{store}/bin"),
                    }
                ),
            ] {
                let line = line.replace(&home.root.display().to_string(), "<home>");
                assert!(record.contains(&line), "{label}: no {line:?} in {record:?}");
            }
            // `nvm current` asks whatever `node` is on `PATH`, and with
            // mise switched on that can be mise's stand-in for it. So it
            // is only checked where nothing else has a say.
            if how == Mise::Off {
                assert!(
                    record.contains(&format!("nvm current: {current}")),
                    "{label}: {record:?}"
                );
            }
            loads.push((label.clone(), record));

            let said = run("uninstall", UNINSTALL);
            assert!(
                said.contains(&format!("Uninstalled node v{version}\n")),
                "{label}"
            );
            assert!(!home.folder(version).exists(), "{label}");
        }
    }

    println!(
        "=== what a new shell ends up with at load (choice: {:?})",
        nvmise::backend::AT_LOAD_WITH_MISE
    );
    for (label, record) in &loads {
        println!("{label}");
        for line in record {
            println!("  {line}");
        }
    }
    println!("=== what mise asked the mirror for");
    for line in seen.lock().unwrap().iter() {
        println!("{line}");
    }
    // Nothing of nvm's own layout appeared, and the person's own Node
    // versions are what they were.
    assert!(!home.nvm_dir().join("versions").exists());
    assert_eq!(names_in(&own_store), own_before);
}
