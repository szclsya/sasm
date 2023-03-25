pub mod pacdb;

use super::{BasicPkgPool, InMemoryPool, PkgPool};
use anyhow::Result;
use std::path::PathBuf;

pub fn create_pool(
    pac_dbs: &[(String, PathBuf)],
    _local_deb_roots: &[PathBuf],
) -> Result<Box<dyn PkgPool>> {
    let mut pool = InMemoryPool::new();
    for (root_url, pac_db) in pac_dbs {
        pacdb::import(pac_db, &mut pool, root_url)?;
    }

    pool.finalize();
    Ok(Box::new(pool))
}
