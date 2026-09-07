//! Video Presentation Bridge between background decoded frames and GTK4 / Wayland.

use fluxcut_decode::DecodedVideoFrame;
use fluxcut_hardware::GpuCapabilities;
use gtk4::gdk;
use gtk4::glib;
use std::time::Instant;
use tracing::debug;

/// Manages presentation of video frames onto a GTK4 Picture widget.
pub struct VideoPresentationBridge {
    picture: gtk4::Picture,
    #[allow(dead_code)]
    caps: GpuCapabilities,
    frames_presented: u64,
    last_frame_time: Option<Instant>,
    current_fps: f64,
}

impl VideoPresentationBridge {
    pub fn new(picture: gtk4::Picture, caps: GpuCapabilities) -> Self {
        Self {
            picture,
            caps,
            frames_presented: 0,
            last_frame_time: None,
            current_fps: 0.0,
        }
    }

    /// Present a decoded video frame to the GTK4 viewport.
    pub fn present_frame(&mut self, frame: &DecodedVideoFrame) {
        let now = Instant::now();
        if let Some(prev) = self.last_frame_time {
            let delta = now.duration_since(prev).as_secs_f64();
            if delta > 0.001 {
                let instant_fps = 1.0 / delta;
                self.current_fps = (self.current_fps * 0.9) + (instant_fps * 0.1);
            }
        }
        self.last_frame_time = Some(now);
        self.frames_presented += 1;

        let glib_bytes = glib::Bytes::from(frame.rgba_data.as_ref());
        let stride = (frame.width * 4) as usize;

        let texture = gdk::MemoryTexture::new(
            frame.width as i32,
            frame.height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib_bytes,
            stride,
        );

        self.picture.set_paintable(Some(&texture));

        debug!(
            pts = frame.pts_sec,
            width = frame.width,
            height = frame.height,
            fps = self.current_fps,
            "Presented video frame to viewport"
        );
    }

    /// Clear the viewport display.
    pub fn clear(&mut self) {
        self.picture.set_paintable(None::<&gdk::Paintable>);
        self.last_frame_time = None;
        self.current_fps = 0.0;
    }

    pub fn frames_presented(&self) -> u64 {
        self.frames_presented
    }

    pub fn current_fps(&self) -> f64 {
        self.current_fps
    }
}
