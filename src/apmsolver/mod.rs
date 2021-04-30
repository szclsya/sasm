use libsolv_sys::ffi::{
    SOLVER_ALL, SOLVER_ERASE, SOLVER_FLAG_ALLOW_UNINSTALL, SOLVER_FLAG_BEST_OBEY_POLICY, SOLVER_INSTALL,
    SOLVER_NOOP, SOLVER_UPDATE, SOLVER_DISTUPGRADE
};
use resolver::solv::{PackageMeta, Pool, Queue, Solver};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum SolveError {
    Unsolvable(String),
    DatabaseInitError(String),
    InternalError(String),
}

impl From<anyhow::Error> for SolveError {
    fn from(e: anyhow::Error) -> Self {
        SolveError::InternalError(e.to_string())
    }
}

pub struct ApmSolver {
    pool: Pool,
}

impl ApmSolver {
    pub fn new(db_files: &[PathBuf]) -> Result<Self, SolveError> {
        // Create new pool
        let mut pool = Pool::new();
        // Populate it with some Package from repository
        resolver::solv::populate_pool(&mut pool, db_files)
            .map_err(|e| SolveError::DatabaseInitError(e.to_string()))?;

        Ok(ApmSolver { pool })
    }

    pub fn install(&self, to_install: &[String]) -> Result<Vec<PackageMeta>, SolveError> {
        let mut queue = Queue::new();
        // Add desired packages to main queue
        for pkg_name in to_install {
            let mut temp_queue = Queue::new();
            temp_queue = self.pool.match_package(&pkg_name, temp_queue)?;
            // Append temp_queue to main queue
            queue.extend(&temp_queue)
        }
        // Mark all queue elements to be installed
        queue.mark_all_as(SOLVER_INSTALL as i32);

        // Create transaction
        let mut solver = Solver::new(&self.pool);
        solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
        solver
            .solve(&mut queue)
            .map_err(|e| SolveError::Unsolvable(e.to_string()))?;
        let transaction = solver.create_transaction()?;
        // Order transaction
        transaction.order(0); // We don't need special order (for now)

        Ok(transaction.create_metadata()?)
    }

    pub fn remove(&self, to_remove: &[String]) -> Result<Vec<PackageMeta>, SolveError> {
        let mut queue = Queue::new();
        // Add desired packages to main queue
        for pkg_name in to_remove {
            let mut temp_queue = Queue::new();
            temp_queue = self.pool.match_package(&pkg_name, temp_queue)?;
            // Append temp_queue to main queue
            queue.extend(&temp_queue)
        }
        // Mark all queue elements to be removed 
        queue.mark_all_as(SOLVER_ERASE as i32);

        // Create transaction
        let mut solver = Solver::new(&self.pool);
        solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
        solver.set_flag(SOLVER_FLAG_ALLOW_UNINSTALL as i32, 1)?;
        solver
            .solve(&mut queue)
            .map_err(|e| SolveError::Unsolvable(e.to_string()))?;
        let transaction = solver.create_transaction()?;
        // Order transaction
        transaction.order(0); // We don't need special order (for now)

        Ok(transaction.create_metadata()?)
    }

    pub fn upgrade(&self) -> Result<Vec<PackageMeta>, SolveError> {
        let mut queue = Queue::new();
        // Mark that we want an upgrade
        queue.push2(SOLVER_UPDATE as i32 | SOLVER_ALL, 0);

        // Create transaction
        let mut solver = Solver::new(&self.pool);
        solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
        solver
            .solve(&mut queue)
            .map_err(|e| SolveError::Unsolvable(e.to_string()))?;
        let transaction = solver.create_transaction()?;
        // Order transaction
        transaction.order(0); // We don't need special order (for now)

        Ok(transaction.create_metadata()?)
    }

    pub fn dist_upgrade(&self) -> Result<Vec<PackageMeta>, SolveError> {
        let mut queue = Queue::new();
         // Mark that we want a dist-upgrade
        queue.push2(SOLVER_DISTUPGRADE as i32 | SOLVER_ALL, 0);

         // Create transaction
        let mut solver = Solver::new(&self.pool);
        solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
        // Since dist-upgrade, uninstall is allowed
        solver.set_flag(SOLVER_FLAG_ALLOW_UNINSTALL as i32, 1)?;
        solver
            .solve(&mut queue)
            .map_err(|e| SolveError::Unsolvable(e.to_string()))?;
        let transaction = solver.create_transaction()?;
        // Order transaction
        transaction.order(0); // We don't need special order (for now)

        Ok(transaction.create_metadata()?)      
    }
}
