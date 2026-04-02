use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Build the HMAC-SHA256 base string from image parameters.
///
/// Format: `url|w|h|format|q|fit|exp`
/// Fields that are `None` are represented as empty strings.
fn build_base_string(
    url: &str,
    w: Option<u32>,
    h: Option<u32>,
    format: Option<&str>,
    q: Option<u8>,
    fit: Option<&str>,
    exp: u64,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        url,
        w.map(|v| v.to_string()).unwrap_or_default(),
        h.map(|v| v.to_string()).unwrap_or_default(),
        format.unwrap_or(""),
        q.map(|v| v.to_string()).unwrap_or_default(),
        fit.unwrap_or(""),
        exp,
    )
}

/// Generate an HMAC-SHA256 signature for the given image parameters.
///
/// Returns a hex-encoded signature string.
/// example generate
#[allow(dead_code)]
pub fn generate_signature(
    secret: &str,
    url: &str,
    w: Option<u32>,
    h: Option<u32>,
    format: Option<&str>,
    q: Option<u8>,
    fit: Option<&str>,
    exp: u64,
) -> String {
    let base = build_base_string(url, w, h, format, q, fit, exp);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(base.as_bytes());

    hex::encode(mac.finalize().into_bytes())
}

/// Verify an HMAC-SHA256 signature (constant-time comparison).
///
/// Returns `true` if the signature is valid.
pub fn verify_signature(
    secret: &str,
    url: &str,
    w: Option<u32>,
    h: Option<u32>,
    format: Option<&str>,
    q: Option<u8>,
    fit: Option<&str>,
    exp: u64,
    sig: &str,
) -> bool {
    let base = build_base_string(url, w, h, format, q, fit, exp);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(base.as_bytes());

    // Decode the provided hex signature and do constant-time comparison
    match hex::decode(sig) {
        Ok(sig_bytes) => mac.verify_slice(&sig_bytes).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-key-123";

    #[test]
    fn roundtrip_sign_verify() {
        let sig = generate_signature(SECRET, "https://example.com/img.jpg", Some(300), Some(200), Some("webp"), Some(85), Some("cover"), 9999999999);
        assert!(verify_signature(SECRET, "https://example.com/img.jpg", Some(300), Some(200), Some("webp"), Some(85), Some("cover"), 9999999999, &sig));
    }

    #[test]
    fn tampered_url_fails() {
        let sig = generate_signature(SECRET, "https://example.com/img.jpg", Some(300), Some(200), Some("webp"), Some(85), Some("cover"), 9999999999);
        assert!(!verify_signature(SECRET, "https://evil.com/img.jpg", Some(300), Some(200), Some("webp"), Some(85), Some("cover"), 9999999999, &sig));
    }

    #[test]
    fn tampered_dimensions_fails() {
        let sig = generate_signature(SECRET, "https://example.com/img.jpg", Some(300), Some(200), Some("webp"), Some(85), Some("cover"), 9999999999);
        assert!(!verify_signature(SECRET, "https://example.com/img.jpg", Some(999), Some(200), Some("webp"), Some(85), Some("cover"), 9999999999, &sig));
    }

    #[test]
    fn different_secret_fails() {
        let sig = generate_signature(SECRET, "https://example.com/img.jpg", Some(300), None, None, None, None, 9999999999);
        assert!(!verify_signature("wrong-secret", "https://example.com/img.jpg", Some(300), None, None, None, None, 9999999999, &sig));
    }

    #[test]
    fn none_params_produce_valid_sig() {
        let sig = generate_signature(SECRET, "https://example.com/img.jpg", None, None, None, None, None, 9999999999);
        assert!(verify_signature(SECRET, "https://example.com/img.jpg", None, None, None, None, None, 9999999999, &sig));
    }

    #[test]
    fn invalid_hex_sig_fails() {
        assert!(!verify_signature(SECRET, "https://example.com/img.jpg", None, None, None, None, None, 9999999999, "not-valid-hex!!!"));
    }

    #[test]
    fn signature_is_64_hex_chars() {
        let sig = generate_signature(SECRET, "https://example.com/img.jpg", Some(300), Some(200), Some("webp"), Some(85), Some("cover"), 9999999999);
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
