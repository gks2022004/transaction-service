use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

/// Token bucket rate limiter
#[derive(Debug, Clone)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    requests_per_second: u32,
    burst_size: u32,
}

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_update: Instant,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            requests_per_second,
            burst_size,
        }
    }

    /// Try to acquire a token for the given key
    pub async fn try_acquire(&self, key: &str) -> bool {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();
        let refill_rate = self.requests_per_second as f64;
        let max_tokens = self.burst_size as f64;

        let bucket = buckets.entry(key.to_string()).or_insert(TokenBucket {
            tokens: max_tokens,
            last_update: now,
        });

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * refill_rate).min(max_tokens);
        bucket.last_update = now;

        // Try to consume a token
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Cleanup old buckets to prevent memory leaks
    pub async fn cleanup(&self, max_age: Duration) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();
        buckets.retain(|_, bucket| now.duration_since(bucket.last_update) < max_age);
    }
}

#[derive(Serialize)]
struct RateLimitError {
    error: RateLimitErrorBody,
}

#[derive(Serialize)]
struct RateLimitErrorBody {
    code: String,
    message: String,
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    request: Request<Body>,
    next: Next,
) -> Response {
    // Extract rate limiter from extensions
    let rate_limiter = request
        .extensions()
        .get::<RateLimiter>()
        .cloned();

    // Extract API key ID for rate limiting
    let api_key_id = request
        .headers()
        .get("x-api-key-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // If no rate limiter or API key, proceed without rate limiting
    let (rate_limiter, key) = match (rate_limiter, api_key_id) {
        (Some(rl), Some(k)) => (rl, k),
        _ => return next.run(request).await,
    };

    // Check rate limit
    if !rate_limiter.try_acquire(&key).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", "1")],
            Json(RateLimitError {
                error: RateLimitErrorBody {
                    code: "rate_limit_exceeded".to_string(),
                    message: "Too many requests. Please slow down.".to_string(),
                },
            }),
        )
            .into_response();
    }

    next.run(request).await
}
