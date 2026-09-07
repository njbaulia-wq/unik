//! Decoded video frame representation for preview presentation.

use std::sync::Arc;

/// Decoded video frame in RGBA format ready for texture upload or DMA-BUF presentation.
#[derive(Debug, Clone)]
pub struct DecodedVideoFrame {
    pub pts_sec: f64,
    pub width: u32,
    pub height: u32,
    pub rgba_data: Arc<Vec<u8>>,
    pub is_keyframe: bool,
}

impl DecodedVideoFrame {
    pub fn new(
        pts_sec: f64,
        width: u32,
        height: u32,
        rgba_data: Vec<u8>,
        is_keyframe: bool,
    ) -> Self {
        Self {
            pts_sec,
            width,
            height,
            rgba_data: Arc::new(rgba_data),
            is_keyframe,
        }
    }
}
