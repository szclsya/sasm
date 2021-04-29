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
    pub path: String,
    pub action: PackageAction,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub name: Option<String>,
    pub flags: c_int,
}

/// Simulate the apt dependency resolution
pub fn calculate_deps(pool: &mut Pool, tasks: &[Task]) -> Result<Transaction> {
    let mut tmp = Queue::new();
    let mut q = Queue::new();
    for task in tasks {
        if let Some(name) = &task.name {
            tmp = pool.match_package(&name, tmp)?;
            q.extend(&tmp);
            q.mark_all_as(task.flags);
            continue;
        }
        q.push2(task.flags, 0);
    }
    let mut solver = Solver::new(pool);
    solver.set_flag(SOLVER_FLAG_ALLOW_UNINSTALL as c_int, 1)?;
    solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY, 1)?;
    solver.solve(&mut q)?;
    let trans = solver.create_transaction()?;
    trans.order(0);

    Ok(trans)
}

/// Populate the packages pool with metadata
pub fn populate_pool(pool: &mut Pool, paths: &[PathBuf]) -> Result<()> {
    let mut repo = Repo::new(pool, "stable")?;
    for path in paths {
        repo.add_debpackages(path)?;
    }
    let mut system = Repo::new(pool, "@System")?;
    //system.add_debdb()?;
    //pool.set_installed(&system);

    pool.createwhatprovides();

    Ok(())
}
