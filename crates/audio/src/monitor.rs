//! Real-time audio playback monitor via cpal.
//!
//! Provides an audio output stream for video/audio preview playback,
//! buffered sample streaming, volume control, and seeking/flush support.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

pub struct AudioMonitor {
    _stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: u32,
    channels: u16,
    is_active: Arc<AtomicBool>,
}

impl AudioMonitor {
    /// Attempts to initialize the default system audio output device.
    /// Returns an AudioMonitor, or an inert fallback if no audio device is available.
    pub fn new() -> Self {
        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(96000)));
        let is_active = Arc::new(AtomicBool::new(true));

        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                warn!("No audio output device found; audio playback will be muted");
                return Self {
                    _stream: None,
                    buffer,
                    sample_rate: 44100,
                    channels: 2,
                    is_active,
                };
            }
        };

        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to query default output audio config: {}", e);
                return Self {
                    _stream: None,
                    buffer,
                    sample_rate: 44100,
                    channels: 2,
                    is_active,
                };
            }
        };

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();

        info!(
            device = ?device.name().unwrap_or_default(),
            sample_rate,
            channels,
            ?sample_format,
            "Initializing cpal audio output monitor"
        );

        let buf_clone = Arc::clone(&buffer);
        let active_clone = Arc::clone(&is_active);

        let err_fn = |err| {
            error!("Audio stream error: {}", err);
        };

        let stream_res = match sample_format {
            cpal::SampleFormat::F32 => {
                let stream_config: cpal::StreamConfig = config.into();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _| {
                        if !active_clone.load(Ordering::Relaxed) {
                            data.fill(0.0);
                            return;
                        }
                        let mut q = buf_clone.lock().unwrap();
                        for sample in data.iter_mut() {
                            *sample = q.pop_front().unwrap_or(0.0);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let stream_config: cpal::StreamConfig = config.into();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _| {
                        if !active_clone.load(Ordering::Relaxed) {
                            data.fill(0);
                            return;
                        }
                        let mut q = buf_clone.lock().unwrap();
                        for sample in data.iter_mut() {
                            let f = q.pop_front().unwrap_or(0.0);
                            *sample = (f * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32)
                                as i16;
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let stream_config: cpal::StreamConfig = config.into();
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _| {
                        if !active_clone.load(Ordering::Relaxed) {
                            data.fill(32768);
                            return;
                        }
                        let mut q = buf_clone.lock().unwrap();
                        for sample in data.iter_mut() {
                            let f = q.pop_front().unwrap_or(0.0);
                            let normalized = (f * 0.5 + 0.5).clamp(0.0, 1.0);
                            *sample = (normalized * u16::MAX as f32) as u16;
                        }
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                warn!("Unsupported audio sample format: {:?}", sample_format);
                return Self {
                    _stream: None,
                    buffer,
                    sample_rate,
                    channels,
                    is_active,
                };
            }
        };

        let stream = match stream_res {
            Ok(s) => {
                if let Err(e) = s.play() {
                    warn!("Failed to start cpal stream: {}", e);
                    None
                } else {
                    Some(s)
                }
            }
            Err(e) => {
                warn!("Failed to build cpal audio output stream: {}", e);
                None
            }
        };

        Self {
            _stream: stream,
            buffer,
            sample_rate,
            channels,
            is_active,
        }
    }

    /// Push interleaved f32 audio samples into the playback buffer.
    /// If buffer grows beyond 1 second (sample_rate * channels), drop oldest samples.
    pub fn push_samples(&self, samples: &[f32]) {
        let max_samples = (self.sample_rate as usize * self.channels as usize).max(44100);
        if let Ok(mut q) = self.buffer.lock() {
            if q.len() + samples.len() > max_samples {
                let overflow = (q.len() + samples.len()) - max_samples;
                let to_drain = overflow.min(q.len());
                q.drain(..to_drain);
            }
            q.extend(samples.iter().copied());
        }
    }

    /// Clear all pending samples in playback buffer (e.g. on seek).
    pub fn clear(&self) {
        if let Ok(mut q) = self.buffer.lock() {
            q.clear();
        }
    }

    /// Enable or pause audio output.
    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::Relaxed);
        if !active {
            self.clear();
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Default for AudioMonitor {
    fn default() -> Self {
        Self::new()
    }
}
