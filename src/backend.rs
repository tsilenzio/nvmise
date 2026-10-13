//! Where nvmise's Node versions live: in mise's own folder.
//!
//! mise keeps every Node version in one folder, `installs/node`, with one
//! folder per version named without the leading `v`: `installs/node/22.1.0`.
//! [`MiseBackend`] tells the nvmimic library about that folder. Listing
//! versions and finding one only reads the folder, which is fast. Installing
//! and removing a version start the `mise` program, so mise's own records
//! stay right.
//!
//! Aliases such as `default` are nvm's idea and mise has nothing like them,
//! so nvmise keeps them in a folder of its own.

use nvmimic::backend::{self, Backend, Flavor, Flavors, InstallRequest, Installed};
use nvmimic::complete::Needs;
use nvmimic::context::Nvm;
use nvmimic::dirs::Dirs;
use nvmimic::nvm::{paths, tools};
use nvmimic::shell::Vars;
use nvmimic::text;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The variable mise sets in a shell it is switched on in (`mise activate`).
const MISE_ACTIVE: &str = "__MISE_DIFF";
/// The variable that tells mise which Node version to use, ahead of any
/// config file.
const MISE_NODE_VERSION: &str = "MISE_NODE_VERSION";

pub struct MiseBackend {
    /// mise's `installs/node` folder.
    store: PathBuf,
    aliases: PathBuf,
    /// Where nvm's install locks go, away from official nvm's own
    /// `$NVM_DIR/.cache`.
    cache: PathBuf,
    /// The mise program, when `NVMISE_MISE` names it. Otherwise it is looked
    /// up on the calling shell's `PATH`.
    mise: Option<PathBuf>,
    /// Whether this is for a shell that is only now loading nvm.
    loading: bool,
}

impl MiseBackend {
    /// Works the folders out from the environment. `var` returns the value
    /// of a variable. `None` when there is no home folder and no variable
    /// that says where things are.
    pub fn resolve(var: impl Fn(&str) -> Option<String>) -> Option<MiseBackend> {
        let absolute = |name: &str| {
            var(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        // The order mise itself looks in.
        let installs = absolute("MISE_INSTALLS_DIR")
            .or_else(|| absolute("MISE_DATA_DIR").map(|data| data.join("installs")))
            .or_else(|| absolute("XDG_DATA_HOME").map(|data| data.join("mise/installs")))
            .or_else(|| absolute("HOME").map(|home| home.join(".local/share/mise/installs")))?;
        let dirs = Dirs::resolve_for(&crate::NVMISE, &var)?;
        Some(MiseBackend {
            store: installs.join("node"),
            aliases: dirs.data.join("alias"),
            cache: dirs.cache.join("nvm"),
            mise: absolute("NVMISE_MISE"),
            loading: false,
        })
    }

    /// [`MiseBackend::resolve`] for this process's environment.
    pub fn from_process() -> Option<MiseBackend> {
        MiseBackend::resolve(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
    }

    /// For a shell that is only now loading nvm. nvm switches such a shell
    /// to the default version without being asked. mise is not told about
    /// that switch, so it never beats what mise itself picked.
    pub fn while_loading(mut self) -> MiseBackend {
        self.loading = true;
        self
    }

    /// The folder of one version. mise's names have no `v` in front.
    pub fn folder(&self, version: &str) -> PathBuf {
        self.store.join(plain(version))
    }

    pub fn store(&self) -> &Path {
        &self.store
    }

    pub fn aliases(&self) -> &Path {
        &self.aliases
    }

    fn store_text(&self) -> String {
        self.store.display().to_string()
    }

    /// The mise program, set up the way the calling shell would start it:
    /// with the shell's exported variables, in the shell's folder. It gets
    /// nothing to read, so it can never stop and ask a question.
    fn mise(&self, vars: &Vars) -> Option<Command> {
        let program = match &self.mise {
            Some(program) => program.clone(),
            None => PathBuf::from(paths::which(vars.get("PATH"), "mise")?),
        };
        let mut command = Command::new(program);
        command
            .env_clear()
            .envs(vars.exported())
            .stdin(Stdio::null());
        if !vars.get("PWD").is_empty() {
            command.current_dir(vars.get("PWD"));
        }
        Some(command)
    }
}

const NO_MISE: &str = "nvmise: could not find the mise program. Put mise on your PATH, or set NVMISE_MISE to where it is.";

impl Backend for MiseBackend {
    fn root(&self, vars: &Vars) -> String {
        vars.get("NVM_DIR").to_string()
    }

    fn installed(&self, _vars: &Vars, flavors: Flavors) -> Vec<Installed> {
        if flavors == Flavors::Iojs {
            return Vec::new();
        }
        // mise also keeps links here, such as `22` and `latest`, that point
        // at a version's folder. `is_dir` is false for those.
        backend::dir_entries(&self.store_text())
            .into_iter()
            .filter(|entry| entry.is_dir && is_exact(&entry.name))
            .map(|entry| Installed {
                flavor: Flavor::Node,
                name: format!("v{}", entry.name),
            })
            .collect()
    }

    fn version_path(&self, _vars: &Vars, version: &str) -> String {
        self.folder(version).display().to_string()
    }

    fn owner_of(&self, _vars: &Vars, path: &str) -> Option<Flavor> {
        (paths::tree_contains_path(&self.store_text(), path) == Some(true)).then_some(Flavor::Node)
    }

    fn remove(&self, vars: &Vars, version: &str) -> io::Result<()> {
        let mut mise = self
            .mise(vars)
            .ok_or_else(|| io::Error::other("could not find the mise program"))?;
        let output = mise
            .args(["uninstall", &format!("node@{}", plain(version))])
            .output()?;
        if !output.status.success() {
            let said = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::other(match said.trim() {
                "" => "mise uninstall failed",
                said => said,
            }));
        }
        if self.folder(version).exists() {
            return Err(io::Error::other("mise left the version in place"));
        }
        Ok(())
    }

    // mise unpacks releases that are already built, keeps Node only, and
    // has no `current` link. A hook that installs on its own would leave
    // mise not knowing about the version.
    fn offers(&self, needs: Needs) -> bool {
        !matches!(
            needs,
            Needs::Source | Needs::Iojs | Needs::SymlinkCurrent | Needs::InstallHook
        )
    }

    fn install(&self, nvm: &mut Nvm, request: &InstallRequest) -> Option<i32> {
        let Some(mut mise) = self.mise(&nvm.vars) else {
            nvm.io.err(NO_MISE);
            return Some(127);
        };
        nvm.io.echo(&text::install::downloading_and_installing(
            "node",
            request.version,
        ));
        let version = plain(request.version);
        let apple_silicon = cfg!(all(target_os = "macos", target_arch = "aarch64"));
        mise.args(["install", &format!("node@{version}")])
            .envs(install_settings(
                version,
                nvm.vars.get("NVM_NODEJS_ORG_MIRROR"),
                apple_silicon,
            ));
        Some(tools::run_through(nvm, mise))
    }

    fn switched(&self, vars: &mut Vars, version: Option<&str>) {
        // Without mise switched on in this shell, nothing undoes nvm's
        // change to `PATH`, so there is nothing to tell.
        if self.loading || !vars.is_set(MISE_ACTIVE) {
            return;
        }
        match version {
            // mise puts its own pick of Node back on `PATH` before every
            // prompt. This makes the version `nvm use` chose that pick.
            Some(version) => vars.export(MISE_NODE_VERSION, plain(version)),
            None if vars.is_set(MISE_NODE_VERSION) => vars.unset(MISE_NODE_VERSION),
            None => {}
        }
    }

    fn cache_dir(&self, _vars: &Vars) -> String {
        self.cache.display().to_string()
    }

    fn strip_path(&self, _vars: &Vars, list: &str, suffix: &str) -> String {
        paths::strip_path(list, &self.store_text(), suffix)
    }

    fn change_path(&self, _vars: &Vars, list: &str, suffix: &str, version_path: &str) -> String {
        paths::change_path(list, suffix, version_path, &self.store_text())
    }

    fn alias_dir(&self, _vars: &Vars) -> String {
        self.aliases.display().to_string()
    }

    fn write_alias(&self, _vars: &Vars, name: &str, target: &str) -> io::Result<()> {
        let file = self.aliases.join(name);
        if let Some(folder) = file.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(file, format!("{target}\n"))
    }
}

/// nvm's `v22.1.0` as mise names it: `22.1.0`.
fn plain(version: &str) -> &str {
    version.strip_prefix('v').unwrap_or(version)
}

/// Whether a folder name is one whole version, such as `22.1.0`.
fn is_exact(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The settings `mise install` runs with, as environment variables.
/// `mirror` is nvm's `NVM_NODEJS_ORG_MIRROR`, empty when it isn't set.
fn install_settings(
    version: &str,
    mirror: &str,
    apple_silicon: bool,
) -> Vec<(&'static str, String)> {
    let mut settings = vec![
        // Without this, mise tries to build Node from source when it finds
        // no download, which takes a long time and then fails.
        ("MISE_NODE_COMPILE", "false".to_string()),
        // mise warns about a version that no config asks for. This asks.
        (MISE_NODE_VERSION, version.to_string()),
    ];
    if mirror.starts_with("http://") || mirror.starts_with("https://") {
        // mise wants the address to end in a slash, and nvm's has none.
        let mirror = mirror.trim_end_matches('/');
        settings.push(("MISE_NODE_MIRROR_URL", format!("{mirror}/")));
    }
    // Node before 16 was never built for Apple's own chips. The Intel build
    // runs there, and mise has to be told to fetch it.
    let major: u32 = version
        .split('.')
        .next()
        .and_then(|major| major.parse().ok())
        .unwrap_or(u32::MAX);
    if apple_silicon && major < 16 {
        settings.push(("MISE_ARCH", "x64".to_string()));
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn backend(vars: &[(&str, &str)]) -> Option<MiseBackend> {
        let vars: BTreeMap<&str, &str> = vars.iter().copied().collect();
        MiseBackend::resolve(|name| vars.get(name).map(|value| value.to_string()))
    }

    #[test]
    fn the_store_is_found_the_way_mise_finds_it() {
        let home = ("HOME", "/h");
        let store = |vars: &[(&str, &str)]| backend(vars).unwrap().store;
        assert_eq!(
            store(&[home]),
            Path::new("/h/.local/share/mise/installs/node")
        );
        assert_eq!(
            store(&[home, ("XDG_DATA_HOME", "/x")]),
            Path::new("/x/mise/installs/node")
        );
        assert_eq!(
            store(&[home, ("XDG_DATA_HOME", "/x"), ("MISE_DATA_DIR", "/d")]),
            Path::new("/d/installs/node")
        );
        assert_eq!(
            store(&[home, ("MISE_DATA_DIR", "/d"), ("MISE_INSTALLS_DIR", "/i")]),
            Path::new("/i/node")
        );
        // A path that doesn't start at the root counts as not set.
        assert_eq!(
            store(&[home, ("MISE_DATA_DIR", "data")]),
            Path::new("/h/.local/share/mise/installs/node")
        );
        assert!(backend(&[]).is_none());
    }

    #[test]
    fn aliases_and_the_cache_are_nvmises_own() {
        let found = backend(&[("HOME", "/h")]).unwrap();
        assert_eq!(found.aliases, Path::new("/h/.local/share/nvmise/alias"));
        assert!(found.cache.ends_with("nvmise/nvm"), "{:?}", found.cache);
        let moved = backend(&[("HOME", "/h"), ("NVMISE_DATA_DIR", "/n")]).unwrap();
        assert_eq!(moved.aliases, Path::new("/n/alias"));
    }

    #[test]
    fn only_whole_versions_count_as_installed() {
        for name in ["22.1.0", "0.10.48", "4.9.1"] {
            assert!(is_exact(name), "{name}");
        }
        for name in [
            "22",
            "22.1",
            "latest",
            "lts",
            "lts-iron",
            "22.1.0-rc.1",
            "v22.1.0",
            "",
        ] {
            assert!(!is_exact(name), "{name}");
        }
    }

    #[test]
    fn mise_is_told_how_to_install() {
        let named = |settings: Vec<(&'static str, String)>| -> Vec<String> {
            settings
                .into_iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect()
        };
        assert_eq!(
            named(install_settings("22.1.0", "", true)),
            ["MISE_NODE_COMPILE=false", "MISE_NODE_VERSION=22.1.0"]
        );
        assert_eq!(
            named(install_settings(
                "14.21.3",
                "https://example.test/dist",
                true
            )),
            [
                "MISE_NODE_COMPILE=false",
                "MISE_NODE_VERSION=14.21.3",
                "MISE_NODE_MIRROR_URL=https://example.test/dist/",
                "MISE_ARCH=x64"
            ]
        );
        assert_eq!(
            named(install_settings(
                "14.21.3",
                "https://example.test/dist/",
                false
            )),
            [
                "MISE_NODE_COMPILE=false",
                "MISE_NODE_VERSION=14.21.3",
                "MISE_NODE_MIRROR_URL=https://example.test/dist/"
            ]
        );
        // Not an address mise can use, so mise keeps its own.
        assert_eq!(named(install_settings("16.0.0", "ftp://x", true)).len(), 2);
    }
}
