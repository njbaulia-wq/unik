//! Non-destructive timeline editing operations.
//!
//! Provides transactional mutations on tracks and clips: adding, moving,
//! trimming, splitting at playhead, deleting, ripple deleting, and snapping.

use crate::error::ProjectError;
use crate::schema::{ClipDefinition, Project, TrackDefinition};
use crate::time::TimeRational;

impl Project {
    /// Add a clip to the specified track.
    pub fn add_clip(&mut self, track_id: &str, clip: ClipDefinition) -> Result<(), ProjectError> {
        let track = self
            .tracks
            .iter_mut()
            .find(|t| t.id == track_id)
            .ok_or_else(|| ProjectError::TrackNotFound(track_id.to_string()))?;

        track.clips.push(clip);
        track.clips.sort_by_key(|c| c.timeline_start);
        Ok(())
    }

    /// Locate a clip and its track by clip ID.
    pub fn find_clip(&self, clip_id: &str) -> Option<(&TrackDefinition, &ClipDefinition)> {
        for track in &self.tracks {
            if let Some(clip) = track.clips.iter().find(|c| c.id == clip_id) {
                return Some((track, clip));
            }
        }
        None
    }

    /// Remove a clip by ID from whichever track contains it.
    pub fn remove_clip(&mut self, clip_id: &str) -> Result<(String, ClipDefinition), ProjectError> {
        for track in &mut self.tracks {
            if let Some(idx) = track.clips.iter().position(|c| c.id == clip_id) {
                let clip = track.clips.remove(idx);
                return Ok((track.id.clone(), clip));
            }
        }
        Err(ProjectError::ClipNotFound(clip_id.to_string()))
    }

    /// Split a clip at a specific timeline timestamp without re-encoding media.
    /// Returns the updated original clip (head) and the newly created clip (tail).
    pub fn split_clip(
        &mut self,
        clip_id: &str,
        split_time: TimeRational,
    ) -> Result<(ClipDefinition, ClipDefinition), ProjectError> {
        for track in &mut self.tracks {
            if let Some(idx) = track.clips.iter().position(|c| c.id == clip_id) {
                let clip = &track.clips[idx];

                if split_time <= clip.timeline_start || split_time >= clip.timeline_end() {
                    return Err(ProjectError::InvalidOperation(format!(
                        "Split time {} is outside clip boundary [{} .. {}]",
                        split_time,
                        clip.timeline_start,
                        clip.timeline_end()
                    )));
                }

                let offset = split_time - clip.timeline_start;
                let original_out = clip.out_point;

                // Mutate head clip
                let head_out = clip.in_point + offset;
                track.clips[idx].out_point = head_out;
                let head_clip = track.clips[idx].clone();

                // Construct tail clip preserving clip properties
                let mut tail_clip = head_clip.clone();
                tail_clip.id = format!("{}_split_{}", clip_id, split_time.num);
                tail_clip.timeline_start = split_time;
                tail_clip.in_point = head_out;
                tail_clip.out_point = original_out;

                track.clips.insert(idx + 1, tail_clip.clone());
                return Ok((head_clip, tail_clip));
            }
        }
        Err(ProjectError::ClipNotFound(clip_id.to_string()))
    }

    /// Trim the in-point (start) of a clip to a new timeline timestamp.
    pub fn trim_clip_start(
        &mut self,
        clip_id: &str,
        new_start: TimeRational,
    ) -> Result<(), ProjectError> {
        for track in &mut self.tracks {
            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                if new_start >= clip.timeline_end() {
                    return Err(ProjectError::InvalidOperation(
                        "New start timestamp must be before the clip out-point".to_string(),
                    ));
                }

                let delta = new_start - clip.timeline_start;
                let new_in_point = clip.in_point + delta;

                if new_in_point < TimeRational::ZERO {
                    return Err(ProjectError::InvalidOperation(
                        "New in-point cannot be negative".to_string(),
                    ));
                }

                clip.timeline_start = new_start;
                clip.in_point = new_in_point;
                track.clips.sort_by_key(|c| c.timeline_start);
                return Ok(());
            }
        }
        Err(ProjectError::ClipNotFound(clip_id.to_string()))
    }

    /// Trim the out-point (end) of a clip to a new timeline timestamp.
    pub fn trim_clip_end(
        &mut self,
        clip_id: &str,
        new_end: TimeRational,
    ) -> Result<(), ProjectError> {
        for track in &mut self.tracks {
            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                if new_end <= clip.timeline_start {
                    return Err(ProjectError::InvalidOperation(
                        "New end timestamp must be after the clip start".to_string(),
                    ));
                }

                let new_duration = new_end - clip.timeline_start;
                clip.out_point = clip.in_point + new_duration;
                return Ok(());
            }
        }
        Err(ProjectError::ClipNotFound(clip_id.to_string()))
    }

    /// Move a clip to a new track and/or new timeline start.
    pub fn move_clip(
        &mut self,
        clip_id: &str,
        target_track_id: &str,
        new_start: TimeRational,
    ) -> Result<(), ProjectError> {
        let (_, mut clip) = self.remove_clip(clip_id)?;
        clip.timeline_start = new_start.max(TimeRational::ZERO);
        self.add_clip(target_track_id, clip)?;
        Ok(())
    }

    /// Ripple delete a clip: remove the clip and shift following clips left to close the gap.
    pub fn ripple_delete_clip(&mut self, clip_id: &str) -> Result<(), ProjectError> {
        for track in &mut self.tracks {
            if let Some(idx) = track.clips.iter().position(|c| c.id == clip_id) {
                let clip = track.clips.remove(idx);
                let removed_start = clip.timeline_start;
                let removed_dur = clip.duration();

                // Ripple shift all subsequent clips on this track
                for follower in track.clips.iter_mut().skip(idx) {
                    if follower.timeline_start >= removed_start + removed_dur {
                        follower.timeline_start = follower.timeline_start - removed_dur;
                    }
                }
                return Ok(());
            }
        }
        Err(ProjectError::ClipNotFound(clip_id.to_string()))
    }

    /// Compute total project duration across all tracks.
    pub fn total_duration(&self) -> TimeRational {
        let mut max_dur = TimeRational::ZERO;
        for track in &self.tracks {
            for clip in &track.clips {
                let end = clip.timeline_end();
                if end > max_dur {
                    max_dur = end;
                }
            }
        }
        max_dur
    }

    /// Snap a target timestamp to the nearest clip boundary or zero.
    pub fn snap_time(
        &self,
        target_time: TimeRational,
        threshold: TimeRational,
        exclude_clip_id: Option<&str>,
    ) -> TimeRational {
        let mut best_snap = target_time;
        let mut min_diff = threshold + TimeRational::new(1, 1000);

        // Check timeline start
        let diff_zero = target_time.abs();
        if diff_zero <= threshold && diff_zero < min_diff {
            min_diff = diff_zero;
            best_snap = TimeRational::ZERO;
        }

        // Check clip boundaries across all tracks
        for track in &self.tracks {
            for clip in &track.clips {
                if let Some(ex) = exclude_clip_id {
                    if clip.id == ex {
                        continue;
                    }
                }

                // Snap to clip start
                let diff_start = (clip.timeline_start - target_time).abs();
                if diff_start <= threshold && diff_start < min_diff {
                    min_diff = diff_start;
                    best_snap = clip.timeline_start;
                }

                // Snap to clip end
                let diff_end = (clip.timeline_end() - target_time).abs();
                if diff_end <= threshold && diff_end < min_diff {
                    min_diff = diff_end;
                    best_snap = clip.timeline_end();
                }
            }
        }

        best_snap
    }

    /// Virtualization query: collect clips intersecting a visible timeline range [start, end].
    pub fn clips_in_range(
        &self,
        start: TimeRational,
        end: TimeRational,
    ) -> Vec<(&TrackDefinition, &ClipDefinition)> {
        let mut result = Vec::new();
        for track in &self.tracks {
            for clip in &track.clips {
                if clip.timeline_end() >= start && clip.timeline_start <= end {
                    result.push((track, clip));
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_clip(id: &str, start_sec: f64, dur_sec: f64) -> ClipDefinition {
        ClipDefinition {
            id: id.to_string(),
            asset_id: "asset-1".to_string(),
            timeline_start: TimeRational::from_seconds(start_sec, 1000),
            in_point: TimeRational::ZERO,
            out_point: TimeRational::from_seconds(dur_sec, 1000),
            volume: 1.0,
            muted: false,
            speed: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_add_and_split_clip() {
        let mut p = Project::default();
        let clip = dummy_clip("c1", 0.0, 10.0);
        p.add_clip("track-v1", clip).unwrap();

        assert_eq!(p.total_duration().to_seconds(), 10.0);

        // Split at 4.0s
        let split_time = TimeRational::from_seconds(4.0, 1000);
        let (head, tail) = p.split_clip("c1", split_time).unwrap();

        assert_eq!(head.timeline_start.to_seconds(), 0.0);
        assert_eq!(head.duration().to_seconds(), 4.0);

        assert_eq!(tail.timeline_start.to_seconds(), 4.0);
        assert_eq!(tail.duration().to_seconds(), 6.0);

        let track = p.tracks.iter().find(|t| t.id == "track-v1").unwrap();
        assert_eq!(track.clips.len(), 2);
    }

    #[test]
    fn test_trim_clip_start_and_end() {
        let mut p = Project::default();
        let clip = dummy_clip("c1", 2.0, 8.0);
        p.add_clip("track-v1", clip).unwrap();

        // Trim start from 2.0 to 3.0s
        p.trim_clip_start("c1", TimeRational::from_seconds(3.0, 1000))
            .unwrap();
        let (_, c) = p.find_clip("c1").unwrap();
        assert_eq!(c.timeline_start.to_seconds(), 3.0);
        assert_eq!(c.in_point.to_seconds(), 1.0);
        assert_eq!(c.duration().to_seconds(), 7.0);

        // Trim end to 7.0s
        p.trim_clip_end("c1", TimeRational::from_seconds(7.0, 1000))
            .unwrap();
        let (_, c2) = p.find_clip("c1").unwrap();
        assert_eq!(c2.timeline_end().to_seconds(), 7.0);
        assert_eq!(c2.duration().to_seconds(), 4.0);
    }

    #[test]
    fn test_ripple_delete() {
        let mut p = Project::default();
        p.add_clip("track-v1", dummy_clip("c1", 0.0, 5.0)).unwrap();
        p.add_clip("track-v1", dummy_clip("c2", 5.0, 5.0)).unwrap();
        p.add_clip("track-v1", dummy_clip("c3", 10.0, 4.0)).unwrap();

        // Ripple delete c2 (starts at 5, duration 5)
        p.ripple_delete_clip("c2").unwrap();

        let track = p.tracks.iter().find(|t| t.id == "track-v1").unwrap();
        assert_eq!(track.clips.len(), 2);
        // c3 should now start at 5.0 instead of 10.0!
        assert_eq!(track.clips[1].id, "c3");
        assert_eq!(track.clips[1].timeline_start.to_seconds(), 5.0);
        assert_eq!(p.total_duration().to_seconds(), 9.0);
    }

    #[test]
    fn test_snap_time() {
        let mut p = Project::default();
        p.add_clip("track-v1", dummy_clip("c1", 5.0, 5.0)).unwrap(); // 5.0 to 10.0

        let threshold = TimeRational::from_seconds(0.2, 1000);

        // Near 0.0 -> snap to 0.0
        let s0 = p.snap_time(TimeRational::from_seconds(0.1, 1000), threshold, None);
        assert_eq!(s0.to_seconds(), 0.0);

        // Near 5.0 -> snap to 5.0
        let s5 = p.snap_time(TimeRational::from_seconds(4.9, 1000), threshold, None);
        assert_eq!(s5.to_seconds(), 5.0);

        // Near 10.0 -> snap to 10.0
        let s10 = p.snap_time(TimeRational::from_seconds(10.15, 1000), threshold, None);
        assert_eq!(s10.to_seconds(), 10.0);

        // Far away -> unchanged
        let s_far = p.snap_time(TimeRational::from_seconds(7.5, 1000), threshold, None);
        assert_eq!(s_far.to_seconds(), 7.5);
    }
}
