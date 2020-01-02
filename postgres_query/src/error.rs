use crate::execute;
use thiserror::Error;

/// Any error that this crate may produce.
#[derive(Debug, Error)]
pub enum Error {
    /// A query failed to execute.
    #[error("failed to execute the query")]
    Execute(#[from] execute::Error),

    /// Failed to start a new transaction.
    #[error("failed to start new transaction")]
    BeginTransaction(#[source] tokio_postgres::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
