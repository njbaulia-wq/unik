//! FluxCut Safe FFmpeg Native Core Wrapper.
//!
//! Provides RAII wrappers around `libavformat`, `libavcodec`, and `libswscale`
//! for asynchronous metadata probing, lazy thumbnail extraction, and audio waveform generation.

pub mod error;
pub mod probe;
pub mod thumbnail;
pub mod waveform;

pub use error::FfmpegCoreError;
pub use probe::{probe_file, AudioStreamProbe, MediaProbeResult, VideoStreamProbe};
pub use thumbnail::{extract_thumbnail, RgbaImage};
pub use waveform::{extract_waveform, WaveformBin, WaveformEnvelope};

use std::sync::Once;
use tracing::info;

static FFMPEG_INIT: Once = Once::new();

/// Initialize FFmpeg library state safely once across the entire process lifetime.
pub fn init() -> Result<(), FfmpegCoreError> {
    let mut init_error = None;
    FFMPEG_INIT.call_once(|| {
        if let Err(e) = ffmpeg_next::init() {
            init_error = Some(e.to_string());
        } else {
            info!("FFmpeg core native libraries initialized");
        }
    });

    if let Some(err) = init_error {
        Err(FfmpegCoreError::InitializationError(err))
    } else {
        Ok(())
    }
}
