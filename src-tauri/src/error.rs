//! Typed errors for the Tauri command boundary.
//!
//! The UI historically received a plain string. `AppError` keeps that contract
//! (it serializes to its message) while letting Rust code use `?` and explicit
//! error sources instead of stringly-typed plumbing.

use serde::ser::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("Операция уже выполняется.")]
    Busy,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_keeps_the_ui_contract() {
        let error = AppError::from("не удалось".to_string());
        assert_eq!(error.to_string(), "не удалось");
    }

    #[test]
    fn io_error_is_wrapped() {
        let error = AppError::from(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        assert!(error.to_string().contains("missing"));
    }
}
