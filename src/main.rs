mod handler;
mod middleware;
mod model;
mod service;
mod storage;
mod utils;

use axum::{routing::{get, post}, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use service::{cache::ImageCache, dedup::DedupManager, processor::init_vips};
use storage::s3::StorageClient;
use self::middleware as mw;


pub struct AppState {
    /// Two-layer cache (memory + disk)
    pub cache: ImageCache,
    /// Storage client (HTTP + S3)
    pub storage: StorageClient,
    /// Request deduplicator (singleflight)
    pub dedup: DedupManager,
    /// Concurrency limit semaphore to prevent OOM
    pub semaphore: Arc<Semaphore>,
    /// HMAC secret for URL signing (None = reject all in production)
    pub cdn_secret: Option<String>,
    /// If true, bypass signature validation (development mode)
    pub dev_mode: bool,
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

    let cdn_secret = std::env::var("CDN_SECRET").ok();
    let dev_mode = std::env::var("CDN_DEV_MODE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if dev_mode {
        tracing::warn!("CDN_DEV_MODE is ON — signature validation DISABLED");
    } else if cdn_secret.is_none() {
        tracing::warn!("CDN_SECRET not set — all signed requests will be rejected");
    }

    let state = Arc::new(AppState {
        cache,
        storage,
        dedup,
        semaphore,
        cdn_secret,
        dev_mode,
    });

    // Routes that require signature validation
    let signed_routes = Router::new()
        .route("/img", get(handler::image::image_handler))
        .route("/img/blur", get(handler::image::blur_handler))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            mw::signature::signature_middleware,
        ));

    let app = Router::new()
        .route("/health", get(handler::image::health_handler))
        .route("/cache/purge", post(handler::cache::purge_handler))
        .merge(signed_routes)
        .with_state(state.clone())
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(30)));

    let cleanup_cache = state.cache.clone();
    let cleanup_ttl_hours = std::env::var("DISK_CACHE_TTL_HOURS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(24);
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Every 1 hour
        let max_age = std::time::Duration::from_secs(cleanup_ttl_hours * 3600);
        loop {
            interval.tick().await;
            let start = std::time::Instant::now();
            let deleted = cleanup_cache.cleanup_expired_disk_cache(max_age).await;
            if deleted > 0 {
                tracing::info!(
                    deleted_files = deleted,
                    elapsed_ms = start.elapsed().as_millis(),
                    ttl_hours = cleanup_ttl_hours,
                    "automated disk cache cleanup completed"
                );
            }
        }
    });

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
