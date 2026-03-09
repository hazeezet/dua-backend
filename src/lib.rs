pub mod util;
pub mod database;
#[cfg(feature = "ssm")]
pub mod ssm;
#[cfg(feature = "encryption")]
pub mod encryption;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub status_code: u16,
    pub error: Option<String>,
    pub message: String,
    pub data: Option<T>,
}

pub enum ErrorCode {
    SessionError,
    SessionExpired,
    UserNotFound,
    BadRequest,
    InvalidCredentials,
    AccountDisabled,
    UnknownError,
}

impl ErrorCode {
    /// Get the corresponding error message for the error code
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::SessionExpired => "Your session has expired. Please log in again",
            ErrorCode::SessionError => "Unable to validate your request",
            ErrorCode::UserNotFound => "User not found",
            ErrorCode::BadRequest => "Bad request, please check your input",
            ErrorCode::InvalidCredentials => "Invalid credentials provided",
            ErrorCode::AccountDisabled => "Your account has been disabled",
            ErrorCode::UnknownError => "An unexpected error occurred",
        }
    }

    /// Get the error code as a string
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCode::SessionError => "SESSION_ERROR",
            ErrorCode::SessionExpired => "SESSION_EXPIRED",
            ErrorCode::UserNotFound => "USER_NOT_FOUND",
            ErrorCode::BadRequest => "BAD_REQUEST",
            ErrorCode::InvalidCredentials => "INVALID_CREDENTIALS",
            ErrorCode::AccountDisabled => "ACCOUNT_DISABLED",
            ErrorCode::UnknownError => "UNKNOWN_ERROR",
        }
    }

    /// Get status code associated with the error
    pub fn status_code(&self) -> u16 {
        match self {
            ErrorCode::SessionError => 401,
            ErrorCode::SessionExpired => 401,
            ErrorCode::UserNotFound => 404,
            ErrorCode::BadRequest => 400,
            ErrorCode::InvalidCredentials => 400,
            ErrorCode::AccountDisabled => 403,
            ErrorCode::UnknownError => 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub status_code: u16,
    pub code: String,
    pub message: String,
}

impl ResponseError {
    /// Create AuthError with custom message
    pub fn new(error: ErrorCode, custom_message: Option<&str>) -> Self {
        Self {
            message: custom_message.unwrap_or(error.message()).to_string(),
            code: error.code().to_string(),
            status_code: error.status_code(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AccountStatus {
    Active,
    Disabled,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub image_url: Option<String>,
    pub email: String,
    pub password: String,
    pub is_work_email: Option<bool>,
    pub bio: Option<String>,
    pub status: AccountStatus,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}
pub struct Location {
    pub country: String,
    pub state: String,
    pub city: String,
    pub coordinates: Vec<f64>,
}
