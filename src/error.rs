use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Argument,
    Auth,
    Network,
    ApiData,
    Local,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AppError {
    kind: ErrorKind,
    message: String,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

pub type AppResult<T> = Result<T, AppError>;
