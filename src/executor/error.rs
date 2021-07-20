use thiserror::Error;

#[derive(Error, Clone, Debug)]
pub enum ExecutionError {
    #[error("Failed to load state of this instance: {0}")]
    StateError(String),
    #[error("Error while fetching some resource: {0}")]
    ResourceFetchError(String),
    #[error("dpkg command exited with return code {0}")]
    DpkgError(i32),
    #[error("dpkg terminated by signal")]
    DpkgTerminated,
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<reqwest::Error> for ExecutionError {
    fn from(e: reqwest::Error) -> Self {
        ExecutionError::ResourceFetchError(e.to_string())
    }
}

impl From<std::io::Error> for ExecutionError {
    fn from(e: std::io::Error) -> Self {
        ExecutionError::ResourceFetchError(e.to_string())
    }
}
