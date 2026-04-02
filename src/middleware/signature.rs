use axum::{
    extract::{Query, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::warn;

use crate::{utils::sign::verify_signature, AppState};

/// Query params extracted solely for signature validation.
/// These mirror the raw query string — we only need access to
/// the fields that participate in the HMAC base string.
#[derive(Debug, serde::Deserialize)]
pub struct SignatureParams {
    pub url: Option<String>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub format: Option<String>,
    pub q: Option<u8>,
    pub fit: Option<String>,
    pub exp: Option<u64>,
    pub sig: Option<String>,
}

/// Axum middleware that validates HMAC-SHA256 signed URLs.
///
/// Skipped entirely when `CDN_DEV_MODE` is set in `AppState`.
pub async fn signature_middleware(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SignatureParams>,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    // Dev mode bypass
    if state.dev_mode {
        return next.run(request).await;
    }

    let secret = match &state.cdn_secret {
        Some(s) => s,
        None => {
            warn!("CDN_SECRET not configured — rejecting all requests");
            return (StatusCode::FORBIDDEN, "CDN not configured").into_response();
        }
    };

    // Extract required fields
    let (exp, sig) = match (params.exp, &params.sig) {
        (Some(exp), Some(sig)) => (exp, sig.as_str()),
        _ => {
            return (StatusCode::FORBIDDEN, "missing exp or sig parameter").into_response();
        }
    };

    // Check expiration
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if now > exp {
        return (StatusCode::FORBIDDEN, "signature expired").into_response();
    }

    // Verify HMAC signature
    let url = params.url.as_deref().unwrap_or("");
    let valid = verify_signature(
        secret,
        url,
        params.w,
        params.h,
        params.format.as_deref(),
        params.q,
        params.fit.as_deref(),
        exp,
        sig,
    );

    if !valid {
        warn!(url = %url, "invalid signature");
        return (StatusCode::FORBIDDEN, "invalid signature").into_response();
    }

    next.run(request).await
}
