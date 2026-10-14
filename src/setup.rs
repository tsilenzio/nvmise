//! nvmise's own part of `self install` and `self uninstall`.
//!
//! The nvmimic library does the big steps: it swaps the scripts in the nvm
//! folder, keeps official nvm's, and writes down who answers `nvm`. It
//! leaves room for a program's own steps, and these are nvmise's. They
//! look after the one thing only nvmise has, its folder of aliases.

use crate::MiseBackend;
use crate::backend::Mise;
use nvmimic::install::{Leaving, Manifest, Site, Steps, Ui};
use std::fs;
use std::path::Path;

/// The line in the install record that says where the aliases are.
const ALIASES: &str = "aliases";

pub struct MiseSteps<'a>(pub &'a MiseBackend);

impl Steps for MiseSteps<'_> {
    fn install_plan(&self, _site: &Site) -> Vec<String> {
        let aliases = self.0.aliases();
        match aliases.is_dir() {
            true => Vec::new(),
            false => vec![format!(
                "make the folder for nvm's aliases at {}",
                aliases.display()
            )],
        }
    }

    fn install(
        &self,
        _site: &Site,
        manifest: &mut Manifest,
        ui: &mut dyn Ui,
    ) -> Result<(), String> {
        let aliases = self.0.aliases();
        fs::create_dir_all(aliases).map_err(|error| format!("{}: {error}", aliases.display()))?;
        manifest.extra.retain(|(key, _)| key != ALIASES);
        manifest
            .extra
            .push((ALIASES.to_string(), aliases.display().to_string()));
        ui.say(&format!(
            "Node versions are the ones mise keeps in {}.",
            self.0.store().display()
        ));
        let path = std::env::var("PATH").unwrap_or_default();
        match self.0.find_mise(&path) {
            Mise::Named(_) | Mise::OnPath(_) => {}
            Mise::Elsewhere(found) => {
                ui.say(&format!(
                    "mise isn't on your PATH. nvmise found it at {} and uses that for `nvm \
                     install` and `nvm uninstall`.",
                    found.display()
                ));
                ui.say(
                    "mise's own `npm` calls `mise` by name after `npm install -g`, so put mise \
                     on your PATH for that.",
                );
            }
            Mise::Missing => ui.say(
                "nvmise could not find the mise program. `nvm install` and `nvm uninstall` \
                 need it. Install mise, or set NVMISE_MISE to where it is.",
            ),
        }
        Ok(())
    }

    fn leave_plan(&self, _site: &Site, how: Leaving, manifest: &Manifest) -> Vec<String> {
        // Switched off, nvmise keeps its aliases for when it comes back.
        if how == Leaving::Eject {
            return Vec::new();
        }
        let Some(folder) = manifest.extra(ALIASES).next() else {
            return Vec::new();
        };
        let names = alias_names(Path::new(folder));
        vec![match names.is_empty() {
            true => format!("delete the empty folder for nvm's aliases at {folder}"),
            false => format!(
                "delete the aliases made with nvmise in {folder}: {}",
                names.join(", ")
            ),
        }]
    }

    fn leave(
        &self,
        site: &Site,
        how: Leaving,
        manifest: &mut Manifest,
        _ui: &mut dyn Ui,
    ) -> Result<(), String> {
        if how == Leaving::Eject {
            return Ok(());
        }
        if let Some(folder) = manifest.extra(ALIASES).next() {
            let _ = fs::remove_dir_all(folder);
        }
        // What is left of nvmise's own folders, when nothing else is in them.
        let _ = fs::remove_dir_all(site.dirs.cache.join("nvm"));
        let _ = fs::remove_dir(&site.dirs.cache);
        let _ = fs::remove_dir(&site.dirs.data);
        Ok(())
    }
}

/// The aliases in a folder, by name, with the ones in `lts/` as `lts/<name>`.
fn alias_names(folder: &Path) -> Vec<String> {
    let files = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|read| {
                read.flatten()
                    .filter(|entry| entry.path().is_file())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    let mut names = files(folder);
    names.extend(
        files(&folder.join("lts"))
            .into_iter()
            .map(|name| format!("lts/{name}")),
    );
    names
}
