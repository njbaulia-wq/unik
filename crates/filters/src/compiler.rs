//! Compiler that transforms a user-facing Project into an EditorGraph AST.

use crate::ast::{
    EditorAudioClipNode, EditorAudioTrack, EditorGraph, EditorVideoClipNode, EditorVideoTrack,
};
use crate::validation::{validate_graph, GraphValidationError};
use fluxcut_project::{Project, TrackKind};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct EditorGraphCompiler;

impl EditorGraphCompiler {
    /// Compile a Project model into an EditorGraph.
    pub fn compile(project: &Project) -> Result<EditorGraph, GraphValidationError> {
        let mut asset_map: HashMap<String, (PathBuf, bool, bool)> = HashMap::new();
        for asset in &project.assets {
            asset_map.insert(
                asset.id.clone(),
                (asset.file_path.clone(), asset.has_video, asset.has_audio),
            );
        }

        let mut video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();

        for track in &project.tracks {
            match track.kind {
                TrackKind::Video => {
                    let mut clip_nodes = Vec::new();
                    for clip in &track.clips {
                        let path = asset_map
                            .get(&clip.asset_id)
                            .map(|(p, _, _)| p.clone())
                            .unwrap_or_else(|| PathBuf::from(&clip.asset_id));

                        let has_v = asset_map
                            .get(&clip.asset_id)
                            .map(|(_, v, _)| *v)
                            .unwrap_or(true);

                        let node = EditorVideoClipNode {
                            clip_id: clip.id.clone(),
                            asset_id: clip.asset_id.clone(),
                            file_path: path,
                            timeline_start: clip.timeline_start,
                            timeline_duration: clip.duration(),
                            source_in: clip.in_point,
                            source_out: clip.out_point,
                            speed: clip.speed,
                            crop: clip.crop,
                            transform: clip.transform,
                            template: clip.template,
                            is_still_image: !has_v,
                        };
                        clip_nodes.push(node);
                    }
                    video_tracks.push(EditorVideoTrack {
                        id: track.id.clone(),
                        clips: clip_nodes,
                    });
                }
                TrackKind::Audio => {
                    let mut clip_nodes = Vec::new();
                    for clip in &track.clips {
                        let path = asset_map
                            .get(&clip.asset_id)
                            .map(|(p, _, _)| p.clone())
                            .unwrap_or_else(|| PathBuf::from(&clip.asset_id));

                        let node = EditorAudioClipNode {
                            clip_id: clip.id.clone(),
                            asset_id: clip.asset_id.clone(),
                            file_path: path,
                            timeline_start: clip.timeline_start,
                            timeline_duration: clip.duration(),
                            source_in: clip.in_point,
                            source_out: clip.out_point,
                            clip_volume: clip.volume,
                            clip_muted: clip.muted,
                            fade_in: clip.fade_in,
                            fade_out: clip.fade_out,
                        };
                        clip_nodes.push(node);
                    }
                    audio_tracks.push(EditorAudioTrack {
                        id: track.id.clone(),
                        track_volume: track.volume,
                        track_muted: track.muted,
                        clips: clip_nodes,
                    });
                }
            }
        }

        let total_duration = project.total_duration();

        let graph = EditorGraph {
            canvas_width: project.settings.width,
            canvas_height: project.settings.height,
            fps_num: project.settings.fps_num,
            fps_den: project.settings.fps_den,
            fit_mode: project.settings.fit_mode,
            total_duration,
            video_tracks,
            audio_tracks,
        };

        validate_graph(&graph)?;
        Ok(graph)
    }

    /// Check if the compiled graph is eligible for fast-path stream copy remuxing (Scenario B).
    pub fn is_fast_path_eligible(graph: &EditorGraph) -> bool {
        // Fast path requires exactly one video track with at most one clip, no active audio mixing or overlays
        let video_clips: Vec<&EditorVideoClipNode> = graph
            .video_tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .collect();

        if video_clips.len() != 1 {
            return false;
        }

        let clip = video_clips[0];
        if clip.crop.is_some() || clip.transform.is_some() || clip.template.is_some() {
            return false;
        }

        if (clip.speed - 1.0).abs() > 0.001 {
            return false;
        }

        // Check if audio tracks have multiple clips or volume fades
        let audio_clips: Vec<&EditorAudioClipNode> = graph
            .audio_tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .collect();

        if !audio_clips.is_empty() {
            if audio_clips.len() > 1 {
                return false;
            }
            let a_clip = audio_clips[0];
            if a_clip.fade_in.is_some() || a_clip.fade_out.is_some() {
                return false;
            }
            if (a_clip.clip_volume - 1.0).abs() > 0.01 {
                return false;
            }
        }

        true
    }
}
