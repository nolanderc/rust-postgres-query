use crate::execute;
use thiserror::Error;

/// Any error that this crate may produce.
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to execute the query")]
    Execute(#[from] execute::Error),

    #[error("failed to start new transaction")]
    BeginTransaction(#[source] tokio_postgres::Error),

    #[error("failed to parse query: {0}")]
    Parse(#[from] ParseError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("failed to find binding matching `${binding}`")]
    UndefinedBinding { binding: String },

    #[error(
        "expected an identifier, found '{next}'. Dollar signs may be escaped: `$$`.", 
        next = found.map(|ch| ch.to_string()).unwrap_or_else(|| "EOF".to_owned())
    )]
    EmptyIdentifier { found: Option<char> },
}
