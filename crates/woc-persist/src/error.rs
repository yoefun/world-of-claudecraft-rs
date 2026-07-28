//! Persist errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("username already taken")]
    UsernameTaken,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("unauthorized")]
    Unauthorized,
    #[error("character not found")]
    CharacterNotFound,
    #[error("character name already taken")]
    CharacterNameTaken,
    #[error("forbidden")]
    Forbidden,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("password hash: {0}")]
    Password(String),
    #[cfg(feature = "postgres")]
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Other(String),
}

pub type PersistResult<T> = Result<T, PersistError>;
