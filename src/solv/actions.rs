use super::{ Pool, PackageMeta };
use std::path::PathBuf;

enum SolveError {
}

struct ApmSolver {
    pool: Pool,
}

impl ApmSolver {
    pub fn new(db_files: &[PathBuf]) -> Result<Self, SolveError> {
        // Create new pool
        let mut pool = Pool::new();
        // Populate it with some Package from repository
        super::populate_pool(&mut pool, db_files).unwrap();

        Ok(ApmSolver {
            pool,
        })
    }

    pub fn install(&self, to_install: &[String]) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }

    pub fn remove(&self, to_remove: &[String]) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }

    pub fn reinstall(&self, to_reinstall: &[String]) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }

    pub fn upgrade(&self, allow_remove: bool) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }
}
