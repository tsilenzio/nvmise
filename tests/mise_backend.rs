//! `MiseBackend` under nvm's commands. Each test runs them through
//! [`nvmimic::run`] in a throwaway home, with a stand-in for the mise
//! program (see `common/mod.rs`), and looks at what was printed, what would
//! change in the shell, and what mise was asked to do.

mod common;

use common::Home;
use nvmimic::context::CallerShell;
use nvmimic::shell::{Dialect, Effect, Vars};
use nvmimic::{Caller, Outcome};
use nvmise::backend::{AT_LOAD_WITH_MISE, AtLoad};
use nvmise::{MiseBackend, NVMISE};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::symlink;

struct World {
    home: Home,
    vars: BTreeMap<String, String>,
}

struct Ran {
    out: String,
    err: String,
    outcome: Outcome,
}

impl World {
    fn new() -> World {
        let home = Home::new("backend");
        let mut vars: BTreeMap<String, String> = home.vars().into_iter().collect();
        // Only the stand-in mise is on `PATH`, so a Node installed on this
        // machine cannot show up as nvm's `system`.
        vars.insert("PATH".to_string(), home.bin().display().to_string());
        World { home, vars }
    }

    fn set(&mut self, name: &str, value: &str) {
        self.vars.insert(name.to_string(), value.to_string());
    }

    fn backend(&self) -> MiseBackend {
        MiseBackend::resolve(|name| self.vars.get(name).cloned()).unwrap()
    }

    fn nvm(&self, args: &[&str]) -> Ran {
        self.call("nvm", args)
    }

    fn call(&self, function: &str, args: &[&str]) -> Ran {
        let caller = Caller {
            vars: Vars::from_env(self.vars.clone()),
            dialect: Dialect::Bash,
            stdout_is_tty: false,
            aliases: Vec::new(),
            command_info: String::new(),
            shell: CallerShell::default(),
            direct_output: false,
            cache_dir: None,
            identity: &NVMISE,
        };
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let outcome = nvmimic::run(
            caller,
            &self.backend(),
            function,
            &args,
            &mut out,
            &mut err,
            &mut |_| false,
        );
        Ran {
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
            outcome,
        }
    }

    /// Carries a call's changes to the variables over to the next call, as
    /// the shell would.
    fn apply(&mut self, ran: &Ran) {
        for effect in &ran.outcome.effects {
            match effect {
                Effect::Set { name, value, .. } => {
                    self.vars.insert(name.clone(), value.clone());
                }
                Effect::Unset(name) => {
                    self.vars.remove(name);
                }
                _ => {}
            }
        }
    }

    /// Nothing of a version's or an alias's may show up in nvm's folder.
    fn assert_nvm_dir_untouched(&self) {
        let nvm_dir = self.home.nvm_dir();
        assert!(!nvm_dir.join("versions").exists());
        assert!(!nvm_dir.join("alias").exists());
        assert!(!nvm_dir.join(".cache").exists());
    }
}

/// Answers every request with a release list holding two versions, and
/// returns the address to use as nvm's mirror.
fn serve_index() -> String {
    const INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
        v22.1.0\t2024-05-02\tosx-arm64-tar,linux-x64,linux-arm64\t10.7.0\t12.4\t1.48.0\t1.3.0\t3.0.13\t127\t-\t-\n\
        v20.11.1\t2024-02-13\tosx-arm64-tar,linux-x64,linux-arm64\t10.2.4\t11.3\t1.46.0\t1.2.13\t3.0.13\t115\tIron\t-\n";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = [0; 2048];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{INDEX}",
                INDEX.len()
            );
        }
    });
    address
}

#[test]
fn lists_the_versions_mise_keeps_and_skips_its_links() {
    let world = World::new();
    world.home.with(&["v20.11.1", "v22.1.0", "v22.3.0"]);
    // mise keeps links beside the versions, and files of its own.
    let store = world.home.store();
    symlink(store.join("22.3.0"), store.join("22")).unwrap();
    symlink(store.join("22.3.0"), store.join("latest")).unwrap();
    symlink(store.join("20.11.1"), store.join("lts-iron")).unwrap();
    fs::write(store.join(".mise.backend"), "core:node\n").unwrap();

    let ran = world.call("nvm_ls", &[]);
    assert_eq!(ran.out, "v20.11.1\nv22.1.0\nv22.3.0\n");

    let ran = world.call("nvm_version", &["22"]);
    assert_eq!((ran.out.as_str(), ran.outcome.status), ("v22.3.0\n", 0));

    let ran = world.call("nvm_version", &["18"]);
    assert_eq!((ran.out.as_str(), ran.outcome.status), ("N/A\n", 3));

    let ran = world.call("nvm_version_path", &["v22.1.0"]);
    assert_eq!(ran.out, format!("{}\n", world.home.path_of("v22.1.0")));

    let ran = world.call("nvm_is_version_installed", &["v20.11.1"]);
    assert_eq!(ran.outcome.status, 0);
    let ran = world.call("nvm_is_version_installed", &["v20.11.2"]);
    assert_ne!(ran.outcome.status, 0);
    // Reading the folder is enough for all of that.
    assert_eq!(world.home.mise_log(), "");
}

#[test]
fn which_points_into_mises_folder() {
    let world = World::new();
    world.home.with(&["v22.1.0"]);
    let ran = world.nvm(&["which", "22"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        format!("{}/bin/node\n", world.home.path_of("v22.1.0"))
    );
}

#[test]
fn use_current_and_deactivate_follow_mises_folder() {
    let mut world = World::new();
    world.home.with(&["v20.11.1", "v22.1.0"]);
    let bin = world.home.bin().display().to_string();

    let ran = world.nvm(&["use", "22"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert!(ran.out.starts_with("Now using node v22.1.0"), "{}", ran.out);
    world.apply(&ran);
    assert_eq!(
        world.vars["PATH"],
        format!("{}/bin:{bin}", world.home.path_of("v22.1.0"))
    );
    assert_eq!(
        world.vars["NVM_BIN"],
        format!("{}/bin", world.home.path_of("v22.1.0"))
    );
    assert_eq!(world.nvm(&["current"]).out, "v22.1.0\n");

    // A second switch takes the first version's place on `PATH`.
    let ran = world.nvm(&["use", "20"]);
    world.apply(&ran);
    assert_eq!(
        world.vars["PATH"],
        format!("{}/bin:{bin}", world.home.path_of("v20.11.1"))
    );

    let ran = world.nvm(&["deactivate"]);
    world.apply(&ran);
    assert_eq!(world.vars["PATH"], bin);
    world.assert_nvm_dir_untouched();
}

#[test]
fn aliases_live_in_nvmises_own_folder() {
    let world = World::new();
    world.home.with(&["v20.11.1", "v22.1.0"]);
    // The folder isn't there before the first alias.
    assert!(!world.home.aliases().exists());

    let ran = world.nvm(&["alias", "default", "20"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert_eq!(
        fs::read_to_string(world.home.aliases().join("default")).unwrap(),
        "20\n"
    );
    assert_eq!(world.call("nvm_version", &["default"]).out, "v20.11.1\n");

    let ran = world.nvm(&["unalias", "default"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert!(!world.home.aliases().join("default").exists());
    world.assert_nvm_dir_untouched();
}

#[test]
fn install_asks_mise_for_the_exact_version() {
    let mut world = World::new();
    // nvm asks the mirror which versions exist before it installs one, and
    // curl fetches the list.
    let mirror = serve_index();
    world.set(
        "PATH",
        &format!("{}:/usr/bin:/bin", world.home.bin().display()),
    );
    world.set("NVM_NODEJS_ORG_MIRROR", &mirror);

    let ran = world.nvm(&["install", "22"]);
    assert_eq!(ran.outcome.status, 0, "{}{}", ran.out, ran.err);
    assert!(
        ran.out
            .starts_with("Downloading and installing node v22.1.0...\n"),
        "{}",
        ran.out
    );
    assert!(ran.out.contains("Now using node v22.1.0"), "{}", ran.out);
    // What mise prints while it works is passed on.
    assert!(
        ran.err.contains("mise node@22.1.0 installed\n"),
        "{}",
        ran.err
    );
    // mise got a whole version, was told not to build from source, and got
    // nvm's mirror with the slash it wants at the end.
    let log = world.home.mise_log();
    assert!(
        log.starts_with(&format!(
            "install node@22.1.0 compile=false wanted=22.1.0 mirror={mirror}/ arch=\n"
        )),
        "{log}"
    );

    world.apply(&ran);
    assert!(world.home.folder("v22.1.0").join("bin/node").exists());
    assert!(
        world.vars["PATH"].starts_with(&format!("{}/bin:", world.home.path_of("v22.1.0"))),
        "{}",
        world.vars["PATH"]
    );
    world.assert_nvm_dir_untouched();

    let ran = world.nvm(&["install", "22"]);
    assert!(
        ran.err.contains("v22.1.0 is already installed."),
        "{}",
        ran.err
    );
    assert_eq!(world.home.mise_log().lines().count(), 1);
}

#[test]
fn an_lts_name_reaches_mise_as_a_version() {
    let mut world = World::new();
    world.set(
        "PATH",
        &format!("{}:/usr/bin:/bin", world.home.bin().display()),
    );
    world.set("NVM_NODEJS_ORG_MIRROR", &serve_index());

    let ran = world.nvm(&["install", "--lts"]);
    assert_eq!(ran.outcome.status, 0, "{}{}", ran.out, ran.err);
    assert!(
        world.home.mise_log().starts_with("install node@20.11.1 "),
        "{}",
        world.home.mise_log()
    );
}

#[test]
fn an_install_that_fails_in_mise_fails_in_nvm() {
    let mut world = World::new();
    world.set(
        "PATH",
        &format!("{}:/usr/bin:/bin", world.home.bin().display()),
    );
    world.set("NVM_NODEJS_ORG_MIRROR", &serve_index());
    fs::write(world.home.root.join("mise-fails"), "").unwrap();

    let ran = world.nvm(&["install", "22"]);
    assert_ne!(ran.outcome.status, 0);
    assert!(ran.err.contains("mise ERROR"), "{}", ran.err);
    assert!(!ran.out.contains("Now using"), "{}", ran.out);
    assert!(!world.home.folder("v22.1.0").exists());
}

#[test]
fn without_mise_nothing_is_installed_or_removed() {
    let mut world = World::new();
    world.home.with(&["v20.11.1"]);
    world.set("NVM_NODEJS_ORG_MIRROR", &serve_index());
    world.set("PATH", "/usr/bin:/bin");

    let ran = world.nvm(&["install", "22"]);
    assert_eq!(ran.outcome.status, 127, "{}", ran.err);
    assert!(
        ran.err.contains("nvmise: could not find the mise program."),
        "{}",
        ran.err
    );

    let ran = world.nvm(&["uninstall", "20"]);
    assert_eq!(ran.outcome.status, 1);
    assert_eq!(
        ran.err,
        "nvm: could not uninstall v20.11.1: could not find the mise program\n"
    );
    assert!(world.home.folder("v20.11.1").exists());

    // `NVMISE_MISE` names a mise that isn't on `PATH`.
    world.set(
        "NVMISE_MISE",
        &world.home.bin().join("mise").display().to_string(),
    );
    let ran = world.nvm(&["uninstall", "20"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert!(!world.home.folder("v20.11.1").exists());
}

#[test]
fn uninstall_goes_through_mise() {
    let world = World::new();
    world.home.with(&["v20.11.1", "v22.1.0"]);
    fs::create_dir_all(world.home.aliases()).unwrap();
    fs::write(world.home.aliases().join("old"), "v20.11.1\n").unwrap();

    let ran = world.nvm(&["uninstall", "20"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert!(
        ran.out.starts_with("Uninstalled node v20.11.1\n"),
        "{}",
        ran.out
    );
    assert!(
        world.home.mise_log().starts_with("uninstall node@20.11.1 "),
        "{}",
        world.home.mise_log()
    );
    assert!(!world.home.folder("v20.11.1").exists());
    assert!(world.home.folder("v22.1.0").exists());
    assert!(!world.home.aliases().join("old").exists());
}

#[test]
fn a_version_mise_will_not_remove_is_not_called_uninstalled() {
    let mut world = World::new();
    world.home.with(&["v20.11.1"]);
    world.set(
        "PATH",
        &format!("{}:/usr/bin:/bin", world.home.bin().display()),
    );
    fs::write(world.home.root.join("mise-fails"), "").unwrap();

    let ran = world.nvm(&["uninstall", "20"]);
    assert_eq!(ran.outcome.status, 1);
    assert_eq!(
        ran.err,
        "nvm: could not uninstall v20.11.1: mise ERROR this stand-in was told to fail\n"
    );
    assert!(world.home.folder("v20.11.1").exists());
}

const MISE_NODE_VERSION: &str = "MISE_NODE_VERSION";

#[test]
fn mise_hears_about_a_switch_only_where_it_is_switched_on() {
    let mut world = World::new();
    world.home.with(&["v20.11.1", "v22.1.0"]);
    let touches_mise = |ran: &Ran| {
        ran.outcome.effects.iter().any(|effect| match effect {
            Effect::Set { name, .. } | Effect::Unset(name) => name == MISE_NODE_VERSION,
            _ => false,
        })
    };

    // A shell without mise: `nvm use` changes `PATH` and that is all.
    let ran = world.nvm(&["use", "22"]);
    assert!(!touches_mise(&ran));
    world.apply(&ran);
    let ran = world.nvm(&["deactivate"]);
    assert!(!touches_mise(&ran));
    world.apply(&ran);

    // A shell with mise switched on.
    world.set("__MISE_DIFF", "x");
    let ran = world.nvm(&["use", "22"]);
    assert!(ran.outcome.effects.contains(&Effect::Set {
        name: MISE_NODE_VERSION.to_string(),
        value: "22.1.0".to_string(),
        export: true,
    }));
    world.apply(&ran);

    let ran = world.nvm(&["use", "20"]);
    world.apply(&ran);
    assert_eq!(world.vars[MISE_NODE_VERSION], "20.11.1");

    let ran = world.nvm(&["deactivate"]);
    world.apply(&ran);
    assert!(!world.vars.contains_key(MISE_NODE_VERSION));
}

#[test]
fn a_loading_shell_follows_the_one_choice_made_for_mise() {
    let mut world = World::new();
    world.home.with(&["v20.11.1", "v22.1.0"]);
    fs::create_dir_all(world.home.aliases()).unwrap();
    fs::write(world.home.aliases().join("default"), "20\n").unwrap();
    world.set("PATH", "/usr/bin:/bin");
    let path = format!("{}/bin:/usr/bin:/bin", world.home.path_of("v20.11.1"));
    // `nvm_auto use` is what a shell runs when it has loaded nvm.
    let load = |world: &World| world.call("nvm_auto", &["use"]);
    let sets_path = |ran: &Ran| {
        ran.outcome.effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Set { name, value, .. } if name == "PATH" && *value == path
            )
        })
    };
    let tells_mise = |ran: &Ran| format!("{:?}", ran.outcome.effects).contains(MISE_NODE_VERSION);

    // Without mise in the shell, loading puts the default version on
    // `PATH` as official nvm does, and mise's variable is left alone.
    let ran = load(&world);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert!(
        sets_path(&ran) && !tells_mise(&ran),
        "{:?}",
        ran.outcome.effects
    );

    // With mise switched on, it goes by `AT_LOAD_WITH_MISE`.
    world.set("__MISE_DIFF", "x");
    let ran = load(&world);
    assert_eq!(ran.outcome.status, 0);
    assert_eq!((ran.out.as_str(), ran.err.as_str()), ("", ""));
    let expected = match AT_LOAD_WITH_MISE {
        AtLoad::LeaveAlone => (false, false),
        AtLoad::SetPathAndTellMise => (true, true),
        AtLoad::SetPathOnly => (true, false),
    };
    assert_eq!(
        (sets_path(&ran), tells_mise(&ran)),
        expected,
        "{:?}",
        ran.outcome.effects
    );
    if AT_LOAD_WITH_MISE == AtLoad::LeaveAlone {
        assert_eq!(ran.outcome.effects, []);
    }

    // `nvm use` typed by someone always works there, and tells mise.
    let ran = world.nvm(&["use", "default"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    world.apply(&ran);
    assert_eq!(world.vars[MISE_NODE_VERSION], "20.11.1");
}

#[test]
fn what_mise_cannot_do_is_turned_down() {
    let mut world = World::new();
    world.home.with(&["v22.1.0"]);

    let ran = world.nvm(&["install", "-s", "20"]);
    assert_eq!(ran.outcome.status, 1);
    assert_eq!(
        ran.err,
        "`nvm install -s` isn't available with this setup.\n"
    );
    assert_eq!(world.home.mise_log(), "");

    // `NVM_SYMLINK_CURRENT` asks for a link in `$NVM_DIR`, and none is made.
    world.set("NVM_SYMLINK_CURRENT", "true");
    let ran = world.nvm(&["use", "22"]);
    assert_eq!(ran.outcome.status, 0, "{}", ran.err);
    assert!(!world.home.nvm_dir().join("current").exists());

    // TAB doesn't offer them either.
    let ran = world.call("__nvm_complete", &["bash", "2", "nvm", "install", "-"]);
    assert!(!ran.out.lines().any(|line| line == "-s"), "{}", ran.out);
    let ran = world.call("__nvm_complete", &["bash", "2", "nvm", "use", ""]);
    assert!(!ran.out.lines().any(|line| line == "iojs"), "{}", ran.out);
}

#[test]
fn there_is_no_nvm_cache_to_use() {
    let world = World::new();
    world.home.with(&["v22.1.0"]);
    // Downloads are mise's. nvm's cache commands and an install from the
    // cache are turned down, and official nvm's cache folder is never
    // touched.
    for (args, what) in [
        (&["cache", "dir"][..], "nvm cache"),
        (&["cache", "clear"], "nvm cache"),
        (&["install", "--offline", "22"], "nvm install --offline"),
    ] {
        let ran = world.nvm(args);
        assert_eq!(ran.outcome.status, 1, "{args:?}");
        assert_eq!(
            ran.err,
            format!("`{what}` isn't available with this setup.\n"),
            "{args:?}"
        );
    }
    world.assert_nvm_dir_untouched();
}
