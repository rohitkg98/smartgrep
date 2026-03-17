use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Validation(String),
    NotFound(String),
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

pub type Result<T> = std::result::Result<T, AppError>;

pub fn wrap_error(msg: &str, inner: AppError) -> AppError {
    AppError::Internal(format!("{}: {}", msg, inner))
}
