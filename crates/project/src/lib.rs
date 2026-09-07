//! FluxCut Project Data Model and Configuration.
//!
//! Exposes the versioned project schema, time arithmetic, preferences,
//! and persistence abstractions.

pub mod config;
pub mod error;
pub mod history;
pub mod operations;
pub mod schema;
pub mod time;

pub use config::{AppConfig, ThemePreference};
pub use error::ProjectError;
pub use history::{HistoryEntry, ProjectHistory};
pub use schema::{
    AssetReference, CanvasRatio, ClipDefinition, CropRect, FitMode, ImageTemplate, Project,
    ProjectSettings, TrackDefinition, TrackKind, Transform, CURRENT_FORMAT_VERSION,
};
pub use time::TimeRational;
