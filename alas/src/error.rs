use thiserror::Error;

use crate::messages::{print_message, MessageType};

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    IOError(#[from] ::std::io::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] ::serde_json::Error),
    #[error(transparent)]
    AnkiDbError(#[from] ::anki_db::Error),
    #[error(transparent)]
    DbError(#[from] ::rusqlite::Error),
    #[error("{0}")]
    ConfigError(String),
    #[error("{0}")]
    LatexError(String),
    #[error("{0}")]
    AlasError(String),
    #[error("{0}")]
    JobError(String),
}

pub fn handle_error(err: Error) {
    print_message(MessageType::Error, &err.to_string());
}
