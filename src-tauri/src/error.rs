use serde::Serialize;
use std::{error::Error, fmt};

pub type AppResult<T> = Result<T, AppError>;

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "message")]
pub enum AppError {
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl Error for AppError {}
