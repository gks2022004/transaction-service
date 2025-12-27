use axum::{
    extract::{Path, Query, Extension},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::api::errors::ApiError;
use crate::api::middleware::ApiKeyAuth;
use crate::models::{AccountResponse, BalanceResponse, CreateAccount};
use crate::services::AccountService;

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// POST /api/v1/accounts - Create a new account
pub async fn create_account(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Json(request): Json<CreateAccount>,
) -> Result<(StatusCode, Json<AccountResponse>), ApiError> {
    // Validate request
    request.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    let account = AccountService::create(&pool, auth.api_key.id, request).await?;

    Ok((StatusCode::CREATED, Json(AccountResponse::from(account))))
}

/// GET /api/v1/accounts - List all accounts
pub async fn list_accounts(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ListResponse<AccountResponse>>, ApiError> {
    let limit = params.limit.min(100).max(1);
    let offset = params.offset.max(0);

    let accounts = AccountService::list(&pool, auth.api_key.id, limit, offset).await?;
    let total = AccountService::count(&pool, auth.api_key.id).await?;

    let data: Vec<AccountResponse> = accounts.into_iter().map(|a| a.into()).collect();

    Ok(Json(ListResponse {
        data,
        total,
        limit,
        offset,
    }))
}

/// GET /api/v1/accounts/:id - Get account details
pub async fn get_account(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Path(account_id): Path<Uuid>,
) -> Result<Json<AccountResponse>, ApiError> {
    let account = AccountService::get_by_id(&pool, auth.api_key.id, account_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Account {} not found", account_id)))?;

    Ok(Json(AccountResponse::from(account)))
}

/// GET /api/v1/accounts/:id/balance - Get account balance
pub async fn get_balance(
    auth: ApiKeyAuth,
    Extension(pool): Extension<Arc<PgPool>>,
    Path(account_id): Path<Uuid>,
) -> Result<Json<BalanceResponse>, ApiError> {
    let account = AccountService::get_by_id(&pool, auth.api_key.id, account_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Account {} not found", account_id)))?;

    Ok(Json(BalanceResponse::from(account)))
}
