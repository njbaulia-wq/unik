//! Media asset domain model and metadata representation.

use fluxcut_project::TimeRational;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Status of media file analysis and preparation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetStatus {
    Pending,
    Probing,
    Ready,
    Failed(String),
}

/// Information about the primary video stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoStreamInfo {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: TimeRational,
    pub pixel_format: String,
}

/// Information about the primary audio stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioStreamInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_layout: String,
}

/// Comprehensive metadata for an ingested media asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub format_name: String,
    pub duration: TimeRational,
    pub video: Option<VideoStreamInfo>,
    pub audio: Option<AudioStreamInfo>,
}

/// First-class media asset registered in the FluxCut project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaAsset {
    pub id: String,
    pub file_path: PathBuf,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub modified_timestamp: u64,
    pub status: AssetStatus,
    pub metadata: Option<MediaMetadata>,
    pub thumbnail_key: Option<String>,
    pub waveform_key: Option<String>,
}

impl MediaAsset {
    /// Create a new asset in Pending state from a filesystem path.
    pub fn from_path(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let meta = std::fs::metadata(&path)?;
        let file_size_bytes = meta.len();
        let modified_timestamp = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed_media".to_string());

        let id = format!("asset_{}_{}", modified_timestamp, file_size_bytes);

        Ok(Self {
            id,
            file_path: path,
            file_name,
            file_size_bytes,
            modified_timestamp,
            status: AssetStatus::Pending,
            metadata: None,
            thumbnail_key: None,
            waveform_key: None,
        })
    }

    pub fn is_ready(&self) -> bool {
        self.status == AssetStatus::Ready
    }
}
