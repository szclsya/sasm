pub mod dpkg;
pub mod modifier;
mod types;

use crate::types::{PkgActions, PkgMeta};
pub use types::{PkgState, PkgStatus};

use anyhow::{Context, Result};
use debcontrol::{BufParse, Streaming};
use std::{collections::HashMap, fs, path::Path};

/// Status of this machine
pub struct MachineStatus {
    pub pkgs: HashMap<String, PkgStatus>,
}

impl MachineStatus {
    pub fn new(root: &Path) -> Result<Self> {
        let mut res = HashMap::new();
        // Load or create dpkg's status db
        let status_file_dir = root.join("var/lib/dpkg");
        if !status_file_dir.is_dir() {
            fs::create_dir_all(&status_file_dir).context("Failed to initialize dpkg directory.")?;
        }
        let stauts_file_path = root.join("var/lib/dpkg/status");
        let status_file = if stauts_file_path.is_file() {
            fs::File::open(&stauts_file_path).context("Failed to open dpkg status file.")?
        } else {
            fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(&stauts_file_path)
                .context("Failed to initialize dpkg status file.")?
        };

        let mut buf_parse = BufParse::new(status_file, 16384);
        while let Some(result) = buf_parse.try_next().unwrap() {
            match result {
                Streaming::Item(paragraph) => {
                    let mut fields = HashMap::new();
                    for field in paragraph.fields {
                        fields.insert(field.name, field.value);
                    }
                    let pkgstatus = PkgStatus::try_from(fields)?;
                    res.insert(pkgstatus.name.clone(), pkgstatus);
                }
                Streaming::Incomplete => buf_parse.buffer().unwrap(),
            }
        }

        Ok(MachineStatus { pkgs: res })
    }

    /// Generate a list of actions according to machine status and package blueprint
    pub fn gen_actions<'a>(&self, blueprint: &[&'a PkgMeta], purge_config: bool) -> PkgActions<'a> {
        let mut res = PkgActions::default();
        // We will modify the list, so do a clone
        let mut old_pkgs = self.pkgs.clone();

        for newpkg in blueprint {
            if !old_pkgs.contains_key(&newpkg.name) {
                // New one! Install it
                res.install.push((newpkg, None));
            } else {
                // Older version exists. Let's check the state of it
                // Remove it to mark it's been processed
                let oldpkg = old_pkgs.remove(&newpkg.name).unwrap();
                match oldpkg.state {
                    PkgState::NotInstalled | PkgState::ConfigFiles | PkgState::HalfInstalled => {
                        // Just install as normal
                        res.install.push((newpkg, None));
                    }
                    PkgState::Installed => {
                        // Check version. If installed is different,
                        //   then install the one in the blueprint
                        if oldpkg.version != newpkg.version {
                            res.install
                                .push((newpkg, Some((oldpkg.version, oldpkg.install_size))));
                        }
                    }
                    PkgState::Unpacked
                    | PkgState::HalfConfigured
                    | PkgState::TriggerAwaited
                    | PkgState::TriggerPending => {
                        // Reconfigure this package, then if have updates, do it
                        res.configure
                            .push((oldpkg.name.clone(), oldpkg.version.clone()));
                        if oldpkg.version != newpkg.version {
                            res.install
                                .push((newpkg, Some((oldpkg.version, oldpkg.install_size))));
                        }
                    }
                }
            }
        }

        // Now deal with the leftovers
        for oldpkg in old_pkgs {
            match oldpkg.1.state {
                PkgState::Installed
                | PkgState::HalfConfigured
                | PkgState::HalfInstalled
                | PkgState::TriggerAwaited
                | PkgState::TriggerPending
                | PkgState::Unpacked => {
                    if purge_config {
                        res.purge
                            .push((oldpkg.0, oldpkg.1.install_size, oldpkg.1.essential));
                    } else {
                        res.remove
                            .push((oldpkg.0, oldpkg.1.install_size, oldpkg.1.essential));
                    }
                }
                PkgState::ConfigFiles | PkgState::NotInstalled => {
                    // Not installed in the first place, nothing to do
                }
            }
        }
        res
    }
}
