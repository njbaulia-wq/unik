//! High-performance custom timeline rendering via GSK scene graph.
//!
//! Renders tracks, virtualized visible clips, ruler timecode ticks,
//! selection highlights, and playhead cursor.

use crate::coords::{TimelineCoords, RULER_HEIGHT, TRACK_GAP, TRACK_HEIGHT, TRIM_HANDLE_WIDTH};
use crate::interaction::TimelineInteractionState;
use fluxcut_project::{Project, TimeRational, TrackKind};
use gtk4::gdk;
use gtk4::graphene;
use gtk4::gsk;
use gtk4::pango;
use gtk4::prelude::*;

pub struct RenderParams<'a> {
    pub project: &'a Project,
    pub coords: &'a TimelineCoords,
    pub state: &'a TimelineInteractionState,
    pub playhead_time: TimeRational,
    pub width: f32,
    pub height: f32,
    pub pango_ctx: &'a pango::Context,
}

pub struct TimelineRenderer;

impl TimelineRenderer {
    pub fn render(snapshot: &gtk4::Snapshot, params: &RenderParams) {
        let project = params.project;
        let coords = params.coords;
        let state = params.state;
        let playhead_time = params.playhead_time;
        let width = params.width;
        let height = params.height;
        let pango_ctx = params.pango_ctx;
        // 1. Clear background
        let bg_color = gdk::RGBA::new(0.08, 0.08, 0.10, 1.0);
        let bounds = graphene::Rect::new(0.0, 0.0, width, height);
        snapshot.append_color(&bg_color, &bounds);

        // 2. Render track lanes and track headers
        let lane_bg_dark = gdk::RGBA::new(0.12, 0.12, 0.14, 1.0);
        let lane_bg_light = gdk::RGBA::new(0.14, 0.14, 0.16, 1.0);
        let header_bg = gdk::RGBA::new(0.16, 0.16, 0.19, 1.0);
        let separator_color = gdk::RGBA::new(0.22, 0.22, 0.25, 1.0);
        let text_color = gdk::RGBA::new(0.85, 0.85, 0.88, 1.0);

        for (idx, track) in project.tracks.iter().enumerate() {
            let y = coords.track_to_y(idx);
            let lane_rect = graphene::Rect::new(
                coords.header_width,
                y,
                (width - coords.header_width).max(1.0),
                TRACK_HEIGHT,
            );
            let lane_color = if idx % 2 == 0 {
                &lane_bg_dark
            } else {
                &lane_bg_light
            };
            snapshot.append_color(lane_color, &lane_rect);

            // Track Header
            let header_rect = graphene::Rect::new(0.0, y, coords.header_width, TRACK_HEIGHT);
            snapshot.append_color(&header_bg, &header_rect);

            // Separator between header and tracks
            let sep_rect = graphene::Rect::new(coords.header_width - 1.0, y, 1.0, TRACK_HEIGHT);
            snapshot.append_color(&separator_color, &sep_rect);

            // Bottom border between tracks
            let bottom_sep = graphene::Rect::new(0.0, y + TRACK_HEIGHT, width, TRACK_GAP);
            snapshot.append_color(&bg_color, &bottom_sep);

            // Track label layout
            let layout = pango::Layout::new(pango_ctx);
            layout.set_text(&track.name);
            snapshot.save();
            snapshot.translate(&graphene::Point::new(8.0, y + 16.0));
            snapshot.append_layout(&layout, &text_color);
            snapshot.restore();
        }

        // 3. Viewport virtualization: determine visible time window
        let visible_start = coords.x_to_time(0.0);
        let visible_end = coords.x_to_time(width);

        // 4. Render visible clips
        let video_clip_bg = gdk::RGBA::new(0.18, 0.42, 0.54, 0.95);
        let audio_clip_bg = gdk::RGBA::new(0.16, 0.46, 0.32, 0.95);
        let clip_text_color = gdk::RGBA::new(0.96, 0.96, 0.98, 1.0);
        let selected_border_color = gdk::RGBA::new(0.30, 0.68, 1.0, 1.0);
        let handle_color = gdk::RGBA::new(1.0, 1.0, 1.0, 0.20);

        for (track_idx, track) in project.tracks.iter().enumerate() {
            let track_y = coords.track_to_y(track_idx);

            for clip in &track.clips {
                // Viewport culling
                if clip.timeline_end() < visible_start || clip.timeline_start > visible_end {
                    continue;
                }

                let clip_x = coords.time_to_x(clip.timeline_start);
                let clip_end_x = coords.time_to_x(clip.timeline_end());
                let clip_w = (clip_end_x - clip_x).max(4.0);
                let clip_h = TRACK_HEIGHT - 8.0;
                let clip_y = track_y + 4.0;

                let clip_rect = graphene::Rect::new(clip_x, clip_y, clip_w, clip_h);
                let rounded = gsk::RoundedRect::from_rect(clip_rect, 4.0);

                // Rounded background
                snapshot.push_rounded_clip(&rounded);
                let fill_color = match track.kind {
                    TrackKind::Video => &video_clip_bg,
                    TrackKind::Audio => &audio_clip_bg,
                };
                snapshot.append_color(fill_color, &clip_rect);

                // Trim handles visualization
                if clip_w >= TRIM_HANDLE_WIDTH * 2.0 {
                    let left_handle =
                        graphene::Rect::new(clip_x, clip_y, TRIM_HANDLE_WIDTH, clip_h);
                    snapshot.append_color(&handle_color, &left_handle);

                    let right_handle = graphene::Rect::new(
                        clip_x + clip_w - TRIM_HANDLE_WIDTH,
                        clip_y,
                        TRIM_HANDLE_WIDTH,
                        clip_h,
                    );
                    snapshot.append_color(&handle_color, &right_handle);
                }

                // Audio waveform visualization
                if track.kind == TrackKind::Audio && clip_w > 16.0 {
                    let waveform_color = gdk::RGBA::new(0.45, 0.88, 0.60, 0.70);
                    let mid_y = clip_y + clip_h / 2.0;
                    let max_amp = clip_h * 0.40;
                    let step = 3.0f32;
                    let mut px = clip_x + TRIM_HANDLE_WIDTH;
                    let end_px = clip_x + clip_w - TRIM_HANDLE_WIDTH;

                    while px < end_px {
                        let factor =
                            (px * 0.07).sin().abs() * 0.75 + (px * 0.17).cos().abs() * 0.25;
                        let bar_h = (factor * max_amp).max(2.0);
                        let bar_rect = graphene::Rect::new(px, mid_y - bar_h, 2.0, bar_h * 2.0);
                        snapshot.append_color(&waveform_color, &bar_rect);
                        px += step;
                    }
                }

                // Clip title text (asset name or ID)
                if clip_w > 40.0 {
                    let display_name = project
                        .assets
                        .iter()
                        .find(|a| a.id == clip.asset_id)
                        .map(|a| a.display_name.as_str())
                        .unwrap_or(&clip.id);

                    let layout = pango::Layout::new(pango_ctx);
                    layout.set_text(display_name);
                    layout.set_width((((clip_w - 20.0).max(10.0)) * pango::SCALE as f32) as i32);
                    layout.set_ellipsize(pango::EllipsizeMode::End);

                    snapshot.save();
                    snapshot.translate(&graphene::Point::new(clip_x + 10.0, clip_y + 6.0));
                    snapshot.append_layout(&layout, &clip_text_color);
                    snapshot.restore();
                }

                snapshot.pop(); // pop rounded clip

                // Selection outline if selected
                if state.selected_clip_id.as_deref() == Some(&clip.id) {
                    let border_widths = [2.0, 2.0, 2.0, 2.0];
                    let border_colors = [
                        selected_border_color,
                        selected_border_color,
                        selected_border_color,
                        selected_border_color,
                    ];
                    snapshot.append_border(&rounded, &border_widths, &border_colors);
                }
            }
        }

        // 5. Timeline Ruler (top bar)
        let ruler_bg = gdk::RGBA::new(0.10, 0.10, 0.12, 1.0);
        let ruler_rect = graphene::Rect::new(0.0, 0.0, width, RULER_HEIGHT);
        snapshot.append_color(&ruler_bg, &ruler_rect);

        // Ruler bottom border
        let ruler_bottom = graphene::Rect::new(0.0, RULER_HEIGHT - 1.0, width, 1.0);
        snapshot.append_color(&separator_color, &ruler_bottom);

        // Ticks and timecode labels
        let (major_step_sec, minor_step_sec) = if coords.px_per_sec >= 120.0 {
            (1.0, 0.2)
        } else if coords.px_per_sec >= 45.0 {
            (2.0, 0.5)
        } else if coords.px_per_sec >= 20.0 {
            (5.0, 1.0)
        } else {
            (10.0, 2.0)
        };

        let start_sec = (visible_start.to_seconds() / minor_step_sec).floor() * minor_step_sec;
        let end_sec = visible_end.to_seconds() + major_step_sec;

        let tick_color = gdk::RGBA::new(0.55, 0.55, 0.60, 1.0);
        let major_tick_color = gdk::RGBA::new(0.80, 0.80, 0.85, 1.0);

        let mut current_sec = start_sec;
        while current_sec <= end_sec {
            let t = TimeRational::from_seconds(current_sec, 1000);
            let tick_x = coords.time_to_x(t);

            if tick_x >= coords.header_width && tick_x <= width {
                let is_major = (current_sec % major_step_sec).abs() < 0.001;
                let tick_h = if is_major { 12.0 } else { 6.0 };
                let tick_y = RULER_HEIGHT - tick_h;
                let col = if is_major {
                    &major_tick_color
                } else {
                    &tick_color
                };

                let tick_rect = graphene::Rect::new(tick_x, tick_y, 1.0, tick_h);
                snapshot.append_color(col, &tick_rect);

                if is_major {
                    let mins = (current_sec / 60.0).floor() as u64;
                    let secs = (current_sec % 60.0) as u64;
                    let label_text = format!("{:02}:{:02}", mins, secs);

                    let layout = pango::Layout::new(pango_ctx);
                    layout.set_text(&label_text);

                    snapshot.save();
                    snapshot.translate(&graphene::Point::new(tick_x + 3.0, 2.0));
                    snapshot.append_layout(&layout, &tick_color);
                    snapshot.restore();
                }
            }
            current_sec += minor_step_sec;
        }

        // 6. Playhead cursor line and top scrubber cap
        let playhead_x = coords.time_to_x(playhead_time);
        if playhead_x >= coords.header_width && playhead_x <= width {
            let playhead_color = gdk::RGBA::new(0.95, 0.28, 0.28, 1.0);

            // Vertical cursor line down through all tracks
            let cursor_line = graphene::Rect::new(playhead_x - 1.0, 0.0, 2.0, height);
            snapshot.append_color(&playhead_color, &cursor_line);

            // Scrubber head cap on ruler (width 12px, height 12px)
            let cap_rect = graphene::Rect::new(playhead_x - 5.0, 2.0, 10.0, 14.0);
            let cap_rounded = gsk::RoundedRect::from_rect(cap_rect, 2.0);
            snapshot.push_rounded_clip(&cap_rounded);
            snapshot.append_color(&playhead_color, &cap_rect);
            snapshot.pop();
        }
    }
}
