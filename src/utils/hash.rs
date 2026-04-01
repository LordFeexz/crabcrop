use crate::model::params::ImageParams;

pub fn cache_key(params: &ImageParams) -> String {
    let mut hasher = blake3::Hasher::new();

    hasher.update(params.url.as_bytes());
    hasher.update(b"|w:");
    hasher.update(
        params
            .width
            .map(|v| v.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(b"|h:");
    hasher.update(
        params
            .height
            .map(|v| v.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(b"|fmt:");
    hasher.update(params.format.as_str().as_bytes());
    hasher.update(b"|q:");
    hasher.update(params.quality.to_string().as_bytes());
    hasher.update(b"|fit:");
    hasher.update(params.fit.as_str().as_bytes());

    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::params::{FitMode, ImageFormat, ImageParams};

    fn sample_params() -> ImageParams {
        ImageParams {
            url: "https://example.com/photo.jpg".to_string(),
            width: Some(800),
            height: Some(600),
            format: ImageFormat::Webp,
            quality: 85,
            fit: FitMode::Cover,
        }
    }

    #[test]
    fn same_params_produce_same_key() {
        let k1 = cache_key(&sample_params());
        let k2 = cache_key(&sample_params());
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_format_produces_different_key() {
        let mut p = sample_params();
        let k1 = cache_key(&p);
        p.format = ImageFormat::Jpeg;
        let k2 = cache_key(&p);
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_quality_produces_different_key() {
        let mut p = sample_params();
        let k1 = cache_key(&p);
        p.quality = 60;
        let k2 = cache_key(&p);
        assert_ne!(k1, k2);
    }

    #[test]
    fn key_is_64_hex_chars() {
        let k = cache_key(&sample_params());
        assert_eq!(k.len(), 64);
        assert!(k.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
