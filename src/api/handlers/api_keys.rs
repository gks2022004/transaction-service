use axum::{
    extract::Extension,
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use std::sync::Arc;

use crate::api::errors::ApiError;
use crate::models::{ApiKey, ApiKeyWithSecret, CreateApiKey};

/// POST /api/v1/api-keys - Create a new API key (admin endpoint)
/// Note: In production, this should be protected by a separate admin authentication
pub async fn create_api_key(
    Extension(pool): Extension<Arc<PgPool>>,
    Json(request): Json<CreateApiKey>,
) -> Result<(StatusCode, Json<ApiKeyWithSecret>), ApiError> {
    // Generate new API key
    let (raw_key, key_hash, key_prefix) = ApiKey::generate_key();

    // Store in database
    let api_key = sqlx::query_as::<_, ApiKey>(
        r#"
        INSERT INTO api_keys (key_hash, key_prefix, business_name)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(&request.business_name)
    .fetch_one(pool.as_ref())
    .await?;

    // Return the raw key (only shown once)
    Ok((
        StatusCode::CREATED,
        Json(ApiKeyWithSecret {
            id: api_key.id,
            key: raw_key,
            key_prefix: api_key.key_prefix,
            business_name: api_key.business_name,
            created_at: api_key.created_at,
        }),
    ))
}
