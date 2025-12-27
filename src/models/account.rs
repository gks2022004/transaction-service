use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: Uuid,
    pub api_key_id: Uuid,
    pub name: String,
    pub balance_cents: i64,
    pub currency: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAccount {
    #[validate(length(min = 1, max = 255, message = "Name must be between 1 and 255 characters"))]
    pub name: String,
    #[validate(length(equal = 3, message = "Currency must be a 3-letter ISO code"))]
    #[serde(default = "default_currency")]
    pub currency: String,
}

fn default_currency() -> String {
    "USD".to_string()
}

#[derive(Debug, Serialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub name: String,
    pub balance_cents: i64,
    pub balance_formatted: String,
    pub currency: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Account> for AccountResponse {
    fn from(account: Account) -> Self {
        let balance_formatted = format!(
            "{:.2} {}",
            account.balance_cents as f64 / 100.0,
            account.currency
        );
        Self {
            id: account.id,
            name: account.name,
            balance_cents: account.balance_cents,
            balance_formatted,
            currency: account.currency,
            is_active: account.is_active,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub account_id: Uuid,
    pub balance_cents: i64,
    pub balance_formatted: String,
    pub currency: String,
    pub as_of: DateTime<Utc>,
}

impl From<Account> for BalanceResponse {
    fn from(account: Account) -> Self {
        let balance_formatted = format!(
            "{:.2} {}",
            account.balance_cents as f64 / 100.0,
            account.currency
        );
        Self {
            account_id: account.id,
            balance_cents: account.balance_cents,
            balance_formatted,
            currency: account.currency,
            as_of: Utc::now(),
        }
    }
}
