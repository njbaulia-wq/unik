//! Video thumbnail extraction using FFmpeg decode and libswscale.

use crate::error::FfmpegCoreError;
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::debug;

/// RGBA image buffer suitable for rendering directly to GdkTexture or caching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Extract a video thumbnail at the specified timestamp.
pub fn extract_thumbnail(
    path: &Path,
    target_time_sec: f64,
    max_width: u32,
    max_height: u32,
) -> Result<RgbaImage, FfmpegCoreError> {
    crate::init()?;

    let mut ictx = ffmpeg::format::input(&path).map_err(|e| FfmpegCoreError::OpenInputError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| FfmpegCoreError::NoVideoStream(path.to_path_buf()))?;

    let stream_index = stream.index();
    let time_base = stream.time_base();

    let context =
        ffmpeg::codec::context::Context::from_parameters(stream.parameters()).map_err(|e| {
            FfmpegCoreError::DecoderInitError {
                stream_index,
                message: e.to_string(),
            }
        })?;

    let mut decoder = context
        .decoder()
        .video()
        .map_err(|e| FfmpegCoreError::DecoderInitError {
            stream_index,
            message: e.to_string(),
        })?;

    // Seek to nearest keyframe before target timestamp if target > 0.1
    if target_time_sec > 0.1 {
        let pts = (target_time_sec * (time_base.denominator() as f64)
            / (time_base.numerator() as f64))
            .round() as i64;
        let _ = ictx.seek(pts, ..pts);
        decoder.flush();
    }

    let mut decoded_frame = ffmpeg::util::frame::Video::empty();

    // Decode packets until we obtain a video frame
    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index
            && decoder.send_packet(&packet).is_ok()
            && decoder.receive_frame(&mut decoded_frame).is_ok()
        {
            return convert_frame_to_rgba(&decoded_frame, max_width, max_height);
        }
    }

    // Flush decoder if packet loop completed
    let _ = decoder.send_eof();
    if decoder.receive_frame(&mut decoded_frame).is_ok() {
        return convert_frame_to_rgba(&decoded_frame, max_width, max_height);
    }

    Err(FfmpegCoreError::DecodeError(
        "Could not decode any video frame from stream for thumbnail".to_string(),
    ))
}

fn convert_frame_to_rgba(
    frame: &ffmpeg::util::frame::Video,
    max_width: u32,
    max_height: u32,
) -> Result<RgbaImage, FfmpegCoreError> {
    let src_w = frame.width();
    let src_h = frame.height();

    // Compute dimensions fitting within bounds while keeping aspect ratio
    let scale_x = (max_width as f32) / (src_w as f32);
    let scale_y = (max_height as f32) / (src_h as f32);
    let scale = scale_x.min(scale_y).min(1.0);

    let out_w = ((src_w as f32 * scale).round() as u32).max(2) & !1;
    let out_h = ((src_h as f32 * scale).round() as u32).max(2) & !1;

    let mut scaler = ffmpeg::software::scaling::Context::get(
        frame.format(),
        src_w,
        src_h,
        ffmpeg::format::Pixel::RGBA,
        out_w,
        out_h,
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .map_err(|e| FfmpegCoreError::ScaleError(e.to_string()))?;

    let mut rgba_frame = ffmpeg::util::frame::Video::empty();
    scaler
        .run(frame, &mut rgba_frame)
        .map_err(|e| FfmpegCoreError::ScaleError(e.to_string()))?;

    let stride = rgba_frame.stride(0);
    let plane = rgba_frame.data(0);
    let mut data = Vec::with_capacity((out_w * out_h * 4) as usize);

    for y in 0..out_h as usize {
        let row_start = y * stride;
        let row_end = row_start + (out_w as usize * 4);
        data.extend_from_slice(&plane[row_start..row_end]);
    }

    debug!(
        width = out_w,
        height = out_h,
        bytes = data.len(),
        "Scaled thumbnail frame"
    );

    Ok(RgbaImage {
        width: out_w,
        height: out_h,
        data,
    })
}
