use thiserror::Error;

#[derive(Error, Clone, Debug)]
pub enum ExecutionError {
    #[error("Failed to load state of this instance: {0}")]
    StateError(String),
}
