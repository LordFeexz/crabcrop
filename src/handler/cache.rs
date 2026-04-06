use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use tracing::{info, warn};

use crate::{
    model::params::{ImageFormat, ImageParams, FitMode},
    utils::hash::cache_key,
    AppState,
};

#[derive(serde::Deserialize)]
pub struct PurgeRequest {
    /// Source image URL
    pub url: String,
    /// Optional: specific width variant to purge
    pub width: Option<u32>,
    /// Optional: specific height variant to purge
    pub height: Option<u32>,
    /// Optional: specific format variant to purge
    pub format: Option<String>,
    /// Optional: specific quality variant to purge
    pub quality: Option<u8>,
    /// Optional: specific fit mode variant to purge
    pub fit: Option<String>,
    /// If true, purge ALL cached variants of this URL
    #[serde(default)]
    pub wildcard: bool,
}

#[derive(serde::Serialize)]
pub struct PurgeResponse {
    pub status: &'static str,
    pub purged: u32,
}

/// `POST /cache/purge`
///
/// Protected by `Authorization: Bearer <CDN_SECRET>` header.
/// Purges cached image variants from both memory and disk.
pub async fn purge_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<PurgeRequest>,
) -> impl IntoResponse {
    let secret = match std::env::var("PURGE_TOKEN") {
        Ok(s) => s,
        Err(_) => return (StatusCode::FORBIDDEN, Json(PurgeResponse { status: "PURGE_TOKEN not configured", purged: 0 })),
    };

    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let expected = format!("Bearer {}", secret);
    if auth != expected {
        warn!("purge: invalid authorization");
        return (StatusCode::FORBIDDEN, Json(PurgeResponse { status: "unauthorized", purged: 0 }));
    }

    if body.wildcard {
        let count = state.cache.invalidate_by_url(&body.url).await;
        info!(url = %body.url, count, "wildcard purge complete");
        return (StatusCode::OK, Json(PurgeResponse { status: "purged", purged: count }));
    }

    // Purge a specific variant
    let format = body.format.as_deref()
        .and_then(|f| match f.to_lowercase().as_str() {
            "webp" => Some(ImageFormat::Webp),
            "avif" => Some(ImageFormat::Avif),
            "jpeg" | "jpg" => Some(ImageFormat::Jpeg),
            "png" => Some(ImageFormat::Png),
            _ => None,
        })
        .unwrap_or(ImageFormat::Jpeg);

    let fit = body.fit.as_deref()
        .and_then(|f| match f.to_lowercase().as_str() {
            "cover" => Some(FitMode::Cover),
            "contain" => Some(FitMode::Contain),
            "fill" => Some(FitMode::Fill),
            _ => None,
        })
        .unwrap_or(FitMode::Cover);

    let params = ImageParams {
        url: body.url.clone(),
        width: body.width,
        height: body.height,
        format,
        quality: body.quality.unwrap_or(85),
        fit,
        revalidate: false,
    };

    let key = cache_key(&params);
    state.cache.invalidate(&key).await;
    info!(key = %key, url = %body.url, "specific variant purged");

    (StatusCode::OK, Json(PurgeResponse { status: "purged", purged: 1 }))
}
