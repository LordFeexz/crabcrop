use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    model::params::{ImageParams, RawImageParams},
    service::processor::process_image,
    utils::hash::cache_key,
    AppState,
};

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::BadRequest(e) => (StatusCode::BAD_REQUEST, e),
            AppError::NotFound(e) => (StatusCode::NOT_FOUND, e),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };
        (status, msg).into_response()
    }
}

/// `GET /img?url=...&w=...&h=...&format=...&q=...&fit=...`
///
/// Full processing pipeline:
/// 1. Parse + validate query params
/// 2. Compute cache key
/// 3. Check cache (memory → disk)
/// 4. Deduplicate in-flight requests
/// 5. Fetch source image
/// 6. Process with libvips (blocking task)
/// 7. Store result in cache
/// 8. Return response with proper Content-Type + Cache-Control
#[instrument(skip(state, headers))]
pub async fn image_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(raw): Query<RawImageParams>,
) -> Result<Response, AppError> {
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok());

    let params = ImageParams::from_raw(raw, accept).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let key = cache_key(&params);
    let mime = params.format.mime_type();

    info!(key = %key, url = %params.url, "image request");

    if let Some(cached) = state.cache.get(&key).await {
        info!("serving from cache");
        return Ok(cached_response(cached, mime));
    }

    let params_clone = params.clone();
    let state_clone = Arc::clone(&state);
    let key_clone = key.clone();

    let result = state
        .dedup
        .run(&key, || async move {
            process_pipeline(state_clone, params_clone, key_clone).await
        })
        .await;

    match result {
        Ok(data) => Ok(cached_response(data, mime)),
        Err(e) => {
            let msg = format!("{e:#}");
            if msg.contains("not found") || msg.contains("404") {
                Err(AppError::NotFound(msg))
            } else if msg.contains("invalid") || msg.contains("400") {
                Err(AppError::BadRequest(msg))
            } else {
                error!("processing error: {msg}");
                Err(AppError::Internal(msg))
            }
        }
    }
}

async fn process_pipeline(
    state: Arc<AppState>,
    params: ImageParams,
    key: String,
) -> anyhow::Result<Bytes> {
    let _permit = state.semaphore.acquire().await?;

    let raw_bytes = state
        .storage
        .fetch(&params.url)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                anyhow::anyhow!("image not found: {}", params.url)
            } else {
                e
            }
        })?;

    let processed = tokio::task::spawn_blocking(move || process_image(&raw_bytes, &params))
        .await
        .map_err(|e| anyhow::anyhow!("blocking task panic: {e}"))??;

    state.cache.ensure_disk_subdir(&key).await?;
    state.cache.put(&key, processed.clone()).await;

    Ok(processed)
}

fn cached_response(data: Bytes, mime: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header("X-Cache", "HIT")
        .body(Body::from(data))
        .unwrap()
}

#[derive(serde::Serialize)]
pub struct HealthResponse {
    status: &'static str,
    memory_usage_mb: f64,
    cpu_usage_pct: f32,
    vips_memory_mb: f64,
    vips_memory_highwater_mb: f64,
    vips_allocations: usize,
    cache_entries: u64,
}

pub async fn health_handler(State(state): State<Arc<AppState>>) -> axum::Json<HealthResponse> {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::get_current_pid().unwrap();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let (mem_mb, cpu) = if let Some(process) = sys.process(pid) {
        (process.memory() as f64 / 1_048_576.0, process.cpu_usage())
    } else {
        (0.0, 0.0)
    };

    let vips_mem_mb = unsafe { libvips::bindings::vips_tracked_get_mem() } as f64 / 1_048_576.0;
    let vips_highwater_mb = unsafe { libvips::bindings::vips_tracked_get_mem_highwater() } as f64 / 1_048_576.0;
    let vips_allocs = unsafe { libvips::bindings::vips_tracked_get_allocs() } as usize;

    let cache_entries = state.cache.entry_count();

    axum::Json(HealthResponse {
        status: "OK",
        memory_usage_mb: mem_mb,
        cpu_usage_pct: cpu,
        vips_memory_mb: vips_mem_mb,
        vips_memory_highwater_mb: vips_highwater_mb,
        vips_allocations: vips_allocs,
        cache_entries,
    })
}
