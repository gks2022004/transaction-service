use crate::models::{
    CreateWebhook, Transaction, Webhook, WebhookDelivery, WebhookDeliveryStatus,
    WebhookPayload, WebhookTransactionData,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

pub struct WebhookService;

impl WebhookService {
    /// Create a new webhook
    pub async fn create(
        pool: &PgPool,
        api_key_id: Uuid,
        request: CreateWebhook,
    ) -> Result<Webhook, sqlx::Error> {
        // Generate a random secret for HMAC signing
        let secret_bytes: [u8; 32] = rand::random();
        let secret = hex::encode(secret_bytes);

        let webhook = sqlx::query_as::<_, Webhook>(
            r#"
            INSERT INTO webhooks (api_key_id, url, secret, events)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(api_key_id)
        .bind(&request.url)
        .bind(&secret)
        .bind(&request.events)
        .fetch_one(pool)
        .await?;

        Ok(webhook)
    }

    /// Get a webhook by ID
    pub async fn get_by_id(
        pool: &PgPool,
        api_key_id: Uuid,
        webhook_id: Uuid,
    ) -> Result<Option<Webhook>, sqlx::Error> {
        let webhook = sqlx::query_as::<_, Webhook>(
            r#"
            SELECT * FROM webhooks 
            WHERE id = $1 AND api_key_id = $2 AND is_active = TRUE
            "#,
        )
        .bind(webhook_id)
        .bind(api_key_id)
        .fetch_optional(pool)
        .await?;

        Ok(webhook)
    }

    /// List all webhooks for an API key
    pub async fn list(
        pool: &PgPool,
        api_key_id: Uuid,
    ) -> Result<Vec<Webhook>, sqlx::Error> {
        let webhooks = sqlx::query_as::<_, Webhook>(
            r#"
            SELECT * FROM webhooks 
            WHERE api_key_id = $1 AND is_active = TRUE
            ORDER BY created_at DESC
            "#,
        )
        .bind(api_key_id)
        .fetch_all(pool)
        .await?;

        Ok(webhooks)
    }

    /// Delete a webhook (deactivate)
    pub async fn delete(
        pool: &PgPool,
        api_key_id: Uuid,
        webhook_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE webhooks 
            SET is_active = FALSE
            WHERE id = $1 AND api_key_id = $2
            "#,
        )
        .bind(webhook_id)
        .bind(api_key_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Queue webhook deliveries for a completed transaction
    pub async fn queue_deliveries(
        pool: &PgPool,
        transaction: &Transaction,
    ) -> Result<Vec<WebhookDelivery>, sqlx::Error> {
        let event = match transaction.status {
            crate::models::TransactionStatus::Completed => "transaction.completed",
            crate::models::TransactionStatus::Failed => "transaction.failed",
            _ => return Ok(vec![]),
        };

        // Find all active webhooks that subscribe to this event
        let webhooks = sqlx::query_as::<_, Webhook>(
            r#"
            SELECT * FROM webhooks 
            WHERE api_key_id = $1 
              AND is_active = TRUE 
              AND $2 = ANY(events)
            "#,
        )
        .bind(transaction.api_key_id)
        .bind(event)
        .fetch_all(pool)
        .await?;

        let mut deliveries = Vec::new();

        for webhook in webhooks {
            let delivery = sqlx::query_as::<_, WebhookDelivery>(
                r#"
                INSERT INTO webhook_deliveries (webhook_id, transaction_id, status, next_retry_at)
                VALUES ($1, $2, $3, $4)
                RETURNING *
                "#,
            )
            .bind(webhook.id)
            .bind(transaction.id)
            .bind(WebhookDeliveryStatus::Pending)
            .bind(Utc::now())
            .fetch_one(pool)
            .await?;

            deliveries.push(delivery);
        }

        Ok(deliveries)
    }

    /// Get pending webhook deliveries that are ready for retry
    pub async fn get_pending_deliveries(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<(WebhookDelivery, Webhook, Transaction)>, sqlx::Error> {
        // Fetch pending deliveries
        let deliveries = sqlx::query_as::<_, WebhookDelivery>(
            r#"
            SELECT * FROM webhook_deliveries
            WHERE status = 'pending'
              AND next_retry_at <= NOW()
            ORDER BY next_retry_at
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let mut result = Vec::new();
        for delivery in deliveries {
            // Fetch related webhook and transaction
            let webhook = sqlx::query_as::<_, Webhook>(
                "SELECT * FROM webhooks WHERE id = $1 AND is_active = TRUE",
            )
            .bind(delivery.webhook_id)
            .fetch_optional(pool)
            .await?;

            let transaction = sqlx::query_as::<_, Transaction>(
                "SELECT * FROM transactions WHERE id = $1",
            )
            .bind(delivery.transaction_id)
            .fetch_optional(pool)
            .await?;

            if let (Some(webhook), Some(transaction)) = (webhook, transaction) {
                result.push((delivery, webhook, transaction));
            }
        }
        
        Ok(result)
    }

    /// Update webhook delivery status
    pub async fn update_delivery_status(
        pool: &PgPool,
        delivery_id: Uuid,
        status: WebhookDeliveryStatus,
        response_code: Option<i32>,
        error: Option<&str>,
        next_retry_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<WebhookDelivery, sqlx::Error> {
        let delivered_at = if status == WebhookDeliveryStatus::Success {
            Some(Utc::now())
        } else {
            None
        };

        let delivery = sqlx::query_as::<_, WebhookDelivery>(
            r#"
            UPDATE webhook_deliveries 
            SET status = $1,
                last_response_code = $2,
                last_error = $3,
                next_retry_at = $4,
                retry_count = retry_count + 1,
                delivered_at = COALESCE($5, delivered_at)
            WHERE id = $6
            RETURNING *
            "#,
        )
        .bind(status)
        .bind(response_code)
        .bind(error)
        .bind(next_retry_at)
        .bind(delivered_at)
        .bind(delivery_id)
        .fetch_one(pool)
        .await?;

        Ok(delivery)
    }

    /// Build webhook payload from transaction
    pub fn build_payload(transaction: &Transaction, event: &str) -> WebhookPayload {
        WebhookPayload {
            id: Uuid::new_v4(),
            event: event.to_string(),
            created_at: Utc::now(),
            data: WebhookTransactionData {
                transaction_id: transaction.id,
                transaction_type: format!("{:?}", transaction.transaction_type).to_lowercase(),
                source_account_id: transaction.source_account_id,
                destination_account_id: transaction.destination_account_id,
                amount_cents: transaction.amount_cents,
                status: format!("{:?}", transaction.status).to_lowercase(),
                created_at: transaction.created_at,
            },
        }
    }
}
