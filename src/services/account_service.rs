use crate::models::{Account, CreateAccount};
use sqlx::PgPool;
use uuid::Uuid;

pub struct AccountService;

impl AccountService {
    /// Create a new account for an API key
    pub async fn create(
        pool: &PgPool,
        api_key_id: Uuid,
        request: CreateAccount,
    ) -> Result<Account, sqlx::Error> {
        let account = sqlx::query_as::<_, Account>(
            r#"
            INSERT INTO accounts (api_key_id, name, currency, balance_cents)
            VALUES ($1, $2, $3, 0)
            RETURNING *
            "#,
        )
        .bind(api_key_id)
        .bind(&request.name)
        .bind(&request.currency)
        .fetch_one(pool)
        .await?;

        Ok(account)
    }

    /// Get an account by ID, ensuring it belongs to the given API key
    pub async fn get_by_id(
        pool: &PgPool,
        api_key_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<Account>, sqlx::Error> {
        let account = sqlx::query_as::<_, Account>(
            r#"
            SELECT * FROM accounts 
            WHERE id = $1 AND api_key_id = $2 AND is_active = TRUE
            "#,
        )
        .bind(account_id)
        .bind(api_key_id)
        .fetch_optional(pool)
        .await?;

        Ok(account)
    }

    /// Get an account by ID without API key check (for internal use)
    pub async fn get_by_id_internal(
        pool: &PgPool,
        account_id: Uuid,
    ) -> Result<Option<Account>, sqlx::Error> {
        let account = sqlx::query_as::<_, Account>(
            r#"
            SELECT * FROM accounts 
            WHERE id = $1 AND is_active = TRUE
            "#,
        )
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

        Ok(account)
    }

    /// List all accounts for an API key
    pub async fn list(
        pool: &PgPool,
        api_key_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Account>, sqlx::Error> {
        let accounts = sqlx::query_as::<_, Account>(
            r#"
            SELECT * FROM accounts 
            WHERE api_key_id = $1 AND is_active = TRUE
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(api_key_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(accounts)
    }

    /// Get total count of accounts for an API key
    pub async fn count(pool: &PgPool, api_key_id: Uuid) -> Result<i64, sqlx::Error> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM accounts 
            WHERE api_key_id = $1 AND is_active = TRUE
            "#,
        )
        .bind(api_key_id)
        .fetch_one(pool)
        .await?;

        Ok(count.0)
    }

    /// Deactivate an account (soft delete)
    pub async fn deactivate(
        pool: &PgPool,
        api_key_id: Uuid,
        account_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE accounts 
            SET is_active = FALSE
            WHERE id = $1 AND api_key_id = $2
            "#,
        )
        .bind(account_id)
        .bind(api_key_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
