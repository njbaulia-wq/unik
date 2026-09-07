//! Error types for the export subsystem.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("I/O error for path {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("FFmpeg export failure: {0}")]
    Ffmpeg(String),
    #[error("Project has no clips to export")]
    EmptyProject,
    #[error("Export validation failed: {0}")]
    Validation(String),
    #[error("Export was cancelled by user")]
    Cancelled,
}
