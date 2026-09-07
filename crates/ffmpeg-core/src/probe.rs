//! Media asset probing using FFmpeg native libraries (libavformat & libavcodec).

use crate::error::FfmpegCoreError;
use ffmpeg_next as ffmpeg;
use fluxcut_project::TimeRational;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::debug;

/// Detailed probe information for an individual video stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoStreamProbe {
    pub index: usize,
    pub codec_name: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: TimeRational,
    pub time_base: TimeRational,
    pub aspect_ratio: Option<TimeRational>,
    pub duration_seconds: Option<f64>,
    pub pixel_format: String,
}

/// Detailed probe information for an individual audio stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioStreamProbe {
    pub index: usize,
    pub codec_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_layout: String,
    pub duration_seconds: Option<f64>,
    pub time_base: TimeRational,
}

/// Overall metadata extracted from container format and streams.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaProbeResult {
    pub format_name: String,
    pub duration_seconds: f64,
    pub bit_rate: Option<i64>,
    pub video_streams: Vec<VideoStreamProbe>,
    pub audio_streams: Vec<AudioStreamProbe>,
}

impl MediaProbeResult {
    /// Return the primary/best video stream if available.
    pub fn primary_video_stream(&self) -> Option<&VideoStreamProbe> {
        self.video_streams.first()
    }

    /// Return the primary/best audio stream if available.
    pub fn primary_audio_stream(&self) -> Option<&AudioStreamProbe> {
        self.audio_streams.first()
    }
}

/// Probe a media file path using FFmpeg libavformat.
pub fn probe_file(path: &Path) -> Result<MediaProbeResult, FfmpegCoreError> {
    crate::init()?;

    let ictx = ffmpeg::format::input(&path).map_err(|e| FfmpegCoreError::OpenInputError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let format_name = ictx.format().name().to_string();
    let mut duration_seconds = if ictx.duration() > 0 {
        (ictx.duration() as f64) / (ffmpeg::ffi::AV_TIME_BASE as f64)
    } else {
        0.0
    };
    let bit_rate = if ictx.bit_rate() > 0 {
        Some(ictx.bit_rate())
    } else {
        None
    };

    let mut video_streams = Vec::new();
    let mut audio_streams = Vec::new();

    for stream in ictx.streams() {
        let codec_params = stream.parameters();
        let medium = codec_params.medium();
        let stream_idx = stream.index();
        let tb = stream.time_base();
        let time_base = TimeRational::new(tb.numerator() as i64, tb.denominator().max(1) as u32);

        let stream_dur = if stream.duration() > 0 && tb.denominator() > 0 {
            let d =
                (stream.duration() as f64) * (tb.numerator() as f64) / (tb.denominator() as f64);
            if d > duration_seconds || duration_seconds <= 0.001 {
                duration_seconds = d;
            }
            Some(d)
        } else {
            None
        };

        match medium {
            ffmpeg::media::Type::Video => {
                let context = ffmpeg::codec::context::Context::from_parameters(codec_params);
                if let Ok(video_ctx) = context.and_then(|c| c.decoder().video()) {
                    let r_fps = stream.rate();
                    let frame_rate = TimeRational::new(
                        r_fps.numerator() as i64,
                        r_fps.denominator().max(1) as u32,
                    );

                    let aspect = video_ctx.aspect_ratio();
                    let aspect_ratio = if aspect.numerator() > 0 {
                        Some(TimeRational::new(
                            aspect.numerator() as i64,
                            aspect.denominator().max(1) as u32,
                        ))
                    } else {
                        None
                    };

                    video_streams.push(VideoStreamProbe {
                        index: stream_idx,
                        codec_name: video_ctx
                            .codec()
                            .map(|c| c.name().to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                        width: video_ctx.width(),
                        height: video_ctx.height(),
                        frame_rate,
                        time_base,
                        aspect_ratio,
                        duration_seconds: stream_dur,
                        pixel_format: format!("{:?}", video_ctx.format()),
                    });
                }
            }
            ffmpeg::media::Type::Audio => {
                let context = ffmpeg::codec::context::Context::from_parameters(codec_params);
                if let Ok(audio_ctx) = context.and_then(|c| c.decoder().audio()) {
                    let layout_str = format!("{:?}", audio_ctx.channel_layout());
                    audio_streams.push(AudioStreamProbe {
                        index: stream_idx,
                        codec_name: audio_ctx
                            .codec()
                            .map(|c| c.name().to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                        sample_rate: audio_ctx.rate(),
                        channels: audio_ctx.channels(),
                        channel_layout: layout_str,
                        duration_seconds: stream_dur,
                        time_base,
                    });
                }
            }
            _ => {}
        }
    }

    debug!(
        path = %path.display(),
        video_count = video_streams.len(),
        audio_count = audio_streams.len(),
        duration = duration_seconds,
        "Completed media probe"
    );

    Ok(MediaProbeResult {
        format_name,
        duration_seconds,
        bit_rate,
        video_streams,
        audio_streams,
    })
}
