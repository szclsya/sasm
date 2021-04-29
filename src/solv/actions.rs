use super::{PackageMeta, Pool, Queue, Solver};
use libsolv_sys::ffi::{SOLVER_FLAG_ALLOW_UNINSTALL, SOLVER_FLAG_BEST_OBEY_POLICY, SOLVER_NOOP, SOLVER_INSTALL, SOLVER_ERASE, SOLVER_UPDATE};
use std::path::PathBuf;

#[derive(Clone, Debug)]
enum SolveError {
    Unsolvable(String),
    DatabaseInitError,
    InternalError(String),
}

impl From<anyhow::Error> for SolveError {
    fn from(e: anyhow::Error) -> Self {
        SolveError::InternalError(e.to_string())
    }
}

struct ApmSolver {
    pool: Pool,
}

impl ApmSolver {
    pub fn new(db_files: &[PathBuf]) -> Result<Self, SolveError> {
        // Create new pool
        let mut pool = Pool::new();
        // Populate it with some Package from repository
        super::populate_pool(&mut pool, db_files).map_err(|_| SolveError::DatabaseInitError)?;

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
        // Mark all queue elements as install
        queue.mark_all_as(SOLVER_INSTALL as i32);

        // Create transaction
        let mut solver = Solver::new(&self.pool);
        solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
        solver.solve(&mut queue)
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
        // Mark all queue elements as install
        queue.mark_all_as(SOLVER_ERASE as i32);

        // Create transaction
        let mut solver = Solver::new(&self.pool);
        solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
        solver.solve(&mut queue)
            .map_err(|e| SolveError::Unsolvable(e.to_string()))?;
        let transaction = solver.create_transaction()?;
        // Order transaction
        transaction.order(0); // We don't need special order (for now)

        Ok(transaction.create_metadata()?)
    }

    pub fn upgrade(&self, allow_remove: bool) -> Result<Vec<PackageMeta>, SolveError> {
        let mut queue = Queue::new();
        // Mark that we want an upgrade
        queue.push2(SOLVER_NOOP as i32, SOLVER_UPDATE as i32);

        // Create transaction
        let mut solver = Solver::new(&self.pool);
        solver.set_flag(SOLVER_FLAG_BEST_OBEY_POLICY as i32, 1)?;
        solver.solve(&mut queue)
            .map_err(|e| SolveError::Unsolvable(e.to_string()))?;
        let transaction = solver.create_transaction()?;
        // Order transaction
        transaction.order(0); // We don't need special order (for now)

        Ok(transaction.create_metadata()?)
    }
}
