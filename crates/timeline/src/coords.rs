//! Timeline coordinate transformations and layout metrics.

use fluxcut_project::TimeRational;

/// Constants for timeline visual geometry.
pub const RULER_HEIGHT: f32 = 28.0;
pub const TRACK_HEIGHT: f32 = 56.0;
pub const TRACK_GAP: f32 = 4.0;
pub const TRACK_HEADER_WIDTH: f32 = 90.0;
pub const TRIM_HANDLE_WIDTH: f32 = 8.0;
pub const MIN_PX_PER_SEC: f32 = 5.0;
pub const MAX_PX_PER_SEC: f32 = 2000.0;
pub const DEFAULT_PX_PER_SEC: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineCoords {
    pub px_per_sec: f32,
    pub header_width: f32,
}

impl Default for TimelineCoords {
    fn default() -> Self {
        Self {
            px_per_sec: DEFAULT_PX_PER_SEC,
            header_width: TRACK_HEADER_WIDTH,
        }
    }
}

impl TimelineCoords {
    pub fn new(px_per_sec: f32) -> Self {
        Self {
            px_per_sec: px_per_sec.clamp(MIN_PX_PER_SEC, MAX_PX_PER_SEC),
            header_width: TRACK_HEADER_WIDTH,
        }
    }

    /// Zoom in by a scale factor.
    pub fn zoom_in(&mut self) {
        self.px_per_sec = (self.px_per_sec * 1.25).clamp(MIN_PX_PER_SEC, MAX_PX_PER_SEC);
    }

    /// Zoom out by a scale factor.
    pub fn zoom_out(&mut self) {
        self.px_per_sec = (self.px_per_sec / 1.25).clamp(MIN_PX_PER_SEC, MAX_PX_PER_SEC);
    }

    /// Convert a timeline rational timestamp to an X pixel coordinate.
    pub fn time_to_x(&self, time: TimeRational) -> f32 {
        self.header_width + (time.to_seconds() as f32 * self.px_per_sec)
    }

    /// Convert an X pixel coordinate to a timeline rational timestamp.
    pub fn x_to_time(&self, x: f32) -> TimeRational {
        let content_x = (x - self.header_width).max(0.0);
        let seconds = (content_x / self.px_per_sec) as f64;
        TimeRational::from_seconds(seconds, 1000)
    }

    /// Calculate the Y coordinate for a given track index.
    pub fn track_to_y(&self, track_index: usize) -> f32 {
        RULER_HEIGHT + (track_index as f32) * (TRACK_HEIGHT + TRACK_GAP)
    }

    /// Determine which track index a Y pixel coordinate falls into.
    pub fn y_to_track(&self, y: f32, track_count: usize) -> Option<usize> {
        if y < RULER_HEIGHT {
            return None;
        }
        let track_area_y = y - RULER_HEIGHT;
        let track_stride = TRACK_HEIGHT + TRACK_GAP;
        let index = (track_area_y / track_stride) as usize;
        if index < track_count {
            Some(index)
        } else {
            None
        }
    }

    /// Calculate snapping threshold in TimeRational for a given pixel tolerance (e.g. 10px).
    pub fn snap_threshold_time(&self, pixel_tolerance: f32) -> TimeRational {
        let sec = (pixel_tolerance / self.px_per_sec) as f64;
        TimeRational::from_seconds(sec, 1000)
    }

    /// Compute total content width needed for a given duration.
    pub fn total_width(&self, duration: TimeRational) -> f32 {
        let track_content = duration.to_seconds() as f32 * self.px_per_sec;
        // Minimum width allows comfortable empty workspace navigation
        self.header_width + track_content.max(800.0) + 120.0
    }

    /// Compute total content height needed for a given track count.
    pub fn total_height(&self, track_count: usize) -> f32 {
        RULER_HEIGHT + (track_count.max(2) as f32) * (TRACK_HEIGHT + TRACK_GAP) + 40.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coords_roundtrip() {
        let coords = TimelineCoords::new(100.0); // 100 px/sec
        let t = TimeRational::from_seconds(2.5, 1000);
        let x = coords.time_to_x(t);
        assert_eq!(x, TRACK_HEADER_WIDTH + 250.0);

        let recovered = coords.x_to_time(x);
        assert_eq!(recovered.to_seconds(), 2.5);
    }

    #[test]
    fn test_track_y_and_hit_test() {
        let coords = TimelineCoords::default();
        let y0 = coords.track_to_y(0);
        assert_eq!(y0, RULER_HEIGHT);

        let y1 = coords.track_to_y(1);
        assert_eq!(y1, RULER_HEIGHT + TRACK_HEIGHT + TRACK_GAP);

        assert_eq!(coords.y_to_track(10.0, 2), None); // in ruler area
        assert_eq!(coords.y_to_track(RULER_HEIGHT + 10.0, 2), Some(0)); // in track 0
        assert_eq!(
            coords.y_to_track(RULER_HEIGHT + TRACK_HEIGHT + TRACK_GAP + 10.0, 2),
            Some(1)
        ); // in track 1
        assert_eq!(coords.y_to_track(9999.0, 2), None); // out of range
    }

    #[test]
    fn test_zoom_clamping() {
        let mut coords = TimelineCoords::new(5.0);
        coords.zoom_out();
        assert_eq!(coords.px_per_sec, MIN_PX_PER_SEC);

        let mut coords_max = TimelineCoords::new(2000.0);
        coords_max.zoom_in();
        assert_eq!(coords_max.px_per_sec, MAX_PX_PER_SEC);
    }
}
