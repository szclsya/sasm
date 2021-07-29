pub mod download;
pub mod dpkg;
mod types;

use crate::types::{PkgActions, PkgMeta};
pub use types::{PkgState, PkgStatus};

use anyhow::{Context, Result};
use debcontrol::{BufParse, Streaming};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::path::Path;

/// Status of this machine
pub struct MachineStatus {
    pkgs: HashMap<String, PkgStatus>,
}

impl MachineStatus {
    pub fn new(root: &Path) -> Result<Self> {
        let mut res = HashMap::new();
        // Load dpkg's status db
        let stauts_file_path = root.join("var/lib/dpkg/status");
        let status_file =
            fs::File::open(&stauts_file_path).context("Failed to open dpkg status file")?;
        let mut buf_parse = BufParse::new(status_file, 4096);
        while let Some(result) = buf_parse.try_next().unwrap() {
            match result {
                Streaming::Item(paragraph) => {
                    let mut fields = HashMap::new();
                    for field in paragraph.fields {
                        fields.insert(field.name, field.value);
                    }
                    let pkgstatus = PkgStatus::try_from(&fields)?;
                    res.insert(pkgstatus.name.clone(), pkgstatus);
                }
                Streaming::Incomplete => buf_parse.buffer().unwrap(),
            }
        }

        Ok(MachineStatus { pkgs: res })
    }

    /// Generate a list of actions according to machine status and package wishlist
    pub fn gen_actions(&self, wishlist: &[&PkgMeta], purge_config: bool) -> PkgActions {
        let mut res = PkgActions::default();
        // We will modify the list, so do a clone
        let mut old_pkgs = self.pkgs.clone();

        for newpkg in wishlist {
            if !old_pkgs.contains_key(&newpkg.name) {
                // New one! Install it
                res.install.push((
                    newpkg.name.clone(),
                    newpkg.url.clone(),
                    newpkg.size,
                    newpkg.checksum.clone(),
                    newpkg.version.clone(),
                    None,
                ));
            } else {
                // Older version exists. Let's check the state of it
                // Remove it to mark it's been processed
                let oldpkg = old_pkgs.remove(&newpkg.name).unwrap();
                match oldpkg.state {
                    PkgState::NotInstalled | PkgState::ConfigFiles | PkgState::HalfInstalled => {
                        // Just install as normal
                        res.install.push((
                            newpkg.name.clone(),
                            newpkg.url.clone(),
                            newpkg.size,
                            newpkg.checksum.clone(),
                            newpkg.version.clone(),
                            None,
                        ));
                    }
                    PkgState::Installed => {
                        // Check version. If installed is different,
                        //   then install the one in the wishlist
                        if oldpkg.version != newpkg.version {
                            res.install.push((
                                newpkg.name.clone(),
                                newpkg.url.clone(),
                                newpkg.size,
                                newpkg.checksum.clone(),
                                newpkg.version.clone(),
                                Some(oldpkg.version),
                            ));
                        }
                    }
                    PkgState::Unpacked
                    | PkgState::HalfConfigured
                    | PkgState::TriggerAwaited
                    | PkgState::TriggerPending => {
                        // Reconfigure this package, then if have updates, do it
                        res.configure.push(oldpkg.name.clone());
                        if oldpkg.version != newpkg.version {
                            res.install.push((
                                newpkg.name.clone(),
                                newpkg.url.clone(),
                                newpkg.size,
                                newpkg.checksum.clone(),
                                newpkg.version.clone(),
                                Some(oldpkg.version),
                            ));
                        }
                    }
                }
            }
        }

        // Now deal with the leftovers
        for oldpkg in old_pkgs {
            match oldpkg.1.state {
                PkgState::Installed => {
                    if purge_config {
                        res.purge.push(oldpkg.0);
                    } else {
                        res.remove.push(oldpkg.0);
                    }
                }
                PkgState::HalfConfigured
                | PkgState::HalfInstalled
                | PkgState::TriggerAwaited
                | PkgState::TriggerPending
                | PkgState::Unpacked => {
                    // Just purge it
                    res.purge.push(oldpkg.0);
                }
                PkgState::ConfigFiles | PkgState::NotInstalled => {
                    // Not installed in the first place, nothing to do
                }
            }
        }
        res
    }
}
