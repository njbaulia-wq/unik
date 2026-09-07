//! Audio waveform envelope extraction using FFmpeg audio decode.

use crate::error::FfmpegCoreError;
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::debug;

/// Audio waveform bin storing min, max, and RMS amplitude values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaveformBin {
    pub min: f32,
    pub max: f32,
    pub rms: f32,
}

/// Waveform envelope containing downsampled bins across the entire audio stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaveformEnvelope {
    pub bins_per_second: u32,
    pub duration_seconds: f64,
    pub bins: Vec<WaveformBin>,
}

/// Extract downsampled waveform envelope bins from an audio or video file.
pub fn extract_waveform(
    path: &Path,
    bins_per_second: u32,
) -> Result<WaveformEnvelope, FfmpegCoreError> {
    crate::init()?;

    let mut ictx = ffmpeg::format::input(&path).map_err(|e| FfmpegCoreError::OpenInputError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or_else(|| FfmpegCoreError::NoAudioStream(path.to_path_buf()))?;

    let stream_index = stream.index();
    let context =
        ffmpeg::codec::context::Context::from_parameters(stream.parameters()).map_err(|e| {
            FfmpegCoreError::DecoderInitError {
                stream_index,
                message: e.to_string(),
            }
        })?;

    let mut decoder = context
        .decoder()
        .audio()
        .map_err(|e| FfmpegCoreError::DecoderInitError {
            stream_index,
            message: e.to_string(),
        })?;

    let sample_rate = decoder.rate();

    // Samples per bin
    let samples_per_bin = ((sample_rate as f64) / (bins_per_second as f64))
        .round()
        .max(1.0) as usize;

    let mut bins = Vec::new();
    let mut current_bin_samples = 0usize;
    let mut current_min = 0.0f32;
    let mut current_max = 0.0f32;
    let mut current_sum_sq = 0.0f64;

    let mut decoded_frame = ffmpeg::util::frame::Audio::empty();

    // Read and decode all audio packets
    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index && decoder.send_packet(&packet).is_ok() {
            while decoder.receive_frame(&mut decoded_frame).is_ok() {
                process_audio_frame(
                    &decoded_frame,
                    samples_per_bin,
                    &mut current_bin_samples,
                    &mut current_min,
                    &mut current_max,
                    &mut current_sum_sq,
                    &mut bins,
                );
            }
        }
    }

    // Flush decoder
    let _ = decoder.send_eof();
    while decoder.receive_frame(&mut decoded_frame).is_ok() {
        process_audio_frame(
            &decoded_frame,
            samples_per_bin,
            &mut current_bin_samples,
            &mut current_min,
            &mut current_max,
            &mut current_sum_sq,
            &mut bins,
        );
    }

    // Flush any remaining sample in final bin
    if current_bin_samples > 0 {
        let rms = ((current_sum_sq / current_bin_samples as f64).sqrt() as f32).min(1.0);
        bins.push(WaveformBin {
            min: current_min,
            max: current_max,
            rms,
        });
    }

    let duration_seconds = (bins.len() as f64) / (bins_per_second as f64);
    debug!(
        path = %path.display(),
        bins_count = bins.len(),
        duration = duration_seconds,
        "Extracted waveform envelope"
    );

    Ok(WaveformEnvelope {
        bins_per_second,
        duration_seconds,
        bins,
    })
}

fn process_audio_frame(
    frame: &ffmpeg::util::frame::Audio,
    samples_per_bin: usize,
    current_bin_samples: &mut usize,
    current_min: &mut f32,
    current_max: &mut f32,
    current_sum_sq: &mut f64,
    bins: &mut Vec<WaveformBin>,
) {
    let samples_count = frame.samples();
    if samples_count == 0 {
        return;
    }

    // Simple sample value extractor for supported common formats (FLT, S16, S32)
    let format = frame.format();
    let plane = frame.data(0);

    for i in 0..samples_count {
        let sample: f32 = match format {
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar)
            | ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed) => {
                let offset = i * 4;
                if offset + 4 <= plane.len() {
                    let bytes: [u8; 4] = plane[offset..offset + 4].try_into().unwrap_or([0; 4]);
                    f32::from_le_bytes(bytes)
                } else {
                    0.0
                }
            }
            ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Planar)
            | ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Packed) => {
                let offset = i * 2;
                if offset + 2 <= plane.len() {
                    let bytes: [u8; 2] = plane[offset..offset + 2].try_into().unwrap_or([0; 2]);
                    (i16::from_le_bytes(bytes) as f32) / (i16::MAX as f32)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };

        let clamped = sample.clamp(-1.0, 1.0);
        *current_min = current_min.min(clamped);
        *current_max = current_max.max(clamped);
        *current_sum_sq += (clamped as f64) * (clamped as f64);
        *current_bin_samples += 1;

        if *current_bin_samples >= samples_per_bin {
            let rms = ((*current_sum_sq / *current_bin_samples as f64).sqrt() as f32).min(1.0);
            bins.push(WaveformBin {
                min: *current_min,
                max: *current_max,
                rms,
            });
            *current_bin_samples = 0;
            *current_min = 0.0;
            *current_max = 0.0;
            *current_sum_sq = 0.0;
        }
    }
}
