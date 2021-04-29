mod solv;
mod actions;
mod cli;

use std::path::PathBuf;

use solv::{Pool, Solver, Queue, PackageMeta};
use libsolv_sys::ffi::{SOLVER_INSTALL, SOLVER_FLAG_BEST_OBEY_POLICY, SOLVER_FLAG_ALLOW_UNINSTALL};

fn main() -> anyhow::Result<()> {
    let package_path: Vec<PathBuf> = vec!(PathBuf::from("/tmp/apm/Packages-all"),
                                          PathBuf::from("/tmp/apm/Packages-amd64"));

    // Create new pool
    let mut pool = Pool::new();

    // Populate it with some Package from repository
    solv::populate_pool(&mut pool, &package_path)?;

    // Try to solve something
    // Prepare pool
    let queue = Queue::new();
    let mut result_queue = pool.match_package("samba", queue)?;
    result_queue.mark_all_as(SOLVER_INSTALL as i32);
    // Create transaction
    let mut solver = Solver::new(&pool);
    solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
    solver.solve(&mut result_queue).unwrap();
    let transaction = solver.create_transaction().unwrap();
    // Order transaction
    transaction.order(0); // We don't need special order (for now)

    println!("Transaction size change: {}", transaction.get_size_change());
    println!("Transaction detail: {:#?}", transaction.create_metadata().unwrap());

    Ok(())
}
