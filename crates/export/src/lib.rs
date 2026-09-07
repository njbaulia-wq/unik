//! FluxCut Export Pipeline Subsystem.
//!
//! Provides fast-path lossless stream-copy remuxing, background video/audio
//! transcoding, progress tracking, and post-export QA file validation.

pub mod controller;
pub mod error;
pub mod remux;
pub mod transcode;
pub mod validator;

pub use controller::{ExportController, ExportEvent, ExportMode};
pub use error::ExportError;
pub use remux::{can_fast_path_remux, fast_path_remux};
pub use transcode::{ExportProgress, TranscodeEngine};
pub use validator::{verify_exported_file, ExportValidationReport};
