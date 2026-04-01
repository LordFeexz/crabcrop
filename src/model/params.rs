use serde::Deserialize;
use thiserror::Error;
use std::fmt::{Display,  Formatter};

const MAX_DIMENSION: u32 = 8000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Webp,
    Avif,
    Jpeg,
    Png,
}

impl ImageFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFormat::Webp => "webp",
            ImageFormat::Avif => "avif",
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Png => "png",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Webp => "image/webp",
            ImageFormat::Avif => "image/avif",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
        }
    }
}

impl Display for ImageFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum FitMode {
    #[default]
    Cover,
    Contain,
    Fill,
}

impl FitMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FitMode::Cover => "cover",
            FitMode::Contain => "contain",
            FitMode::Fill => "fill",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RawImageParams {
    /// Source image URL (https://... or s3://bucket/key)
    pub url: String,
    /// Target width in pixels
    pub w: Option<u32>,
    /// Target height in pixels
    pub h: Option<u32>,
    /// Output format: webp | avif | jpeg | png
    pub format: Option<String>,
    /// Quality (1–100, default 85)
    pub q: Option<u8>,
    /// Fit mode: cover | contain | fill
    pub fit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageParams {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: ImageFormat,
    pub quality: u8,
    pub fit: FitMode,
}

#[derive(Debug, Error)]
pub enum ParamError {
    #[error("url is required")]
    MissingUrl,
    #[error("url is not a valid https or s3 URL")]
    InvalidUrl,
    #[error("width must be between 1 and 8000")]
    InvalidWidth,
    #[error("height must be between 1 and 8000")]
    InvalidHeight,
    #[error("quality must be between 1 and 100")]
    InvalidQuality,
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("unsupported fit mode: {0}")]
    UnsupportedFit(String),
}

impl ImageParams {
    pub fn from_raw(raw: RawImageParams, accept_header: Option<&str>) -> Result<Self, ParamError> {
        let url = raw.url.trim().to_string();
        if url.is_empty() {
            return Err(ParamError::MissingUrl);
        }
        if !url.starts_with("https://")
            && !url.starts_with("http://")
            && !url.starts_with("s3://")
        {
            return Err(ParamError::InvalidUrl);
        }

        if let Some(w) = raw.w {
            if w == 0 || w > MAX_DIMENSION {
                return Err(ParamError::InvalidWidth);
            }
        }
        if let Some(h) = raw.h {
            if h == 0 || h > MAX_DIMENSION {
                return Err(ParamError::InvalidHeight);
            }
        }

        let quality = raw.q.unwrap_or(85);
        if quality == 0 || quality > 100 {
            return Err(ParamError::InvalidQuality);
        }

        let format = if let Some(fmt) = raw.format.as_deref() {
            parse_format(fmt)?
        } else {
            detect_format_from_accept(accept_header)
        };

        let fit = if let Some(f) = raw.fit.as_deref() {
            parse_fit(f)?
        } else {
            FitMode::Cover
        };

        Ok(ImageParams {
            url,
            width: raw.w,
            height: raw.h,
            format,
            quality,
            fit,
        })
    }
}

fn parse_format(s: &str) -> Result<ImageFormat, ParamError> {
    match s.to_lowercase().as_str() {
        "webp" => Ok(ImageFormat::Webp),
        "avif" => Ok(ImageFormat::Avif),
        "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
        "png" => Ok(ImageFormat::Png),
        other => Err(ParamError::UnsupportedFormat(other.to_string())),
    }
}

fn parse_fit(s: &str) -> Result<FitMode, ParamError> {
    match s.to_lowercase().as_str() {
        "cover" => Ok(FitMode::Cover),
        "contain" => Ok(FitMode::Contain),
        "fill" => Ok(FitMode::Fill),
        other => Err(ParamError::UnsupportedFit(other.to_string())),
    }
}

pub fn detect_format_from_accept(accept: Option<&str>) -> ImageFormat {
    let format = match accept {
        Some(a) => a,
        None => return ImageFormat::Jpeg,
    };
    if format.contains("image/avif") {
        ImageFormat::Avif
    } else if format.contains("image/webp") {
        ImageFormat::Webp
    } else {
        ImageFormat::Jpeg
    }
}
