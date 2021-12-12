pub mod debrepo;
pub mod local;

use super::{BasicPkgPool, InMemoryPool, PkgPool};
use crate::debug;
use anyhow::{bail, Result};
use std::{fs, path::PathBuf, ffi::OsStr};

pub fn create_pool(
    deb_dbs: &[(String, PathBuf)],
    local_deb_roots: &[PathBuf],
) -> Result<Box<dyn PkgPool>> {
    let mut pool = InMemoryPool::new();
    for (root_url, deb_db) in deb_dbs {
        debrepo::import(&deb_db, &mut pool, root_url)?;
    }

    // Import debs
    for deb_root in local_deb_roots {
        // Read dir
        if !deb_root.is_dir() {
            bail!(
                "Invalid local repository: {} is not a dir",
                deb_root.display()
            );
        }
        for entry in fs::read_dir(deb_root)? {
            let entry = entry?;
            let path = entry.path();
            debug!("Parsing local deb {}", path.display());
            if !path.is_file() || path.extension() != Some(OsStr::new("deb")) {
                continue;
            }
            // Now we confirm it is a deb file. Read it and add it to pool
            let pkgmeta = local::read_control_from_deb(&path)?;
            debug!("{:?}", pkgmeta);
            pool.add(pkgmeta);
        }
    }

    pool.finalize();
    Ok(Box::new(pool))
}
