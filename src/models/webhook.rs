use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "webhook_delivery_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum WebhookDeliveryStatus {
    Pending,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Webhook {
    pub id: Uuid,
    pub api_key_id: Uuid,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateWebhook {
    #[validate(url(message = "Invalid URL format"))]
    #[validate(length(max = 2048, message = "URL must be at most 2048 characters"))]
    pub url: String,
    #[serde(default = "default_events")]
    pub events: Vec<String>,
}

fn default_events() -> Vec<String> {
    vec![
        "transaction.completed".to_string(),
        "transaction.failed".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub transaction_id: Uuid,
    pub status: WebhookDeliveryStatus,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub last_response_code: Option<i32>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Webhook> for WebhookResponse {
    fn from(webhook: Webhook) -> Self {
        Self {
            id: webhook.id,
            url: webhook.url,
            events: webhook.events,
            is_active: webhook.is_active,
            created_at: webhook.created_at,
        }
    }
}

/// Payload sent to webhook endpoints
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub id: Uuid,
    pub event: String,
    pub created_at: DateTime<Utc>,
    pub data: WebhookTransactionData,
}

#[derive(Debug, Serialize)]
pub struct WebhookTransactionData {
    pub transaction_id: Uuid,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub source_account_id: Option<Uuid>,
    pub destination_account_id: Option<Uuid>,
    pub amount_cents: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
}
