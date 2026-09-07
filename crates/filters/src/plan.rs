//! Deterministic execution plan and filter graph generation.

use crate::ast::EditorGraph;

/// Compiled execution plan summary.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub mode_name: &'static str,
    pub target_resolution: (u32, u32),
    pub target_fps: f64,
    pub video_filter_desc: Option<String>,
    pub audio_filter_desc: Option<String>,
    pub diagnostic_dump: String,
}

pub fn generate_execution_plan(graph: &EditorGraph, is_fast_path: bool) -> ExecutionPlan {
    let mode_name = if is_fast_path {
        "Fast-Path Stream Copy (Lossless Remux)"
    } else {
        "H.264 Multi-Track Transcode"
    };

    let target_resolution = (graph.canvas_width, graph.canvas_height);
    let target_fps = (graph.fps_num as f64) / (graph.fps_den as f64).max(1.0);

    let mut diag = String::new();
    diag.push_str(&format!("EditorGraph Execution Plan [{}]\n", mode_name));
    diag.push_str(&format!(
        "Canvas: {}x{} @ {:.2} fps, Duration: {:.2}s\n",
        graph.canvas_width,
        graph.canvas_height,
        target_fps,
        graph.total_duration.to_seconds()
    ));

    diag.push_str(&format!("Video Tracks: {}\n", graph.video_tracks.len()));
    for (i, vt) in graph.video_tracks.iter().enumerate() {
        diag.push_str(&format!("  [Track {}] Clips: {}\n", i, vt.clips.len()));
        for c in &vt.clips {
            diag.push_str(&format!(
                "    - Clip {}: timeline [{:.2}s .. {:.2}s] -> source [{:.2}s .. {:.2}s]\n",
                c.clip_id,
                c.timeline_start.to_seconds(),
                (c.timeline_start + c.timeline_duration).to_seconds(),
                c.source_in.to_seconds(),
                c.source_out.to_seconds()
            ));
        }
    }

    diag.push_str(&format!("Audio Tracks: {}\n", graph.audio_tracks.len()));
    for (i, at) in graph.audio_tracks.iter().enumerate() {
        diag.push_str(&format!(
            "  [Track {}] Volume: {:.2}, Muted: {}, Clips: {}\n",
            i,
            at.track_volume,
            at.track_muted,
            at.clips.len()
        ));
        for c in &at.clips {
            diag.push_str(&format!(
                "    - Audio {}: vol={:.2}, muted={}, timeline [{:.2}s .. {:.2}s]\n",
                c.clip_id,
                c.clip_volume,
                c.clip_muted,
                c.timeline_start.to_seconds(),
                (c.timeline_start + c.timeline_duration).to_seconds()
            ));
        }
    }

    let video_filter_desc = if is_fast_path {
        None
    } else {
        Some(format!(
            "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
            graph.canvas_width, graph.canvas_height, graph.canvas_width, graph.canvas_height
        ))
    };

    let audio_filter_desc = if is_fast_path {
        None
    } else {
        Some("amix=inputs=2:duration=longest:dropout_transition=2".to_string())
    };

    ExecutionPlan {
        mode_name,
        target_resolution,
        target_fps,
        video_filter_desc,
        audio_filter_desc,
        diagnostic_dump: diag,
    }
}
