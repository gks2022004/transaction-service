use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::ApiKey;

/// Extractor that validates the API key from the Authorization header
#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    pub api_key: ApiKey,
}

#[derive(Serialize)]
struct AuthErrorResponse {
    error: AuthError,
}

#[derive(Serialize)]
struct AuthError {
    code: String,
    message: String,
}

impl IntoResponse for AuthErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, Json(self)).into_response()
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for ApiKeyAuth
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get the pool from extensions
        let pool = parts
            .extensions
            .get::<Arc<PgPool>>()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthErrorResponse {
                        error: AuthError {
                            code: "internal_error".to_string(),
                            message: "Database pool not configured".to_string(),
                        },
                    }),
                )
                    .into_response()
            })?;

        // Extract the Authorization header
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                AuthErrorResponse {
                    error: AuthError {
                        code: "unauthorized".to_string(),
                        message: "Missing Authorization header".to_string(),
                    },
                }
                .into_response()
            })?;

        // Parse Bearer token
        let api_key_str = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| {
                AuthErrorResponse {
                    error: AuthError {
                        code: "invalid_auth_format".to_string(),
                        message: "Authorization header must use Bearer scheme".to_string(),
                    },
                }
                .into_response()
            })?;

        // Hash the provided key
        let key_hash = ApiKey::hash_key(api_key_str);

        // Look up the API key
        let api_key = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT * FROM api_keys 
            WHERE key_hash = $1 AND is_active = TRUE
            "#,
        )
        .bind(&key_hash)
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| {
            tracing::error!("Database error during auth: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: AuthError {
                        code: "internal_error".to_string(),
                        message: "Authentication failed".to_string(),
                    },
                }),
            )
                .into_response()
        })?
        .ok_or_else(|| {
            AuthErrorResponse {
                error: AuthError {
                    code: "invalid_api_key".to_string(),
                    message: "Invalid or inactive API key".to_string(),
                },
            }
            .into_response()
        })?;

        Ok(ApiKeyAuth { api_key })
    }
}

/// Get API key ID from an optional header (for public endpoints that optionally use auth)
pub fn extract_api_key_header(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}
