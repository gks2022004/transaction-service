use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "transaction_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Credit,
    Debit,
    Transfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "transaction_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub idempotency_key: Option<Uuid>,
    pub api_key_id: Uuid,
    pub transaction_type: TransactionType,
    pub source_account_id: Option<Uuid>,
    pub destination_account_id: Option<Uuid>,
    pub amount_cents: i64,
    pub status: TransactionStatus,
    pub metadata: serde_json::Value,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTransaction {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub source_account_id: Option<Uuid>,
    pub destination_account_id: Option<Uuid>,
    #[validate(range(min = 1, message = "Amount must be greater than 0"))]
    pub amount_cents: i64,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl CreateTransaction {
    pub fn validate_accounts(&self) -> Result<(), &'static str> {
        match self.transaction_type {
            TransactionType::Credit => {
                if self.destination_account_id.is_none() {
                    return Err("destination_account_id is required for credit transactions");
                }
            }
            TransactionType::Debit => {
                if self.source_account_id.is_none() {
                    return Err("source_account_id is required for debit transactions");
                }
            }
            TransactionType::Transfer => {
                if self.source_account_id.is_none() || self.destination_account_id.is_none() {
                    return Err("Both source_account_id and destination_account_id are required for transfer transactions");
                }
                if self.source_account_id == self.destination_account_id {
                    return Err("source_account_id and destination_account_id cannot be the same");
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub id: Uuid,
    pub idempotency_key: Option<Uuid>,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub source_account_id: Option<Uuid>,
    pub destination_account_id: Option<Uuid>,
    pub amount_cents: i64,
    pub amount_formatted: String,
    pub status: TransactionStatus,
    pub metadata: serde_json::Value,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<Transaction> for TransactionResponse {
    fn from(tx: Transaction) -> Self {
        Self {
            id: tx.id,
            idempotency_key: tx.idempotency_key,
            transaction_type: tx.transaction_type,
            source_account_id: tx.source_account_id,
            destination_account_id: tx.destination_account_id,
            amount_cents: tx.amount_cents,
            amount_formatted: format!("{:.2}", tx.amount_cents as f64 / 100.0),
            status: tx.status,
            metadata: tx.metadata,
            error_message: tx.error_message,
            created_at: tx.created_at,
        }
    }
}
