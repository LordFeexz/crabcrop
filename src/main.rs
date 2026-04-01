use axum::{routing::get,Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use service::{cache::ImageCache, dedup::DedupManager, processor::init_vips};
use storage::s3::StorageClient;

mod handler;
mod model;
mod service;
mod storage;
mod utils;


pub struct AppState {
    /// Two-layer cache (memory + disk)
    pub cache: ImageCache,
    /// Storage client (HTTP + S3)
    pub storage: StorageClient,
    /// Request deduplicator (singleflight)
    pub dedup: DedupManager,
    /// Concurrency limit semaphore to prevent OOM
    pub semaphore: Arc<Semaphore>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "crabcrop=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let vips_app = init_vips();
    vips_app.concurrency_set(2);
    vips_app.cache_set_max(0);

    let storage = StorageClient::new(None);
    let cache = ImageCache::default_cache().await?;
    let dedup = DedupManager::new();
    let semaphore = Arc::new(Semaphore::new(100)); // Max 100 concurrent image processing jobs

    let state = Arc::new(AppState {
        cache,
        storage,
        dedup,
        semaphore,
    });

    let app = Router::new()
        .route("/health", get(handler::image::health_handler))
        .route("/img", get(handler::image::image_handler))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(30)));

    let port = std::env::var("PORT").unwrap_or_else(|_| "3005".to_string());
    let addr = SocketAddr::from(([0, 0, 0, 0], port.parse()?));
    tracing::info!("Server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
