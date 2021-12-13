pub mod debrepo;
pub mod local;

use super::{BasicPkgPool, InMemoryPool, PkgPool};
use anyhow::Result;
use std::path::PathBuf;

pub fn create_pool(
    deb_dbs: &[(String, PathBuf)],
    local_deb_roots: &[PathBuf],
) -> Result<Box<dyn PkgPool>> {
    let mut pool = InMemoryPool::new();
    for (root_url, deb_db) in deb_dbs {
        debrepo::import(deb_db, &mut pool, root_url)?;
    }

    // Import debs
    for deb_root in local_deb_roots {
        for pkg in local::read_debs_from_path(deb_root)? {
            pool.add(pkg);
        }
    }

    pool.finalize();
    Ok(Box::new(pool))
}
