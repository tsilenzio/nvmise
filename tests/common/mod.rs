//! A throwaway home for the tests, with a stand-in for the mise program.
//! Nothing here touches the mise or the nvm folder of the person running
//! the tests.

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A stand-in for mise. It knows `install node@<version>` and `uninstall
/// node@<version>`, and writes every call into `$HOME/mise.log` with the
/// settings it was given. A file `$HOME/mise-fails` makes it fail.
const MISE: &str = r#"#!/bin/sh
PATH=/usr/bin:/bin
store="${MISE_DATA_DIR:-$HOME/.local/share/mise}/installs/node"
echo "$* compile=${MISE_NODE_COMPILE-} wanted=${MISE_NODE_VERSION-} mirror=${MISE_NODE_MIRROR_URL-} arch=${MISE_ARCH-}" >> "$HOME/mise.log"
if [ -e "$HOME/mise-fails" ]; then
  echo "mise ERROR this stand-in was told to fail" >&2
  exit 1
fi
version="${2#node@}"
case "$1" in
  install)
    mkdir -p "$store/$version/bin"
    printf '#!/bin/sh\necho v%s\n' "$version" > "$store/$version/bin/node"
    printf '#!/bin/sh\necho 10.7.0\n' > "$store/$version/bin/npm"
    chmod +x "$store/$version/bin/node" "$store/$version/bin/npm"
    echo "mise node@$version installed" >&2
    ;;
  uninstall)
    rm -rf "${store:?}/$version"
    echo "mise node@$version uninstalled" >&2
    ;;
  *)
    echo "mise ERROR the stand-in does not know $1" >&2
    exit 2
    ;;
esac
"#;

/// A home folder that is deleted again when the test ends.
pub struct Home {
    pub root: PathBuf,
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Home {
    pub fn new(label: &str) -> Home {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("nvmise-{label}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        // macOS's temp folder is behind a symlink, and nvm compares paths
        // as text, so the tests use the real path throughout.
        let root = root.canonicalize().unwrap();
        let home = Home { root };
        for dir in [home.store(), home.nvm_dir(), home.bin()] {
            fs::create_dir_all(dir).unwrap();
        }
        write_program(&home.bin().join("mise"), MISE);
        home
    }

    /// Where the stand-in mise is, and nothing else.
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn nvm_dir(&self) -> PathBuf {
        self.root.join(".nvm")
    }

    pub fn mise_data(&self) -> PathBuf {
        self.root.join("mise/data")
    }

    /// mise's folder of Node versions.
    pub fn store(&self) -> PathBuf {
        self.mise_data().join("installs/node")
    }

    pub fn aliases(&self) -> PathBuf {
        self.root.join(".local/share/nvmise/alias")
    }

    pub fn folder(&self, version: &str) -> PathBuf {
        self.store().join(version.trim_start_matches('v'))
    }

    pub fn path_of(&self, version: &str) -> String {
        self.folder(version).display().to_string()
    }

    /// The variables that keep mise and nvmise inside this home. Every folder
    /// mise uses is pointed here, so a test that starts the mise program of
    /// this machine still can't reach the folders of the person running it.
    pub fn vars(&self) -> Vec<(String, String)> {
        let inside = |path: &str| self.root.join(path).display().to_string();
        vec![
            ("HOME".to_string(), self.root.display().to_string()),
            ("NVM_DIR".to_string(), inside(".nvm")),
            ("PWD".to_string(), self.root.display().to_string()),
            ("MISE_DATA_DIR".to_string(), inside("mise/data")),
            ("MISE_CONFIG_DIR".to_string(), inside("mise/config")),
            ("MISE_CACHE_DIR".to_string(), inside("mise/cache")),
            ("MISE_STATE_DIR".to_string(), inside("mise/state")),
            // Where mise looks for settings that apply to the whole machine.
            ("MISE_SYSTEM_DIR".to_string(), inside("mise/system")),
        ]
    }

    /// Puts stand-ins for installed versions into mise's folder.
    pub fn with(&self, versions: &[&str]) -> &Home {
        for version in versions {
            fake_node(&self.folder(version), version);
        }
        self
    }

    /// What the stand-in mise was asked, one call per line.
    pub fn mise_log(&self) -> String {
        fs::read_to_string(self.root.join("mise.log")).unwrap_or_default()
    }
}

/// A stand-in for an installed Node: a `node` and an `npm` that only know
/// their versions.
pub fn fake_node(folder: &Path, version: &str) {
    let bin = folder.join("bin");
    fs::create_dir_all(&bin).unwrap();
    write_program(&bin.join("node"), &format!("#!/bin/sh\necho {version}\n"));
    write_program(&bin.join("npm"), "#!/bin/sh\necho 10.7.0\n");
}

fn write_program(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// The three scripts other tools look for in the nvm folder.
pub const SCRIPTS: [&str; 3] = ["nvm.sh", "nvm-exec", "bash_completion"];

/// What stands in for one of official nvm's scripts.
pub fn official_text(name: &str) -> String {
    format!("official {name}\n")
}

/// Whether Homebrew's nvm is on this machine. `self install` offers to run
/// `brew uninstall nvm` when it is, and it looks in Homebrew's usual places
/// whatever the test's environment says. A test that runs `self install
/// --force` must not run on such a machine.
pub fn homebrew_nvm_is_here() -> bool {
    ["/opt/homebrew", "/usr/local", "/home/linuxbrew/.linuxbrew"]
        .iter()
        .any(|prefix| Path::new(prefix).join("opt/nvm").exists())
}

impl Home {
    /// Puts stand-ins for official nvm's three scripts into the nvm folder.
    pub fn with_official_nvm(&self) -> &Home {
        for name in SCRIPTS {
            fs::write(self.nvm_dir().join(name), official_text(name)).unwrap();
        }
        self
    }

    /// Whether official nvm's three scripts are in the nvm folder.
    pub fn official_is_back(&self) -> bool {
        SCRIPTS.iter().all(|name| {
            fs::read_to_string(self.nvm_dir().join(name))
                .is_ok_and(|text| text == official_text(name))
        })
    }

    /// Who answers `nvm`: a program's name, `official`, or `nobody`.
    pub fn answers(&self) -> String {
        match fs::read_to_string(self.nvm_dir().join("nvm.sh")) {
            Ok(script) if script == official_text("nvm.sh") => "official".to_string(),
            Ok(script) => nvmimic::identity::program_of(&script)
                .unwrap_or("someone else")
                .to_string(),
            Err(_) => "nobody".to_string(),
        }
    }

    /// A program's own state folder.
    pub fn state(&self, program: &str) -> PathBuf {
        self.root.join(".local/state").join(program)
    }

    /// Whether a program is set up, in control or asleep.
    pub fn installed(&self, program: &str) -> bool {
        self.state(program).join("manifest").is_file()
    }

    /// The folder nvmise and nvmimic share: official nvm's scripts and the
    /// record of who answers `nvm`.
    pub fn shared(&self) -> PathBuf {
        self.state("nvmimic-shared")
    }

    /// Whether the kept copies of official nvm's scripts are all there.
    pub fn backup_is_whole(&self) -> bool {
        SCRIPTS.iter().all(|name| {
            fs::read_to_string(self.shared().join("backup").join(name))
                .is_ok_and(|text| text == official_text(name))
        })
    }
}
