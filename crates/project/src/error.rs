//! Error definitions for project persistence and data modeling.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("I/O error for path {path}: {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Serialization / Deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Unsupported project format version: {found}. Maximum supported is {supported}.")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("Invalid project data: {0}")]
    ValidationError(String),
    #[error("Configuration directory could not be determined")]
    ConfigDirectoryUnavailable,
    #[error("Clip not found with id: {0}")]
    ClipNotFound(String),
    #[error("Track not found with id: {0}")]
    TrackNotFound(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}
