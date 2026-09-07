//! Internal EditorGraph Abstract Syntax Tree (AST) representing the non-linear editing graph.
//!
//! Independent of FFmpeg string syntax, providing type-safe nodes for video and audio
//! transformation, compositing, scaling, audio mixing, and volume envelopes.

use fluxcut_project::{CropRect, FitMode, ImageTemplate, TimeRational, Transform};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Complete non-linear editing media graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorGraph {
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub fit_mode: FitMode,
    pub total_duration: TimeRational,
    pub video_tracks: Vec<EditorVideoTrack>,
    pub audio_tracks: Vec<EditorAudioTrack>,
}

/// Video track containing sequential or layered video/image clips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorVideoTrack {
    pub id: String,
    pub clips: Vec<EditorVideoClipNode>,
}

/// Node representing a video or image slice on the timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorVideoClipNode {
    pub clip_id: String,
    pub asset_id: String,
    pub file_path: PathBuf,
    pub timeline_start: TimeRational,
    pub timeline_duration: TimeRational,
    pub source_in: TimeRational,
    pub source_out: TimeRational,
    pub speed: f32,
    pub crop: Option<CropRect>,
    pub transform: Option<Transform>,
    pub template: Option<ImageTemplate>,
    pub is_still_image: bool,
}

/// Audio track containing sequential or layered audio clips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorAudioTrack {
    pub id: String,
    pub track_volume: f32,
    pub track_muted: bool,
    pub clips: Vec<EditorAudioClipNode>,
}

/// Node representing an audio slice with volume, mute, and fade envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorAudioClipNode {
    pub clip_id: String,
    pub asset_id: String,
    pub file_path: PathBuf,
    pub timeline_start: TimeRational,
    pub timeline_duration: TimeRational,
    pub source_in: TimeRational,
    pub source_out: TimeRational,
    pub clip_volume: f32,
    pub clip_muted: bool,
    pub fade_in: Option<TimeRational>,
    pub fade_out: Option<TimeRational>,
}
