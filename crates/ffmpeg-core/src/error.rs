//! Error types for FFmpeg core operations.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FfmpegCoreError {
    #[error("Failed to initialize FFmpeg subsystem: {0}")]
    InitializationError(String),
    #[error("Failed to open media file at {path}: {message}")]
    OpenInputError { path: PathBuf, message: String },
    #[error("No video stream found in {0}")]
    NoVideoStream(PathBuf),
    #[error("No audio stream found in {0}")]
    NoAudioStream(PathBuf),
    #[error("Failed to initialize decoder for stream {stream_index}: {message}")]
    DecoderInitError {
        stream_index: usize,
        message: String,
    },
    #[error("Seeking error at timestamp {timestamp_sec}s: {message}")]
    SeekError { timestamp_sec: f64, message: String },
    #[error("Decoding frame error: {0}")]
    DecodeError(String),
    #[error("Software scaler/converter error: {0}")]
    ScaleError(String),
    #[error("Software resampler error: {0}")]
    ResampleError(String),
    #[error("I/O error during FFmpeg processing: {0}")]
    IoError(#[from] std::io::Error),
}
