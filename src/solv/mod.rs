mod actions;
mod ffi;
use std::path::PathBuf;

use anyhow::Result;
pub use ffi::{Pool, Queue, Repo, Solver, Transaction, SOLVER_FLAG_BEST_OBEY_POLICY};
use libc::c_int;
use libsolv_sys::ffi::SOLVER_FLAG_ALLOW_UNINSTALL;

#[derive(Clone, Debug)]
pub enum PackageAction {
    Noop,
    // true = reinstall, false = normal install
    Install(bool),
    Erase,
    Downgrade,
    Upgrade,
}

#[derive(Clone, Debug)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub action: PackageAction,
}

/// Populate the packages pool with metadata
pub fn populate_pool(pool: &mut Pool, paths: &[PathBuf]) -> Result<()> {
    let mut repo = Repo::new(pool, "stable")?;
    for path in paths {
        repo.add_debpackages(path)?;
    }
    let mut system = Repo::new(pool, "@System")?;
    system.add_debdb()?;
    pool.set_installed(&system);

    pool.createwhatprovides();

    Ok(())
}
