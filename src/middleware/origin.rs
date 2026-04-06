use axum::{
    extract::State,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::warn;

use crate::AppState;

/// Axum middleware that validates HTTP Origin or Referer against an accept list.
///
/// Skipped entirely when `CDN_DEV_MODE` is set or if `ACCEPT_ORIGINS` is empty.
pub async fn origin_middleware(
    State(state): State<Arc<AppState>>,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    // Dev mode bypass or empty allowlist allows all
    if state.dev_mode || state.accept_origins.is_empty() {
        return next.run(request).await;
    }

    let origin_or_referer = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            request
                .headers()
                .get(header::REFERER)
                .and_then(|h| h.to_str().ok())
        });

    let allowed = match origin_or_referer {
        Some(o) => {
            // Check if origin/referer starts with any allowed origin
            state.accept_origins.iter().any(|allowed| o.starts_with(allowed))
        }
        None => {
            // Block direct requests or requests with no referer
            false
        }
    };

    if !allowed {
        warn!(origin = ?origin_or_referer, "forbidden origin attempted to hotlink");
        return (StatusCode::FORBIDDEN, "Origin not allowed").into_response();
    }

    next.run(request).await
}
