use crate::models::{
    CreateTransaction, Transaction, TransactionStatus, TransactionType,
};
use sqlx::PgPool;
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("Account not found: {0}")]
    AccountNotFound(Uuid),

    #[error("Insufficient balance: account {0} has {1} cents, but {2} cents required")]
    InsufficientBalance(Uuid, i64, i64),

    #[error("Account does not belong to this API key")]
    AccountAccessDenied,

    #[error("Duplicate idempotency key")]
    DuplicateIdempotencyKey(Transaction),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct TransactionService;

impl TransactionService {
    /// Create a new transaction with atomic balance updates
    pub async fn create(
        pool: &PgPool,
        api_key_id: Uuid,
        request: CreateTransaction,
        idempotency_key: Option<Uuid>,
    ) -> Result<Transaction, TransactionError> {
        // Check for existing transaction with same idempotency key
        if let Some(idem_key) = idempotency_key {
            if let Some(existing) = Self::get_by_idempotency_key(pool, idem_key).await? {
                info!("Returning existing transaction for idempotency key: {}", idem_key);
                return Err(TransactionError::DuplicateIdempotencyKey(existing));
            }
        }

        // Start a database transaction for atomicity
        let mut tx = pool.begin().await?;

        // Validate accounts belong to the API key and exist
        if let Some(source_id) = request.source_account_id {
            let source = sqlx::query_as::<_, (Uuid, i64)>(
                "SELECT id, balance_cents FROM accounts WHERE id = $1 AND is_active = TRUE FOR UPDATE",
            )
            .bind(source_id)
            .fetch_optional(&mut *tx)
            .await?;

            match source {
                None => return Err(TransactionError::AccountNotFound(source_id)),
                Some((_, balance)) if balance < request.amount_cents => {
                    return Err(TransactionError::InsufficientBalance(
                        source_id,
                        balance,
                        request.amount_cents,
                    ));
                }
                _ => {}
            }

            // Verify account belongs to API key
            let belongs = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = $1 AND api_key_id = $2)",
            )
            .bind(source_id)
            .bind(api_key_id)
            .fetch_one(&mut *tx)
            .await?;

            if !belongs {
                return Err(TransactionError::AccountAccessDenied);
            }
        }

        if let Some(dest_id) = request.destination_account_id {
            let dest_exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = $1 AND is_active = TRUE)",
            )
            .bind(dest_id)
            .fetch_one(&mut *tx)
            .await?;

            if !dest_exists {
                return Err(TransactionError::AccountNotFound(dest_id));
            }

            // For credits and transfers, verify destination also belongs to API key
            // (or allow cross-business transfers by removing this check)
            let belongs = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = $1 AND api_key_id = $2)",
            )
            .bind(dest_id)
            .bind(api_key_id)
            .fetch_one(&mut *tx)
            .await?;

            if !belongs {
                return Err(TransactionError::AccountAccessDenied);
            }
        }

        // Create the transaction record
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            INSERT INTO transactions (
                idempotency_key, api_key_id, transaction_type, 
                source_account_id, destination_account_id, 
                amount_cents, status, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(idempotency_key)
        .bind(api_key_id)
        .bind(request.transaction_type)
        .bind(request.source_account_id)
        .bind(request.destination_account_id)
        .bind(request.amount_cents)
        .bind(TransactionStatus::Pending)
        .bind(&request.metadata)
        .fetch_one(&mut *tx)
        .await?;

        // Update balances based on transaction type
        match request.transaction_type {
            TransactionType::Credit => {
                sqlx::query(
                    "UPDATE accounts SET balance_cents = balance_cents + $1 WHERE id = $2",
                )
                .bind(request.amount_cents)
                .bind(request.destination_account_id.unwrap())
                .execute(&mut *tx)
                .await?;
            }
            TransactionType::Debit => {
                sqlx::query(
                    "UPDATE accounts SET balance_cents = balance_cents - $1 WHERE id = $2",
                )
                .bind(request.amount_cents)
                .bind(request.source_account_id.unwrap())
                .execute(&mut *tx)
                .await?;
            }
            TransactionType::Transfer => {
                // Debit from source
                sqlx::query(
                    "UPDATE accounts SET balance_cents = balance_cents - $1 WHERE id = $2",
                )
                .bind(request.amount_cents)
                .bind(request.source_account_id.unwrap())
                .execute(&mut *tx)
                .await?;

                // Credit to destination
                sqlx::query(
                    "UPDATE accounts SET balance_cents = balance_cents + $1 WHERE id = $2",
                )
                .bind(request.amount_cents)
                .bind(request.destination_account_id.unwrap())
                .execute(&mut *tx)
                .await?;
            }
        }

        // Mark transaction as completed
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            UPDATE transactions 
            SET status = $1 
            WHERE id = $2
            RETURNING *
            "#,
        )
        .bind(TransactionStatus::Completed)
        .bind(transaction.id)
        .fetch_one(&mut *tx)
        .await?;

        // Commit the database transaction
        tx.commit().await?;

        info!(
            transaction_id = %transaction.id,
            transaction_type = ?request.transaction_type,
            amount_cents = request.amount_cents,
            "Transaction completed successfully"
        );

        Ok(transaction)
    }

    /// Get a transaction by ID
    pub async fn get_by_id(
        pool: &PgPool,
        api_key_id: Uuid,
        transaction_id: Uuid,
    ) -> Result<Option<Transaction>, sqlx::Error> {
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT * FROM transactions 
            WHERE id = $1 AND api_key_id = $2
            "#,
        )
        .bind(transaction_id)
        .bind(api_key_id)
        .fetch_optional(pool)
        .await?;

        Ok(transaction)
    }

    /// Get a transaction by idempotency key
    pub async fn get_by_idempotency_key(
        pool: &PgPool,
        idempotency_key: Uuid,
    ) -> Result<Option<Transaction>, sqlx::Error> {
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT * FROM transactions 
            WHERE idempotency_key = $1
            "#,
        )
        .bind(idempotency_key)
        .fetch_optional(pool)
        .await?;

        Ok(transaction)
    }

    /// List transactions for an account
    pub async fn list_for_account(
        pool: &PgPool,
        api_key_id: Uuid,
        account_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>, sqlx::Error> {
        let transactions = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT t.* FROM transactions t
            WHERE t.api_key_id = $1 
              AND (t.source_account_id = $2 OR t.destination_account_id = $2)
            ORDER BY t.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(api_key_id)
        .bind(account_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(transactions)
    }

    /// Mark a transaction as failed
    pub async fn fail_transaction(
        pool: &PgPool,
        transaction_id: Uuid,
        error_message: &str,
    ) -> Result<Transaction, sqlx::Error> {
        warn!(
            transaction_id = %transaction_id,
            error = error_message,
            "Marking transaction as failed"
        );

        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            UPDATE transactions 
            SET status = $1, error_message = $2
            WHERE id = $3
            RETURNING *
            "#,
        )
        .bind(TransactionStatus::Failed)
        .bind(error_message)
        .bind(transaction_id)
        .fetch_one(pool)
        .await?;

        Ok(transaction)
    }
}
