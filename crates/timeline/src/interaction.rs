//! Timeline mouse interactions, drag handling, and hit testing.

use crate::coords::{TimelineCoords, RULER_HEIGHT, TRIM_HANDLE_WIDTH};
use fluxcut_project::{Project, TimeRational, TrackDefinition};

#[derive(Debug, Clone, PartialEq)]
pub enum HitTarget {
    None,
    Ruler,
    ClipBody { track_index: usize, clip_id: String },
    ClipTrimLeft { track_index: usize, clip_id: String },
    ClipTrimRight { track_index: usize, clip_id: String },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum DragState {
    #[default]
    None,
    ScrubbingPlayhead,
    MovingClip {
        clip_id: String,
        initial_mouse_x: f32,
        initial_start: TimeRational,
    },
    TrimmingLeft {
        clip_id: String,
        initial_mouse_x: f32,
        initial_start: TimeRational,
        initial_in: TimeRational,
    },
    TrimmingRight {
        clip_id: String,
        initial_mouse_x: f32,
        initial_end: TimeRational,
        initial_out: TimeRational,
    },
}

#[derive(Debug, Clone, Default)]
pub struct TimelineInteractionState {
    pub selected_clip_id: Option<String>,
    pub drag_state: DragState,
    pub hover_target: Option<HitTarget>,
}

impl TimelineInteractionState {
    /// Perform a hit test on the project given mouse coordinates (x, y).
    pub fn hit_test(
        &self,
        x: f32,
        y: f32,
        coords: &TimelineCoords,
        project: &Project,
    ) -> HitTarget {
        if y < RULER_HEIGHT {
            return HitTarget::Ruler;
        }

        if let Some(track_idx) = coords.y_to_track(y, project.tracks.len()) {
            let track: &TrackDefinition = &project.tracks[track_idx];
            for clip in &track.clips {
                let clip_x = coords.time_to_x(clip.timeline_start);
                let clip_w = (coords.time_to_x(clip.timeline_end()) - clip_x).max(4.0);

                if x >= clip_x && x <= clip_x + clip_w {
                    // Check handles
                    if x <= clip_x + TRIM_HANDLE_WIDTH {
                        return HitTarget::ClipTrimLeft {
                            track_index: track_idx,
                            clip_id: clip.id.clone(),
                        };
                    } else if x >= clip_x + clip_w - TRIM_HANDLE_WIDTH {
                        return HitTarget::ClipTrimRight {
                            track_index: track_idx,
                            clip_id: clip.id.clone(),
                        };
                    } else {
                        return HitTarget::ClipBody {
                            track_index: track_idx,
                            clip_id: clip.id.clone(),
                        };
                    }
                }
            }
        }

        HitTarget::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluxcut_project::{ClipDefinition, TrackDefinition, TrackKind};

    fn make_test_project() -> Project {
        let mut p = Project::default();
        p.tracks.clear();
        p.tracks.push(TrackDefinition {
            id: "v1".to_string(),
            name: "Video 1".to_string(),
            kind: TrackKind::Video,
            muted: false,
            volume: 1.0,
            clips: vec![ClipDefinition {
                id: "clip-1".to_string(),
                asset_id: "a1".to_string(),
                timeline_start: TimeRational::from_seconds(2.0, 1000),
                in_point: TimeRational::ZERO,
                out_point: TimeRational::from_seconds(6.0, 1000), // duration 4.0s (2.0 to 6.0s)
                volume: 1.0,
                muted: false,
                speed: 1.0,
                ..Default::default()
            }],
        });
        p
    }

    #[test]
    fn test_hit_test_ruler_and_clips() {
        let p = make_test_project();
        let coords = TimelineCoords::new(100.0); // 100 px/sec
        let state = TimelineInteractionState::default();

        // 1. Ruler hit test
        assert_eq!(state.hit_test(50.0, 10.0, &coords, &p), HitTarget::Ruler);

        // 2. Before clip (x at 1.0s = 90 + 100 = 190)
        assert_eq!(state.hit_test(190.0, 40.0, &coords, &p), HitTarget::None);

        // 3. Clip start (x at 2.0s = 90 + 200 = 290)
        let hit_start = state.hit_test(292.0, 40.0, &coords, &p);
        assert_eq!(
            hit_start,
            HitTarget::ClipTrimLeft {
                track_index: 0,
                clip_id: "clip-1".to_string()
            }
        );

        // 4. Clip body (x at 3.0s = 90 + 300 = 390)
        let hit_body = state.hit_test(390.0, 40.0, &coords, &p);
        assert_eq!(
            hit_body,
            HitTarget::ClipBody {
                track_index: 0,
                clip_id: "clip-1".to_string()
            }
        );

        // 5. Clip end (duration is 6.0s, so end is at 2.0 + 6.0 = 8.0s; x = 90 + 800 = 890)
        let hit_end = state.hit_test(888.0, 40.0, &coords, &p);
        assert_eq!(
            hit_end,
            HitTarget::ClipTrimRight {
                track_index: 0,
                clip_id: "clip-1".to_string()
            }
        );
    }
}
