pub mod dpkg;
mod error;
mod types;

use crate::solver::PackageMeta;
pub use error::ExecutionError;
pub use types::{PkgAction, PkgState, PkgStatus};

use debcontrol::{BufParse, Streaming};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};

/// Status of this machine
pub struct MachineStatus {
    pkgs: HashMap<String, PkgStatus>,
}

impl MachineStatus {
    pub fn new(root: &Path) -> Result<Self, ExecutionError> {
        let mut res = HashMap::new();
        // Load dpkg's status db
        let stauts_file_path = root.join("var/lib/dpkg/status");
        let status_file = fs::File::open(&stauts_file_path)
            .map_err(|err| ExecutionError::StateError(err.to_string()))?;
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
    pub fn gen_actions(&self, wishlist: &[&PackageMeta], purge_config: bool) -> Vec<PkgAction> {
        let mut res: Vec<PkgAction> = Vec::new();
        // We will modify the list, so do a clone
        let mut old_pkgs = self.pkgs.clone();

        for newpkg in wishlist {
            if !old_pkgs.contains_key(&newpkg.name) {
                // New one! Install it
                res.push(PkgAction::Install(
                    newpkg.name.clone(),
                    newpkg.url.clone(),
                ));
            } else {
                // Older version exists. Let's check the state of it
                // Remove it to mark it's been processed
                let oldpkg = old_pkgs.remove(&newpkg.name).unwrap();
                match oldpkg.state {
                    PkgState::NotInstalled | PkgState::ConfigFiles | PkgState::HalfInstalled => {
                        // Just install as normal
                        res.push(PkgAction::Install(
                            newpkg.name.clone(),
                            newpkg.url.clone(),
                        ));
                    }
                    PkgState::Installed => {
                        // Check version. If installed is different,
                        //   then install the one in the wishlist
                        if oldpkg.version != newpkg.version {
                            res.push(PkgAction::Upgrade(
                                newpkg.name.clone(),
                                newpkg.url.clone(),
                            ));
                        }
                    }
                    PkgState::Unpacked
                    | PkgState::HalfConfigured
                    | PkgState::TriggerAwaited
                    | PkgState::TriggerPending => {
                        // Reconfigure this package, then if have updates, do it
                        res.push(PkgAction::Reconfigure(oldpkg.name.clone()));
                        if oldpkg.version != newpkg.version {
                            res.push(PkgAction::Upgrade(
                                newpkg.name.clone(),
                                newpkg.url.clone(),
                            ));
                        }
                    }
                }
            }
        }

        // Now deal with the leftovers
        let mut remove_list = Vec::new();
        let mut purge_list = Vec::new();
        for oldpkg in old_pkgs {
            match oldpkg.1.state {
                PkgState::Installed => {
                    if purge_config {
                        purge_list.push(oldpkg.0);
                    } else {
                        remove_list.push(oldpkg.0);
                    }
                }
                PkgState::HalfConfigured
                | PkgState::HalfInstalled
                | PkgState::TriggerAwaited
                | PkgState::TriggerPending
                | PkgState::Unpacked => {
                    // Just purge it
                    purge_list.push(oldpkg.0);
                }
                PkgState::ConfigFiles | PkgState::NotInstalled => {
                    // Not installed in the first place, nothing to do
                }
            }
        }
        res.push(PkgAction::Remove(remove_list, false));
        res.push(PkgAction::Remove(purge_list, true));
        res
    }
}
