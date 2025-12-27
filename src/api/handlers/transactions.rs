use axum::{
    extract::{Extension, Path, Query},
    http::{HeaderMap, StatusCode},
    Json,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::api::errors::ApiError;
use crate::api::handlers::accounts::{ListResponse, PaginationParams};
use crate::api::middleware::ApiKeyAuth;
use crate::models::{CreateTransaction, TransactionResponse};
use crate::services::{TransactionService, WebhookService};
use crate::services::transaction_service::TransactionError;

/// POST /api/v1/transactions - Create a new transaction
pub async fn create_transaction(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    headers: HeaderMap,
    Json(request): Json<CreateTransaction>,
) -> Result<(StatusCode, Json<TransactionResponse>), ApiError> {
    // Validate request
    request.validate().map_err(|e| ApiError::Validation(e.to_string()))?;
    request.validate_accounts().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Extract idempotency key from header if present
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok());

    // Create transaction
    let result = TransactionService::create(&pool, auth.api_key.id, request, idempotency_key).await;

    match result {
        Ok(transaction) => {
            // Queue webhook deliveries asynchronously
            let pool_clone = pool.clone();
            let tx_clone = transaction.clone();
            tokio::spawn(async move {
                if let Err(e) = WebhookService::queue_deliveries(&pool_clone, &tx_clone).await {
                    tracing::error!("Failed to queue webhook deliveries: {:?}", e);
                }
            });

            Ok((StatusCode::CREATED, Json(TransactionResponse::from(transaction))))
        }
        Err(TransactionError::DuplicateIdempotencyKey(existing)) => {
            // Return the existing transaction for idempotent requests
            Ok((StatusCode::OK, Json(TransactionResponse::from(existing))))
        }
        Err(e) => Err(e.into()),
    }
}

/// GET /api/v1/transactions/:id - Get a transaction
pub async fn get_transaction(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Path(transaction_id): Path<Uuid>,
) -> Result<Json<TransactionResponse>, ApiError> {
    let transaction = TransactionService::get_by_id(&pool, auth.api_key.id, transaction_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Transaction {} not found", transaction_id)))?;

    Ok(Json(TransactionResponse::from(transaction)))
}

/// GET /api/v1/accounts/:id/transactions - List transactions for an account
pub async fn list_account_transactions(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Path(account_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ListResponse<TransactionResponse>>, ApiError> {
    let limit = params.limit.min(100).max(1);
    let offset = params.offset.max(0);

    let transactions = TransactionService::list_for_account(
        &pool,
        auth.api_key.id,
        account_id,
        limit,
        offset,
    )
    .await?;

    let total = TransactionService::count_for_account(&pool, auth.api_key.id, account_id).await?;

    let data: Vec<TransactionResponse> = transactions.into_iter().map(|t| t.into()).collect();

    Ok(Json(ListResponse {
        data,
        total,
        limit,
        offset,
    }))
}
