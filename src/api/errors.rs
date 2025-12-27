use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Access forbidden")]
    Forbidden,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),

    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),

    #[error("Database error")]
    Database(#[from] sqlx::Error),
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                self.to_string(),
            ),
            ApiError::InvalidApiKey => (
                StatusCode::UNAUTHORIZED,
                "invalid_api_key",
                self.to_string(),
            ),
            ApiError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                self.to_string(),
            ),
            ApiError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "not_found",
                msg.clone(),
            ),
            ApiError::Validation(msg) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                msg.clone(),
            ),
            ApiError::Conflict(msg) => (
                StatusCode::CONFLICT,
                "conflict",
                msg.clone(),
            ),
            ApiError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_exceeded",
                self.to_string(),
            ),
            ApiError::InsufficientBalance(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "insufficient_balance",
                msg.clone(),
            ),
            ApiError::Internal(e) => {
                tracing::error!("Internal error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An internal error occurred".to_string(),
                )
            }
            ApiError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "A database error occurred".to_string(),
                )
            }
        };

        let body = Json(ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message,
                details: None,
            },
        });

        (status, body).into_response()
    }
}

impl From<crate::services::transaction_service::TransactionError> for ApiError {
    fn from(err: crate::services::transaction_service::TransactionError) -> Self {
        match err {
            crate::services::transaction_service::TransactionError::AccountNotFound(id) => {
                ApiError::NotFound(format!("Account {} not found", id))
            }
            crate::services::transaction_service::TransactionError::InsufficientBalance(
                id,
                balance,
                required,
            ) => ApiError::InsufficientBalance(format!(
                "Account {} has {} cents, but {} cents required",
                id, balance, required
            )),
            crate::services::transaction_service::TransactionError::AccountAccessDenied => {
                ApiError::Forbidden
            }
            crate::services::transaction_service::TransactionError::DuplicateIdempotencyKey(_) => {
                ApiError::Conflict("Duplicate idempotency key".to_string())
            }
            crate::services::transaction_service::TransactionError::Database(e) => {
                ApiError::Database(e)
            }
        }
    }
}
