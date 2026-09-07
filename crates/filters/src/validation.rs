//! EditorGraph validation logic ensuring all invariants and boundary conditions hold.

use crate::ast::EditorGraph;
use fluxcut_project::TimeRational;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum GraphValidationError {
    #[error("Zero canvas dimensions: {0}x{1}")]
    ZeroCanvasDimensions(u32, u32),

    #[error("Empty timeline duration")]
    EmptyTimelineDuration,

    #[error("Clip {clip_id} has invalid source bounds: in={source_in:?}, out={source_out:?}")]
    InvalidSourceBounds {
        clip_id: String,
        source_in: TimeRational,
        source_out: TimeRational,
    },

    #[error("Clip {clip_id} references non-existent media file: {path}")]
    MissingSourceFile { clip_id: String, path: String },
}

pub fn validate_graph(graph: &EditorGraph) -> Result<(), GraphValidationError> {
    if graph.canvas_width == 0 || graph.canvas_height == 0 {
        return Err(GraphValidationError::ZeroCanvasDimensions(
            graph.canvas_width,
            graph.canvas_height,
        ));
    }

    if graph.total_duration <= TimeRational::ZERO {
        return Err(GraphValidationError::EmptyTimelineDuration);
    }

    for track in &graph.video_tracks {
        for clip in &track.clips {
            if clip.source_out <= clip.source_in {
                return Err(GraphValidationError::InvalidSourceBounds {
                    clip_id: clip.clip_id.clone(),
                    source_in: clip.source_in,
                    source_out: clip.source_out,
                });
            }
        }
    }

    for track in &graph.audio_tracks {
        for clip in &track.clips {
            if clip.source_out <= clip.source_in {
                return Err(GraphValidationError::InvalidSourceBounds {
                    clip_id: clip.clip_id.clone(),
                    source_in: clip.source_in,
                    source_out: clip.source_out,
                });
            }
        }
    }

    Ok(())
}
