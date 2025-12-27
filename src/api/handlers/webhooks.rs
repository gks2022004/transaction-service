use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::api::errors::ApiError;
use crate::api::middleware::ApiKeyAuth;
use crate::models::{CreateWebhook, WebhookResponse};
use crate::services::WebhookService;

#[derive(Debug, Serialize)]
pub struct WebhookWithSecretResponse {
    pub id: Uuid,
    pub url: String,
    pub secret: String,  // Only shown on creation
    pub events: Vec<String>,
    pub is_active: bool,
}

/// POST /api/v1/webhooks - Register a new webhook
pub async fn create_webhook(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Json(request): Json<CreateWebhook>,
) -> Result<(StatusCode, Json<WebhookWithSecretResponse>), ApiError> {
    // Validate request
    request.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    let webhook = WebhookService::create(&pool, auth.api_key.id, request).await?;

    Ok((
        StatusCode::CREATED,
        Json(WebhookWithSecretResponse {
            id: webhook.id,
            url: webhook.url,
            secret: webhook.secret,
            events: webhook.events,
            is_active: webhook.is_active,
        }),
    ))
}

/// GET /api/v1/webhooks - List all webhooks
pub async fn list_webhooks(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
) -> Result<Json<Vec<WebhookResponse>>, ApiError> {
    let webhooks = WebhookService::list(&pool, auth.api_key.id).await?;

    let data: Vec<WebhookResponse> = webhooks.into_iter().map(|w| w.into()).collect();

    Ok(Json(data))
}

/// GET /api/v1/webhooks/:id - Get a webhook
pub async fn get_webhook(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Path(webhook_id): Path<Uuid>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let webhook = WebhookService::get_by_id(&pool, auth.api_key.id, webhook_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Webhook {} not found", webhook_id)))?;

    Ok(Json(WebhookResponse::from(webhook)))
}

/// DELETE /api/v1/webhooks/:id - Delete a webhook
pub async fn delete_webhook(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Path(webhook_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = WebhookService::delete(&pool, auth.api_key.id, webhook_id).await?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("Webhook {} not found", webhook_id)))
    }
}
