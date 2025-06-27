use std::fmt;

#[derive(Debug)]
pub enum Error {
    Database(sqlx::Error),
    Io(std::io::Error),
    ProcessNotFound(String),
    ProcessAlreadyExists(String),
    InvalidProcessState(String),
    SerializationError(serde_json::Error),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Database(e) => write!(f, "Database error: {}", e),
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::ProcessNotFound(name) => write!(f, "Process '{}' not found", name),
            Error::ProcessAlreadyExists(name) => write!(f, "Process '{}' already exists", name),
            Error::InvalidProcessState(msg) => write!(f, "Invalid process state: {}", msg),
            Error::SerializationError(e) => write!(f, "Serialization error: {}", e),
            Error::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Error::Database(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::SerializationError(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
