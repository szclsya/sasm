mod pool;
mod sat;
mod types;
mod version;

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

/*
pub struct ApmSolver {
}

impl ApmSolver {
    pub fn new(db_files: &[PathBuf]) -> Result<Self, SolveError> {
        todo!()
    }

    pub fn install(&self, to_install: &[String]) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }

    pub fn remove(&self, to_remove: &[String]) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }

    pub fn upgrade(&self) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }

    pub fn dist_upgrade(&self) -> Result<Vec<PackageMeta>, SolveError> {
        todo!()
    }
}
*/
