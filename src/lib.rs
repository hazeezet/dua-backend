pub mod util;
#[cfg(feature = "ssm")]
pub mod ssm;
#[cfg(feature = "gemini")]
pub mod gemini;
#[cfg(feature = "tts")]
pub mod tts;

use serde::{Deserialize, Serialize};

// ─── Standardized API response ──────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub status_code: u16,
    pub error: Option<String>,
    pub message: String,
    pub data: Option<T>,
}

// ─── Error codes ────────────────────────────────────────────────────────────

pub enum ErrorCode {
    BadRequest,
    UnknownError,
}

impl ErrorCode {
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::BadRequest => "Bad request, please check your input",
            ErrorCode::UnknownError => "An unexpected error occurred",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ErrorCode::BadRequest => "BAD_REQUEST",
            ErrorCode::UnknownError => "UNKNOWN_ERROR",
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            ErrorCode::BadRequest => 400,
            ErrorCode::UnknownError => 500,
        }
    }
}

// ─── Response error ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub status_code: u16,
    pub code: String,
    pub message: String,
}

impl ResponseError {
    pub fn new(error: ErrorCode, custom_message: Option<&str>) -> Self {
        Self {
            message: custom_message.unwrap_or(error.message()).to_string(),
            code: error.code().to_string(),
            status_code: error.status_code(),
        }
    }
}
