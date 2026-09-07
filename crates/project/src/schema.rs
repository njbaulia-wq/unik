//! Project Schema Definitions (Version 1).
//!
//! Models project configuration, tracks, clips, and asset metadata.

use crate::error::ProjectError;
use crate::time::TimeRational;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CURRENT_FORMAT_VERSION: u32 = 1;

/// Supported canvas aspect ratios and dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CanvasRatio {
    /// 16:9 Landscape (1920x1080)
    #[default]
    Landscape16x9,
    /// 9:16 Vertical (1080x1920) - YouTube Shorts / TikTok / Reels
    Vertical9x16,
    /// 1:1 Square (1080x1080) - Instagram post
    Square1x1,
    /// 4:3 Classic (1440x1080)
    Classic4x3,
    /// Custom resolution
    Custom { width: u32, height: u32 },
}

impl CanvasRatio {
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            CanvasRatio::Landscape16x9 => (1920, 1080),
            CanvasRatio::Vertical9x16 => (1080, 1920),
            CanvasRatio::Square1x1 => (1080, 1080),
            CanvasRatio::Classic4x3 => (1440, 1080),
            CanvasRatio::Custom { width, height } => (width, height),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CanvasRatio::Landscape16x9 => "16:9 Landscape (1920x1080)",
            CanvasRatio::Vertical9x16 => "9:16 Vertical (1080x1920)",
            CanvasRatio::Square1x1 => "1:1 Square (1080x1080)",
            CanvasRatio::Classic4x3 => "4:3 Classic (1440x1080)",
            CanvasRatio::Custom { .. } => "Custom Dimensions",
        }
    }
}

/// Canvas aspect ratio fit behavior (PRD FR-CANVAS-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FitMode {
    /// Fit content with letterbox / pillarbox bars (preserve aspect ratio).
    #[default]
    Fit,
    /// Fill canvas by cropping excess edges.
    Fill,
    /// Stretch content anamorphic to match canvas dimensions.
    Stretch,
    /// Center content without scaling (1:1 pixels).
    Center,
}

/// Normalized cropping rectangle (0.0 to 1.0 coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 2D geometric and compositing transformation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_degrees: f32,
    pub opacity: f32,
    pub position_x: f32,
    pub position_y: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_degrees: 0.0,
            opacity: 1.0,
            position_x: 0.0,
            position_y: 0.0,
        }
    }
}

/// Built-in image overlay templates (PRD FR-IMAGE-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageTemplate {
    CenterCard,
    FullBleed,
    TopLeftWatermark,
    BottomRightWatermark,
    SplitScreen,
    PictureInPicture,
    IntroTitleCard,
    EndCard,
}

/// Project settings such as resolution, frame rate, and canvas fit behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub background_color: String,
    pub default_image_duration_sec: f64,
    #[serde(default)]
    pub fit_mode: FitMode,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        let (width, height) = CanvasRatio::Landscape16x9.dimensions();
        Self {
            width,
            height,
            fps_num: 30,
            fps_den: 1,
            background_color: "#000000".to_string(),
            default_image_duration_sec: 5.0,
            fit_mode: FitMode::Fit,
        }
    }
}

/// Type of media track on the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

/// Non-destructive clip instance on a track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipDefinition {
    pub id: String,
    pub asset_id: String,
    /// Starting position on the timeline.
    pub timeline_start: TimeRational,
    /// In-point (trim start) within the source asset.
    pub in_point: TimeRational,
    /// Out-point (trim end) within the source asset.
    pub out_point: TimeRational,
    /// Per-clip audio volume multiplier (1.0 = 100%).
    pub volume: f32,
    /// Whether the audio on this clip is muted.
    pub muted: bool,
    /// Speed multiplier (1.0 = 1x).
    pub speed: f32,
    /// Audio fade-in duration (PRD FR-AUDIO-005).
    #[serde(default)]
    pub fade_in: Option<TimeRational>,
    /// Audio fade-out duration (PRD FR-AUDIO-005).
    #[serde(default)]
    pub fade_out: Option<TimeRational>,
    /// Optional crop rectangle.
    #[serde(default)]
    pub crop: Option<CropRect>,
    /// Optional geometric transform and opacity.
    #[serde(default)]
    pub transform: Option<Transform>,
    /// Optional image layout preset template.
    #[serde(default)]
    pub template: Option<ImageTemplate>,
}

impl Default for ClipDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            asset_id: String::new(),
            timeline_start: TimeRational::ZERO,
            in_point: TimeRational::ZERO,
            out_point: TimeRational::ZERO,
            volume: 1.0,
            muted: false,
            speed: 1.0,
            fade_in: None,
            fade_out: None,
            crop: None,
            transform: None,
            template: None,
        }
    }
}

impl ClipDefinition {
    pub fn new(
        id: impl Into<String>,
        asset_id: impl Into<String>,
        start: TimeRational,
        duration: TimeRational,
    ) -> Self {
        Self {
            id: id.into(),
            asset_id: asset_id.into(),
            timeline_start: start,
            in_point: TimeRational::ZERO,
            out_point: duration,
            ..Default::default()
        }
    }

    pub fn duration(&self) -> TimeRational {
        self.out_point - self.in_point
    }

    pub fn timeline_end(&self) -> TimeRational {
        self.timeline_start + self.duration()
    }
}

/// Track containing an ordered sequence of clips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackDefinition {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub muted: bool,
    pub volume: f32,
    pub clips: Vec<ClipDefinition>,
}

/// Reference to an imported source media file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetReference {
    pub id: String,
    pub file_path: PathBuf,
    pub display_name: String,
    pub duration: TimeRational,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_audio: bool,
    pub has_video: bool,
}

/// Full FluxCut project representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format_version: u32,
    pub name: String,
    pub settings: ProjectSettings,
    pub assets: Vec<AssetReference>,
    pub tracks: Vec<TrackDefinition>,
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled Project")
    }
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            name: name.into(),
            settings: ProjectSettings::default(),
            assets: Vec::new(),
            tracks: vec![
                TrackDefinition {
                    id: "track-v1".to_string(),
                    name: "Video 1".to_string(),
                    kind: TrackKind::Video,
                    muted: false,
                    volume: 1.0,
                    clips: Vec::new(),
                },
                TrackDefinition {
                    id: "track-a1".to_string(),
                    name: "Audio 1".to_string(),
                    kind: TrackKind::Audio,
                    muted: false,
                    volume: 1.0,
                    clips: Vec::new(),
                },
            ],
        }
    }

    /// Set canvas ratio, adjusting width and height accordingly.
    pub fn set_canvas_ratio(&mut self, ratio: CanvasRatio) {
        let (w, h) = ratio.dimensions();
        self.settings.width = w;
        self.settings.height = h;
    }

    /// Save the project file as JSON to disk.
    pub fn save_to_file(&self, path: &Path) -> Result<(), ProjectError> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json).map_err(|source| ProjectError::IoError {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    /// Load and validate a project file from disk.
    pub fn load_from_file(path: &Path) -> Result<Self, ProjectError> {
        let content = fs::read_to_string(path).map_err(|source| ProjectError::IoError {
            path: path.to_path_buf(),
            source,
        })?;
        let project: Project = serde_json::from_str(&content)?;

        if project.format_version > CURRENT_FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion {
                found: project.format_version,
                supported: CURRENT_FORMAT_VERSION,
            });
        }

        Ok(project)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_defaults() {
        let p = Project::default();
        assert_eq!(p.format_version, CURRENT_FORMAT_VERSION);
        assert_eq!(p.tracks.len(), 2);
        assert_eq!(p.settings.width, 1920);
        assert_eq!(p.settings.height, 1080);
    }

    #[test]
    fn test_canvas_ratio_change() {
        let mut p = Project::default();
        p.set_canvas_ratio(CanvasRatio::Vertical9x16);
        assert_eq!(p.settings.width, 1080);
        assert_eq!(p.settings.height, 1920);
    }

    #[test]
    fn test_project_serialization_roundtrip() {
        let mut p = Project::new("Demo Project");
        p.set_canvas_ratio(CanvasRatio::Square1x1);
        let serialized = serde_json::to_string(&p).expect("Serialization failed");
        let deserialized: Project =
            serde_json::from_str(&serialized).expect("Deserialization failed");
        assert_eq!(p, deserialized);
    }
}
