pub mod alpm;
mod types;

use crate::types::{PkgActions, PkgMeta};
pub use types::PkgStatus;

use anyhow::{Context, Result};
use std::{collections::HashMap, fs, path::Path};

/// Status of this machine
pub struct MachineStatus {
    pub pkgs: HashMap<String, PkgStatus>,
}

impl MachineStatus {
    pub async fn new(root: &Path) -> Result<Self> {
        let mut res = HashMap::new();
        // Load or create ALPM local db
        let alpm_local_db_root = root.join("var/lib/pacman/local");
        if !alpm_local_db_root.is_dir() {
            fs::create_dir_all(&alpm_local_db_root)
                .context("Failed to initialize ALPM local database.")?;
        }

        alpm::read_alpm_local_db(&alpm_local_db_root).await?;

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
                todo!()
            }
        }

        // Now deal with the leftovers
        for oldpkg in old_pkgs {
            todo!()
        }
        res
    }
}
