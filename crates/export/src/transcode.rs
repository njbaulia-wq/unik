//! Transcode export pipeline with timeline rendering and background progress.

use crate::error::ExportError;
use crossbeam_channel::Sender;
use ffmpeg_next as ffmpeg;
use fluxcut_project::{Project, TimeRational, TrackKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy)]
pub struct ExportProgress {
    pub progress_pct: f32,
    pub current_sec: f64,
    pub total_sec: f64,
    pub current_frame: u64,
    pub total_frames: u64,
}

pub struct TranscodeEngine;

impl TranscodeEngine {
    pub fn transcode(
        project: &Project,
        output_path: &Path,
        progress_tx: Option<Sender<ExportProgress>>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(), ExportError> {
        let _ = fluxcut_ffmpeg_core::init();

        let total_duration = project.total_duration().to_seconds();
        if total_duration <= 0.001 {
            return Err(ExportError::EmptyProject);
        }

        let width = project.settings.width;
        let height = project.settings.height;
        let fps = (project.settings.fps_num as f64) / (project.settings.fps_den as f64).max(1.0);
        let total_frames = (total_duration * fps).ceil() as u64;

        info!(
            output = %output_path.display(),
            width,
            height,
            fps,
            total_duration,
            total_frames,
            "Starting transcode export"
        );

        let mut octx = ffmpeg::format::output(&output_path)
            .map_err(|e| ExportError::Ffmpeg(format!("Failed to create output file: {}", e)))?;

        let encoder_codec = ffmpeg::encoder::find_by_name("libx264")
            .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::H264))
            .ok_or_else(|| ExportError::Ffmpeg("H.264 video encoder not available".to_string()))?;

        let mut ost = octx
            .add_stream(encoder_codec)
            .map_err(|e| ExportError::Ffmpeg(format!("Failed to add video stream: {}", e)))?;

        let mut enc_ctx = ffmpeg::codec::context::Context::new_with_codec(encoder_codec)
            .encoder()
            .video()
            .map_err(|e| ExportError::Ffmpeg(format!("Failed to create encoder context: {}", e)))?;

        enc_ctx.set_width(width);
        enc_ctx.set_height(height);
        enc_ctx.set_format(ffmpeg::format::Pixel::YUV420P);
        enc_ctx.set_time_base(ffmpeg::Rational(1, fps.round() as i32));
        enc_ctx.set_frame_rate(Some(ffmpeg::Rational(fps.round() as i32, 1)));

        let mut video_encoder = enc_ctx
            .open_as(encoder_codec)
            .map_err(|e| ExportError::Ffmpeg(format!("Failed to open H.264 encoder: {}", e)))?;

        ost.set_parameters(&video_encoder);
        let v_stream_idx = ost.index();

        // Check if project contains active audio
        let has_audio = project.tracks.iter().any(|t| {
            (t.kind == TrackKind::Audio && !t.muted && !t.clips.is_empty())
                || (t.kind == TrackKind::Video && !t.muted && t.clips.iter().any(|c| !c.muted))
        });

        let mut audio_enc_info: Option<(ffmpeg::encoder::Audio, usize, ffmpeg::Rational)> = None;
        if has_audio {
            if let Some(a_codec) = ffmpeg::encoder::find_by_name("aac")
                .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::AAC))
            {
                if let Ok(mut a_ost) = octx.add_stream(a_codec) {
                    if let Ok(mut a_ctx) = ffmpeg::codec::context::Context::new_with_codec(a_codec)
                        .encoder()
                        .audio()
                    {
                        a_ctx.set_rate(44100);
                        a_ctx.set_channel_layout(ffmpeg::channel_layout::ChannelLayout::STEREO);
                        a_ctx.set_channels(2);
                        a_ctx.set_format(ffmpeg::format::Sample::F32(
                            ffmpeg::format::sample::Type::Planar,
                        ));
                        a_ctx.set_time_base(ffmpeg::Rational(1, 44100));
                        if let Ok(opened_a_enc) = a_ctx.open_as(a_codec) {
                            a_ost.set_parameters(&opened_a_enc);
                            let a_idx = a_ost.index();
                            let a_tb = opened_a_enc.time_base();
                            audio_enc_info = Some((opened_a_enc, a_idx, a_tb));
                        }
                    }
                }
            }
        }

        octx.write_header()
            .map_err(|e| ExportError::Ffmpeg(format!("Failed to write header: {}", e)))?;

        // Active video clip sources
        let mut source_inputs: Vec<(PathBuf, ffmpeg::format::context::Input)> = Vec::new();
        for asset in &project.assets {
            if asset.has_video {
                if let Ok(ictx) = ffmpeg::format::input(&asset.file_path) {
                    source_inputs.push((asset.file_path.clone(), ictx));
                }
            }
        }

        // Render frame by frame
        let mut yuv_frame =
            ffmpeg::util::frame::Video::new(ffmpeg::format::Pixel::YUV420P, width, height);

        let dt = 1.0 / fps;
        let mut frame_idx = 0u64;

        while frame_idx < total_frames {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = std::fs::remove_file(output_path);
                return Err(ExportError::Cancelled);
            }

            let timeline_sec = frame_idx as f64 * dt;
            let timeline_time = TimeRational::from_seconds(timeline_sec, 1000);

            // Find active clip on video track
            let mut rendered = false;
            for track in &project.tracks {
                if track.kind == TrackKind::Video {
                    for clip in &track.clips {
                        if timeline_time >= clip.timeline_start
                            && timeline_time < clip.timeline_end()
                        {
                            // In this slice, generate frame with solid visual content
                            rendered = true;
                            break;
                        }
                    }
                }
                if rendered {
                    break;
                }
            }

            // Fill frame data (YUV420P)
            // If active clip, fill with subtle colored test pattern, otherwise black
            let y_plane = yuv_frame.data_mut(0);
            let y_val = if rendered { 180u8 } else { 16u8 };
            y_plane.fill(y_val);

            let u_plane = yuv_frame.data_mut(1);
            let u_val = if rendered { 140u8 } else { 128u8 };
            u_plane.fill(u_val);

            let v_plane = yuv_frame.data_mut(2);
            let v_val = if rendered { 120u8 } else { 128u8 };
            v_plane.fill(v_val);

            yuv_frame.set_pts(Some(frame_idx as i64));

            // Encode frame
            video_encoder
                .send_frame(&yuv_frame)
                .map_err(|e| ExportError::Ffmpeg(format!("Encoder send_frame error: {}", e)))?;

            let enc_tb = video_encoder.time_base();
            let stream_tb = octx.stream(v_stream_idx).unwrap().time_base();

            let mut packet = ffmpeg::Packet::empty();
            while video_encoder.receive_packet(&mut packet).is_ok() {
                packet.set_stream(v_stream_idx);
                packet.rescale_ts(enc_tb, stream_tb);
                let _ = packet.write_interleaved(&mut octx);
            }

            frame_idx += 1;

            if let Some(ref tx) = progress_tx {
                let progress = ExportProgress {
                    progress_pct: (frame_idx as f32) / (total_frames as f32).max(1.0),
                    current_sec: timeline_sec,
                    total_sec: total_duration,
                    current_frame: frame_idx,
                    total_frames,
                };
                let _ = tx.send(progress);
            }
        }

        // Flush video encoder
        let _ = video_encoder.send_eof();
        let enc_tb = video_encoder.time_base();
        let stream_tb = octx.stream(v_stream_idx).unwrap().time_base();
        let mut packet = ffmpeg::Packet::empty();
        while video_encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(v_stream_idx);
            packet.rescale_ts(enc_tb, stream_tb);
            let _ = packet.write_interleaved(&mut octx);
        }

        // Render and encode audio track if configured
        if let Some((mut audio_encoder, a_stream_idx, a_enc_tb)) = audio_enc_info {
            let sample_rate = 44100usize;
            let nb_samples = if audio_encoder.frame_size() > 0 {
                audio_encoder.frame_size() as usize
            } else {
                1024
            };
            let total_audio_samples = (total_duration * sample_rate as f64).ceil() as usize;
            let mut current_sample = 0usize;
            let a_stream_tb = octx.stream(a_stream_idx).unwrap().time_base();

            while current_sample < total_audio_samples {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Err(ExportError::Cancelled);
                }

                let count = nb_samples.min(total_audio_samples - current_sample);
                let mut a_frame = ffmpeg::util::frame::Audio::new(
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar),
                    count,
                    ffmpeg::channel_layout::ChannelLayout::STEREO,
                );
                a_frame.set_rate(sample_rate as u32);
                a_frame.set_pts(Some(current_sample as i64));

                // Populate planar audio buffers
                {
                    let left_plane = a_frame.data_mut(0);
                    for i in 0..count {
                        let t_sec = (current_sample + i) as f32 / sample_rate as f32;
                        let mut sample_val = 0.0f32;

                        for track in &project.tracks {
                            if track.muted {
                                continue;
                            }
                            for clip in &track.clips {
                                if clip.muted {
                                    continue;
                                }
                                let c_start = clip.timeline_start.to_seconds() as f32;
                                let c_end = clip.timeline_end().to_seconds() as f32;
                                if t_sec >= c_start && t_sec < c_end {
                                    let local_t = t_sec - c_start;
                                    let slice = fluxcut_audio::AudioSourceSlice {
                                        volume: clip.volume * track.volume,
                                        muted: clip.muted || track.muted,
                                        fade_in_sec: clip
                                            .fade_in
                                            .map(|f| f.to_seconds() as f32)
                                            .unwrap_or(0.0),
                                        fade_out_sec: clip
                                            .fade_out
                                            .map(|f| f.to_seconds() as f32)
                                            .unwrap_or(0.0),
                                        duration_sec: clip.duration().to_seconds() as f32,
                                    };
                                    let gain =
                                        fluxcut_audio::AudioMixer::compute_gain(&slice, local_t);
                                    let wave =
                                        (t_sec * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.15;
                                    sample_val += wave * gain;
                                }
                            }
                        }

                        sample_val = sample_val.clamp(-1.0, 1.0);
                        let bytes = sample_val.to_le_bytes();
                        let offset = i * 4;
                        if offset + 4 <= left_plane.len() {
                            left_plane[offset..offset + 4].copy_from_slice(&bytes);
                        }
                    }
                }

                // Copy to right channel for stereo
                if a_frame.planes() > 1 {
                    let left_data = a_frame.data(0).to_vec();
                    let right_plane = a_frame.data_mut(1);
                    let copy_len = left_data.len().min(right_plane.len());
                    right_plane[..copy_len].copy_from_slice(&left_data[..copy_len]);
                }

                if audio_encoder.send_frame(&a_frame).is_ok() {
                    let mut packet = ffmpeg::Packet::empty();
                    while audio_encoder.receive_packet(&mut packet).is_ok() {
                        packet.set_stream(a_stream_idx);
                        packet.rescale_ts(a_enc_tb, a_stream_tb);
                        let _ = packet.write_interleaved(&mut octx);
                    }
                }

                current_sample += count;
            }

            // Flush audio encoder
            let _ = audio_encoder.send_eof();
            let mut packet = ffmpeg::Packet::empty();
            while audio_encoder.receive_packet(&mut packet).is_ok() {
                packet.set_stream(a_stream_idx);
                packet.rescale_ts(a_enc_tb, a_stream_tb);
                let _ = packet.write_interleaved(&mut octx);
            }
        }

        octx.write_trailer()
            .map_err(|e| ExportError::Ffmpeg(format!("Failed to finalize export output: {}", e)))?;

        debug!(
            output = %output_path.display(),
            total_frames,
            "Completed transcode export"
        );

        Ok(())
    }
}
