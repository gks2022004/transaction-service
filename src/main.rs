use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

use transaction_service::{
    api::create_router,
    config::Config,
    db::pool::{create_pool, run_migrations},
    observability::logging::init_logging,
    webhooks::WebhookDispatcher,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize logging
    init_logging();

    info!("Starting Transaction Service");

    // Load configuration
    let config = Config::from_env().expect("Failed to load configuration");
    info!("Configuration loaded: {:?}", config);

    // Create database connection pool
    let pool = create_pool(&config.database_url).await?;

    // Run database migrations
    if let Err(e) = run_migrations(&pool).await {
        // Log but don't fail - migrations might already be applied
        info!("Migration note: {:?}", e);
    }

    // Start webhook dispatcher in background
    let dispatcher = Arc::new(WebhookDispatcher::new(Arc::new(pool.clone()), &config));
    let dispatcher_clone = dispatcher.clone();
    tokio::spawn(async move {
        dispatcher_clone.start().await;
    });

    // Create the router
    let app = create_router(pool, &config);

    // Start the server
    let addr = config.server_addr();
    info!("Starting server on {}", addr);
    
    let listener = TcpListener::bind(&addr).await?;
    
    axum::serve(listener, app)
        .await
        .map_err(|e| {
            error!("Server error: {:?}", e);
            anyhow::anyhow!("Server error: {}", e)
        })?;

    Ok(())
}
