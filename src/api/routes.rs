use axum::{
    routing::{get, post, delete},
    Extension, Router,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::api::handlers;
use crate::api::middleware::RateLimiter;
use crate::config::Config;

/// Health check response
async fn health_check() -> &'static str {
    "OK"
}

/// Readiness check - verifies database connection
async fn readiness_check(
    Extension(pool): Extension<Arc<PgPool>>,
) -> Result<&'static str, &'static str> {
    sqlx::query("SELECT 1")
        .execute(pool.as_ref())
        .await
        .map(|_| "OK")
        .map_err(|_| "Database connection failed")
}

/// Create the main application router
pub fn create_router(pool: PgPool, config: &Config) -> Router {
    let pool = Arc::new(pool);
    let rate_limiter = RateLimiter::new(
        config.rate_limit_requests_per_second,
        config.rate_limit_burst_size,
    );

    // API v1 routes requiring authentication
    let api_v1 = Router::new()
        // Accounts
        .route("/accounts", post(handlers::create_account))
        .route("/accounts", get(handlers::list_accounts))
        .route("/accounts/:id", get(handlers::get_account))
        .route("/accounts/:id/balance", get(handlers::get_balance))
        .route("/accounts/:id/transactions", get(handlers::list_account_transactions))
        // Transactions
        .route("/transactions", post(handlers::create_transaction))
        .route("/transactions/:id", get(handlers::get_transaction))
        // Webhooks
        .route("/webhooks", post(handlers::create_webhook))
        .route("/webhooks", get(handlers::list_webhooks))
        .route("/webhooks/:id", get(handlers::get_webhook))
        .route("/webhooks/:id", delete(handlers::delete_webhook));

    // Admin routes (for API key creation)
    let admin = Router::new()
        .route("/api-keys", post(handlers::create_api_key));

    // Combine all routes
    Router::new()
        // Health endpoints (no auth required)
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        // API routes
        .nest("/api/v1", api_v1)
        .nest("/admin", admin)
        // Extensions
        .layer(Extension(pool))
        .layer(Extension(rate_limiter))
        // Tracing
        .layer(TraceLayer::new_for_http())
}
