use anyhow::{Context, Result};
use bytes::Bytes;
use libvips::{ops, VipsApp, VipsImage};
use tracing::instrument;

use crate::model::params::{FitMode, ImageFormat, ImageParams};

pub fn init_vips() -> VipsApp {
    VipsApp::new("crabcrop", true).expect("failed to initialize libvips")
}

/// Operations (in order):
/// 1. Load image from buffer
/// 2. Resize / fit
/// 3. Strip metadata
/// 4. Encode to target format + quality
#[instrument(skip(input), fields(
    url  = %params.url,
    fmt  = %params.format,
    w    = ?params.width,
    h    = ?params.height,
    q    = params.quality,
    fit  = %params.fit.as_str(),
))]
pub fn process_image(input: &[u8], params: &ImageParams) -> Result<Bytes> {
    // 1 & 2. Load and resize
    let resized = resize_buffer(input, params)
        .context("libvips: resize failed")?;

    // 3. Strip EXIF / ICC profile metadata
    let stripped = ops::autorot(&resized)
        .context("libvips: autorot failed")?;

    // 4. Encode to target format
    encode(&stripped, params)
        .context("libvips: encode failed")
}

fn resize_buffer(input: &[u8], params: &ImageParams) -> Result<VipsImage> {
    let image = VipsImage::new_from_buffer(input, "access=sequential")
        .context("load image from buffer")?;

    let (target_w,target_h) = (params.width.unwrap_or(0) as i32, params.height.unwrap_or(0) as i32);

    if target_w <= 0 && target_h <= 0 {
        return Ok(image);
    }

    let src_w = image.get_width() as f64;
    let src_h = image.get_height() as f64;

    let tw = if target_w > 0 { target_w as f64 } else { src_w * (target_h as f64 / src_h) };
    let th = if target_h > 0 { target_h as f64 } else { src_h * (target_w as f64 / src_w) };

    tracing::info!("RESIZE DEBUG: src={}x{}, target={}x{}, tw={}, th={}", src_w, src_h, target_w, target_h, tw, th);

    match params.fit {
        FitMode::Cover => {
            let scale = (tw / src_w).max(th / src_h);

            let scaled = ops::resize(&image, scale)
            .context("cover: scale")?;

            let crop_w = scaled.get_width().min(tw as i32);
            let crop_h = scaled.get_height().min(th as i32);
            let x = (scaled.get_width() - crop_w) / 2;
            let y = (scaled.get_height() - crop_h) / 2;
            ops::extract_area(&scaled, x, y, crop_w, crop_h)
                .context("cover: crop")
        }

        FitMode::Contain => {
            let scale = (tw / src_w).min(th / src_h);

            ops::resize(&image, scale)
            .context("contain: scale")
        }

        FitMode::Fill => {
            let scale_x = tw / src_w;
            let scale_y = th / src_h;

            ops::resize_with_opts(
                &image,
                scale_x,
                &ops::ResizeOptions {
                    vscale: scale_y,
                    ..Default::default()
                },
            )
            .context("fill resize")
        }
    }
}

fn encode(image: &VipsImage, params: &ImageParams) -> Result<Bytes> {
    let quality = params.quality as i32;

    let buf = match params.format {
        ImageFormat::Webp => ops::webpsave_buffer_with_opts(
            image,
            &ops::WebpsaveBufferOptions {
                q: quality,
                ..Default::default()
            },
        )
        .context("encode webp")?,

        ImageFormat::Avif => ops::heifsave_buffer_with_opts(
            image,
            &ops::HeifsaveBufferOptions {
                q: quality,
                compression: ops::ForeignHeifCompression::Av1,
                ..Default::default()
            },
        )
        .context("encode avif")?,

        ImageFormat::Jpeg => ops::jpegsave_buffer_with_opts(
            image,
            &ops::JpegsaveBufferOptions {
                q: quality,
                ..Default::default()
            },
        )
        .context("encode jpeg")?,

        ImageFormat::Png => {
            ops::pngsave_buffer_with_opts(
                image,
                &ops::PngsaveBufferOptions {
                    ..Default::default()
                },
            )
            .context("encode png")?
        }
    };

    Ok(Bytes::from(buf))
}
