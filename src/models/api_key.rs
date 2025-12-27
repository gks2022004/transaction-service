use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub key_hash: String,
    pub key_prefix: String,
    pub business_name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKey {
    pub business_name: String,
}

impl ApiKey {
    /// Generate a new API key and return (raw_key, hashed_key, prefix)
    pub fn generate_key() -> (String, String, String) {
        let key_bytes: [u8; 32] = rand::random();
        let raw_key = format!("sk_live_{}", hex::encode(key_bytes));
        let prefix = raw_key.chars().take(8).collect();
        let hash = Self::hash_key(&raw_key);
        (raw_key, hash, prefix)
    }

    /// Hash an API key using SHA-256
    pub fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify if a given key matches this API key's hash
    pub fn verify_key(&self, key: &str) -> bool {
        Self::hash_key(key) == self.key_hash
    }
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: Uuid,
    pub key_prefix: String,
    pub business_name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<ApiKey> for ApiKeyResponse {
    fn from(key: ApiKey) -> Self {
        Self {
            id: key.id,
            key_prefix: key.key_prefix,
            business_name: key.business_name,
            is_active: key.is_active,
            created_at: key.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiKeyWithSecret {
    pub id: Uuid,
    pub key: String,  // The raw API key - only shown once
    pub key_prefix: String,
    pub business_name: String,
    pub created_at: DateTime<Utc>,
}
