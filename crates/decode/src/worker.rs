//! Background playback and decode worker thread with coalesced seeking.

use crate::frame::DecodedVideoFrame;
use crossbeam_channel::{bounded, Receiver, Sender};
use ffmpeg_next as ffmpeg;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::info;

/// Commands sent to the playback worker.
#[derive(Debug, Clone)]
pub enum PlaybackCommand {
    Load(PathBuf),
    Play { speed: f64 },
    Pause,
    Seek { timestamp_sec: f64 },
    StepFrame { forward: bool },
    Close,
}

/// Events emitted from playback worker to the UI / presentation layer.
#[derive(Debug, Clone)]
pub enum PlaybackEvent {
    Loaded {
        duration_sec: f64,
        width: u32,
        height: u32,
        fps: f64,
    },
    Frame(DecodedVideoFrame),
    StateChanged(PlaybackState),
    Eof,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Playing,
    Paused,
    Seeking,
}

/// Controller handle for managing background playback and decoding.
pub struct PlaybackController {
    cmd_tx: Option<Sender<PlaybackCommand>>,
    event_rx: Receiver<PlaybackEvent>,
    shutdown_flag: Arc<AtomicBool>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl PlaybackController {
    /// Spawn the playback decode worker thread.
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = bounded::<PlaybackCommand>(128);
        let (event_tx, event_rx) = bounded::<PlaybackEvent>(128);
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&shutdown_flag);

        let worker_handle = thread::Builder::new()
            .name("fluxcut-decode-worker".to_string())
            .spawn(move || {
                info!("Playback decode worker started");
                let mut worker_state = DecodeWorkerState::new(cmd_rx, event_tx, flag_clone);
                worker_state.run();
                info!("Playback decode worker exited cleanly");
            })
            .expect("Failed to spawn decode worker thread");

        Self {
            cmd_tx: Some(cmd_tx),
            event_rx,
            shutdown_flag,
            worker_handle: Some(worker_handle),
        }
    }

    /// Load a media file for playback.
    pub fn load(&self, path: PathBuf) -> Result<(), String> {
        self.send_cmd(PlaybackCommand::Load(path))
    }

    /// Start playback at specified speed (1.0 = normal).
    pub fn play(&self, speed: f64) -> Result<(), String> {
        self.send_cmd(PlaybackCommand::Play { speed })
    }

    /// Pause playback.
    pub fn pause(&self) -> Result<(), String> {
        self.send_cmd(PlaybackCommand::Pause)
    }

    /// Seek to target timestamp with automatic coalescing of rapid seek requests.
    pub fn seek(&self, timestamp_sec: f64) -> Result<(), String> {
        self.send_cmd(PlaybackCommand::Seek { timestamp_sec })
    }

    /// Step single frame forward or backward.
    pub fn step_frame(&self, forward: bool) -> Result<(), String> {
        self.send_cmd(PlaybackCommand::StepFrame { forward })
    }

    pub fn try_recv(&self) -> Option<PlaybackEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn receiver(&self) -> Receiver<PlaybackEvent> {
        self.event_rx.clone()
    }

    fn send_cmd(&self, cmd: PlaybackCommand) -> Result<(), String> {
        if let Some(ref tx) = self.cmd_tx {
            tx.send(cmd)
                .map_err(|e| format!("Worker channel closed: {}", e))
        } else {
            Err("PlaybackController has been shut down".to_string())
        }
    }
}

impl Default for PlaybackController {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PlaybackController {
    fn drop(&mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        self.cmd_tx.take(); // explicitly drop sender to unblock receiver
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

struct DecodeWorkerState {
    cmd_rx: Receiver<PlaybackCommand>,
    event_tx: Sender<PlaybackEvent>,
    shutdown_flag: Arc<AtomicBool>,
    input_ctx: Option<ffmpeg::format::context::Input>,
    decoder: Option<ffmpeg::decoder::Video>,
    scaler: Option<ffmpeg::software::scaling::Context>,
    stream_index: usize,
    time_base: ffmpeg::Rational,
    fps: f64,
    duration_sec: f64,
    current_pts_sec: f64,
    is_playing: bool,
    speed: f64,
}

impl DecodeWorkerState {
    fn new(
        cmd_rx: Receiver<PlaybackCommand>,
        event_tx: Sender<PlaybackEvent>,
        shutdown_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            cmd_rx,
            event_tx,
            shutdown_flag,
            input_ctx: None,
            decoder: None,
            scaler: None,
            stream_index: 0,
            time_base: ffmpeg::Rational(1, 1000),
            fps: 30.0,
            duration_sec: 0.0,
            current_pts_sec: 0.0,
            is_playing: false,
            speed: 1.0,
        }
    }

    fn run(&mut self) {
        let _ = fluxcut_ffmpeg_core::init();

        while !self.shutdown_flag.load(Ordering::Relaxed) {
            if self.is_playing && self.input_ctx.is_some() {
                // Playback loop: poll commands with zero timeout, then decode next frame
                if let Ok(cmd) = self.cmd_rx.try_recv() {
                    self.handle_command(cmd);
                } else {
                    let frame_start = Instant::now();
                    let target_frame_duration =
                        Duration::from_secs_f64(1.0 / (self.fps * self.speed).max(1.0));

                    if !self.decode_and_send_next_frame() {
                        self.is_playing = false;
                        let _ = self.event_tx.send(PlaybackEvent::Eof);
                        let _ = self
                            .event_tx
                            .send(PlaybackEvent::StateChanged(PlaybackState::Paused));
                    } else {
                        let elapsed = frame_start.elapsed();
                        if elapsed < target_frame_duration {
                            thread::sleep(target_frame_duration - elapsed);
                        }
                    }
                }
            } else {
                // Paused/Idle loop: wait for next command with timeout
                match self.cmd_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(cmd) => self.handle_command(cmd),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: PlaybackCommand) {
        match cmd {
            PlaybackCommand::Load(path) => self.load_media(&path),
            PlaybackCommand::Play { speed } => {
                self.is_playing = true;
                self.speed = speed.max(0.1);
                let _ = self
                    .event_tx
                    .send(PlaybackEvent::StateChanged(PlaybackState::Playing));
            }
            PlaybackCommand::Pause => {
                self.is_playing = false;
                let _ = self
                    .event_tx
                    .send(PlaybackEvent::StateChanged(PlaybackState::Paused));
            }
            PlaybackCommand::Seek { mut timestamp_sec } => {
                // Coalesced seeking: drain any subsequent seek commands in queue
                while let Ok(PlaybackCommand::Seek {
                    timestamp_sec: newer_ts,
                }) = self.cmd_rx.try_recv()
                {
                    timestamp_sec = newer_ts;
                }
                self.seek_to(timestamp_sec);
            }
            PlaybackCommand::StepFrame { forward } => {
                self.is_playing = false;
                let delta = if forward {
                    1.0 / self.fps
                } else {
                    -(1.0 / self.fps)
                };
                let target = (self.current_pts_sec + delta).clamp(0.0, self.duration_sec);
                self.seek_to(target);
                let _ = self
                    .event_tx
                    .send(PlaybackEvent::StateChanged(PlaybackState::Paused));
            }
            PlaybackCommand::Close => {
                self.input_ctx = None;
                self.decoder = None;
                self.scaler = None;
                self.is_playing = false;
                let _ = self
                    .event_tx
                    .send(PlaybackEvent::StateChanged(PlaybackState::Idle));
            }
        }
    }

    fn load_media(&mut self, path: &Path) {
        self.is_playing = false;
        match ffmpeg::format::input(&path) {
            Ok(ictx) => {
                if let Some(stream) = ictx.streams().best(ffmpeg::media::Type::Video) {
                    let stream_idx = stream.index();
                    let tb = stream.time_base();
                    let rate = stream.rate();
                    let fps = if rate.denominator() > 0 {
                        (rate.numerator() as f64) / (rate.denominator() as f64)
                    } else {
                        30.0
                    };
                    let duration_sec = if stream.duration() > 0 {
                        (stream.duration() as f64) * (tb.numerator() as f64)
                            / (tb.denominator() as f64)
                    } else {
                        (ictx.duration() as f64) / (ffmpeg::ffi::AV_TIME_BASE as f64)
                    };

                    match ffmpeg::codec::context::Context::from_parameters(stream.parameters())
                        .and_then(|c| c.decoder().video())
                    {
                        Ok(dec) => {
                            let width = dec.width();
                            let height = dec.height();
                            let format = dec.format();

                            let scaler = ffmpeg::software::scaling::Context::get(
                                format,
                                width,
                                height,
                                ffmpeg::format::Pixel::RGBA,
                                width,
                                height,
                                ffmpeg::software::scaling::Flags::BILINEAR,
                            )
                            .ok();

                            self.input_ctx = Some(ictx);
                            self.decoder = Some(dec);
                            self.scaler = scaler;
                            self.stream_index = stream_idx;
                            self.time_base = tb;
                            self.fps = fps;
                            self.duration_sec = duration_sec.max(0.0);
                            self.current_pts_sec = 0.0;

                            let _ = self.event_tx.send(PlaybackEvent::Loaded {
                                duration_sec: self.duration_sec,
                                width,
                                height,
                                fps,
                            });

                            // Display first frame
                            self.seek_to(0.0);
                            let _ = self
                                .event_tx
                                .send(PlaybackEvent::StateChanged(PlaybackState::Paused));
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(PlaybackEvent::Error(format!("Decoder error: {}", e)));
                        }
                    }
                } else {
                    let _ = self
                        .event_tx
                        .send(PlaybackEvent::Error("No video stream found".to_string()));
                }
            }
            Err(e) => {
                let _ = self
                    .event_tx
                    .send(PlaybackEvent::Error(format!("Open input error: {}", e)));
            }
        }
    }

    fn seek_to(&mut self, timestamp_sec: f64) {
        let tb = self.time_base;
        let target_stream = self.stream_index;

        let (ictx, decoder) = match (&mut self.input_ctx, &mut self.decoder) {
            (Some(i), Some(d)) => (i, d),
            _ => return,
        };

        let target_pts =
            (timestamp_sec * (tb.denominator() as f64) / (tb.numerator() as f64)).round() as i64;

        let _ = ictx.seek(target_pts, ..target_pts);
        decoder.flush();

        let mut last_decoded_pts = None;
        let mut target_frame = None;
        let mut decoded_frame = ffmpeg::util::frame::Video::empty();

        // Decode forward from keyframe until we reach target timestamp without redundant scaling
        for (stream, packet) in ictx.packets() {
            if stream.index() == target_stream && decoder.send_packet(&packet).is_ok() {
                let mut reached_target = false;
                while decoder.receive_frame(&mut decoded_frame).is_ok() {
                    let frame_pts = decoded_frame.pts().unwrap_or(0) as f64
                        * (tb.numerator() as f64)
                        / (tb.denominator() as f64);
                    last_decoded_pts = Some(frame_pts);
                    target_frame = Some(decoded_frame.clone());

                    if frame_pts >= timestamp_sec {
                        reached_target = true;
                        break;
                    }
                }
                if reached_target {
                    break;
                }
            }
        }

        // Only scale and emit the single target frame at seek destination
        if let (Some(frame), Some(pts)) = (target_frame, last_decoded_pts) {
            self.current_pts_sec = pts;
            Self::emit_frame_internal(&mut self.scaler, &self.event_tx, pts, &frame);
        }
    }

    fn decode_and_send_next_frame(&mut self) -> bool {
        let tb = self.time_base;
        let target_stream = self.stream_index;

        let (ictx, decoder) = match (&mut self.input_ctx, &mut self.decoder) {
            (Some(i), Some(d)) => (i, d),
            _ => return false,
        };

        let mut decoded_frame = ffmpeg::util::frame::Video::empty();

        for (stream, packet) in ictx.packets() {
            if stream.index() == target_stream
                && decoder.send_packet(&packet).is_ok()
                && decoder.receive_frame(&mut decoded_frame).is_ok()
            {
                let frame_pts = decoded_frame.pts().unwrap_or(0) as f64 * (tb.numerator() as f64)
                    / (tb.denominator() as f64);

                self.current_pts_sec = frame_pts;
                Self::emit_frame_internal(
                    &mut self.scaler,
                    &self.event_tx,
                    frame_pts,
                    &decoded_frame,
                );
                return true;
            }
        }

        // Flush decoder
        let _ = decoder.send_eof();
        if decoder.receive_frame(&mut decoded_frame).is_ok() {
            Self::emit_frame_internal(
                &mut self.scaler,
                &self.event_tx,
                self.current_pts_sec,
                &decoded_frame,
            );
            return true;
        }

        false
    }

    fn emit_frame_internal(
        scaler: &mut Option<ffmpeg::software::scaling::Context>,
        event_tx: &Sender<PlaybackEvent>,
        pts_sec: f64,
        frame: &ffmpeg::util::frame::Video,
    ) {
        let scaler = match scaler.as_mut() {
            Some(s) => s,
            None => return,
        };

        let mut rgba_frame = ffmpeg::util::frame::Video::empty();
        if scaler.run(frame, &mut rgba_frame).is_ok() {
            let width = rgba_frame.width();
            let height = rgba_frame.height();
            let stride = rgba_frame.stride(0);
            let plane = rgba_frame.data(0);
            let mut data = Vec::with_capacity((width * height * 4) as usize);

            for y in 0..height as usize {
                let start = y * stride;
                let end = start + (width as usize * 4);
                if end <= plane.len() {
                    data.extend_from_slice(&plane[start..end]);
                }
            }

            let video_frame = DecodedVideoFrame::new(pts_sec, width, height, data, frame.is_key());
            let _ = event_tx.send(PlaybackEvent::Frame(video_frame));
        }
    }
}
