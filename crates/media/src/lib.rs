//! FluxCut Media Ingestion and Asset Management.
//!
//! Provides the `MediaAsset` model, asynchronous worker threadpool for lazy probing,
//! and disk caching integration.

pub mod asset;
pub mod worker;

pub use asset::{AssetStatus, AudioStreamInfo, MediaAsset, MediaMetadata, VideoStreamInfo};
pub use worker::{IngestRequest, IngestResponse, MediaWorkerPool};
