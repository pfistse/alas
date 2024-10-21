// Code in this file is based on or derived from the Anki project.
// You can find the original code at https://github.com/ankitects/anki.

use rusqlite::types::FromSqlError;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Database(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Template(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Encode(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Decode(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    JSON(Box<dyn std::error::Error + Send + Sync>),
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Database(Box::new(e))
    }
}

impl From<prost::EncodeError> for Error {
    fn from(e: prost::EncodeError) -> Self {
        Error::Encode(Box::new(e))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Encode(Box::new(e))
    }
}

impl From<prost::DecodeError> for Error {
    fn from(e: prost::DecodeError) -> Self {
        Error::Decode(Box::new(e))
    }
}

impl From<FromSqlError> for Error {
    fn from(e: FromSqlError) -> Self {
        Error::Database(Box::new(e))
    }
}