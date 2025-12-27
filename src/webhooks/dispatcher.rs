use chrono::{Duration, Utc};
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::models::{TransactionStatus, WebhookDeliveryStatus};
use crate::services::WebhookService;
use crate::webhooks::signer::sign_payload;

pub struct WebhookDispatcher {
    pool: Arc<PgPool>,
    client: Client,
    max_retries: u32,
}

impl WebhookDispatcher {
    pub fn new(pool: Arc<PgPool>, config: &Config) -> Self {
        let client = Client::builder()
            .timeout(StdDuration::from_secs(config.webhook_timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            pool,
            client,
            max_retries: config.webhook_max_retries,
        }
    }

    /// Start the background webhook delivery loop
    pub async fn start(self: Arc<Self>) {
        info!("Starting webhook dispatcher");
        
        let mut ticker = interval(StdDuration::from_secs(5));
        
        loop {
            ticker.tick().await;
            
            if let Err(e) = self.process_pending_deliveries().await {
                error!("Error processing webhook deliveries: {:?}", e);
            }
        }
    }

    /// Process pending webhook deliveries
    async fn process_pending_deliveries(&self) -> Result<(), sqlx::Error> {
        let deliveries = WebhookService::get_pending_deliveries(&self.pool, 10).await?;
        
        for (delivery, webhook, transaction) in deliveries {
            let event = match transaction.status {
                TransactionStatus::Completed => "transaction.completed",
                TransactionStatus::Failed => "transaction.failed",
                _ => continue,
            };
            
            // Build payload
            let payload = WebhookService::build_payload(&transaction, event);
            let payload_json = serde_json::to_string(&payload)
                .expect("Failed to serialize webhook payload");
            
            // Sign the payload
            let signature = sign_payload(payload_json.as_bytes(), &webhook.secret);
            
            // Send the webhook
            let result = self
                .client
                .post(&webhook.url)
                .header("Content-Type", "application/json")
                .header("X-Webhook-Signature", &signature)
                .header("X-Webhook-Id", delivery.id.to_string())
                .body(payload_json.clone())
                .send()
                .await;
            
            match result {
                Ok(response) => {
                    let status_code = response.status().as_u16() as i32;
                    
                    if response.status().is_success() {
                        info!(
                            webhook_id = %webhook.id,
                            delivery_id = %delivery.id,
                            status_code = status_code,
                            "Webhook delivered successfully"
                        );
                        
                        WebhookService::update_delivery_status(
                            &self.pool,
                            delivery.id,
                            WebhookDeliveryStatus::Success,
                            Some(status_code),
                            None,
                            None,
                        )
                        .await?;
                    } else {
                        let error_msg = format!("HTTP {}", status_code);
                        self.handle_failure(delivery.id, delivery.retry_count, Some(status_code), &error_msg)
                            .await?;
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    warn!(
                        webhook_id = %webhook.id,
                        delivery_id = %delivery.id,
                        error = %error_msg,
                        "Webhook delivery failed"
                    );
                    
                    self.handle_failure(delivery.id, delivery.retry_count, None, &error_msg)
                        .await?;
                }
            }
        }
        
        Ok(())
    }

    /// Handle a failed delivery - schedule retry or mark as permanently failed
    async fn handle_failure(
        &self,
        delivery_id: uuid::Uuid,
        retry_count: i32,
        response_code: Option<i32>,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        let new_retry_count = retry_count + 1;
        
        if new_retry_count >= self.max_retries as i32 {
            // Max retries reached, mark as failed
            warn!(
                delivery_id = %delivery_id,
                retry_count = new_retry_count,
                "Webhook delivery permanently failed after max retries"
            );
            
            WebhookService::update_delivery_status(
                &self.pool,
                delivery_id,
                WebhookDeliveryStatus::Failed,
                response_code,
                Some(error),
                None,
            )
            .await?;
        } else {
            // Schedule retry with exponential backoff
            let backoff_seconds = 2i64.pow(new_retry_count as u32).min(3600);
            let next_retry = Utc::now() + Duration::seconds(backoff_seconds);
            
            info!(
                delivery_id = %delivery_id,
                retry_count = new_retry_count,
                next_retry = %next_retry,
                "Scheduling webhook retry"
            );
            
            WebhookService::update_delivery_status(
                &self.pool,
                delivery_id,
                WebhookDeliveryStatus::Pending,
                response_code,
                Some(error),
                Some(next_retry),
            )
            .await?;
        }
        
        Ok(())
    }
}
