//! Fast-path stream copy remuxer without re-encoding.
//!
//! Performs direct AVPacket stream copying with timestamp shifting when technical
//! boundaries permit, achieving sub-second cut exports.

use crate::error::ExportError;
use ffmpeg_next as ffmpeg;
use fluxcut_project::{Project, TrackKind};
use std::path::{Path, PathBuf};
use tracing::info;

/// Evaluates if the project qualifies for lossless fast-path stream copy.
pub fn can_fast_path_remux(project: &Project) -> Option<(PathBuf, f64, f64)> {
    // Single video track with exactly 1 clip, no speed alteration, matching canvas size
    let video_tracks: Vec<_> = project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video)
        .collect();

    if video_tracks.len() != 1 {
        return None;
    }

    let v_track = video_tracks[0];
    if v_track.clips.len() != 1 {
        return None;
    }

    let clip = &v_track.clips[0];
    if (clip.speed - 1.0).abs() > 0.001 {
        return None;
    }

    // Muted audio, crop, transform, or template presets require re-encoding/transcoding
    if clip.muted || clip.crop.is_some() || clip.transform.is_some() || clip.template.is_some() {
        return None;
    }

    // Dedicated audio tracks require mixing/re-encoding
    let has_audio_tracks = project
        .tracks
        .iter()
        .any(|t| t.kind == TrackKind::Audio && !t.clips.is_empty());
    if has_audio_tracks {
        return None;
    }

    let asset = project.assets.iter().find(|a| a.id == clip.asset_id)?;

    // Dimensions must match project canvas
    if let (Some(w), Some(h)) = (asset.width, asset.height) {
        if w != project.settings.width || h != project.settings.height {
            return None;
        }
    }

    Some((
        asset.file_path.clone(),
        clip.in_point.to_seconds(),
        clip.out_point.to_seconds(),
    ))
}

/// Execute stream-copy remuxing from `in_sec` to `out_sec`.
pub fn fast_path_remux(
    input_path: &Path,
    output_path: &Path,
    in_sec: f64,
    out_sec: f64,
) -> Result<(), ExportError> {
    info!(
        input = %input_path.display(),
        output = %output_path.display(),
        in_sec,
        out_sec,
        "Executing lossless fast-path remux"
    );

    let mut ictx = ffmpeg::format::input(&input_path)
        .map_err(|e| ExportError::Ffmpeg(format!("Failed to open input for remux: {}", e)))?;

    let mut octx = ffmpeg::format::output(&output_path)
        .map_err(|e| ExportError::Ffmpeg(format!("Failed to open output for remux: {}", e)))?;

    let mut stream_mapping = Vec::new();

    for (idx, ist) in ictx.streams().enumerate() {
        let medium = ist.parameters().medium();
        if medium == ffmpeg::media::Type::Video || medium == ffmpeg::media::Type::Audio {
            let mut ost = octx
                .add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))
                .map_err(|e| {
                    ExportError::Ffmpeg(format!("Failed to add stream to remux output: {}", e))
                })?;
            ost.set_parameters(ist.parameters());
            stream_mapping.push((idx, ost.index()));
        }
    }

    octx.write_header()
        .map_err(|e| ExportError::Ffmpeg(format!("Failed to write remux header: {}", e)))?;

    // Seek to in_sec
    if let Some(vs) = ictx.streams().best(ffmpeg::media::Type::Video) {
        let tb = vs.time_base();
        let in_pts = (in_sec * (tb.denominator() as f64) / (tb.numerator() as f64)).round() as i64;
        let _ = ictx.seek(in_pts, ..in_pts);
    }

    let mut base_pts: Option<i64> = None;
    let mut base_dts: Option<i64> = None;

    for (stream, mut packet) in ictx.packets() {
        if let Some(&(_in_idx, out_idx)) = stream_mapping.iter().find(|(i, _)| *i == stream.index())
        {
            let in_tb = stream.time_base();
            let pts_sec = packet.pts().unwrap_or(0) as f64 * (in_tb.numerator() as f64)
                / (in_tb.denominator() as f64);

            if pts_sec > out_sec + 0.1 {
                break;
            }
            if pts_sec >= in_sec - 0.05 {
                if base_pts.is_none() {
                    base_pts = packet.pts();
                }
                if base_dts.is_none() {
                    base_dts = packet.dts();
                }

                if let (Some(pts), Some(b_pts)) = (packet.pts(), base_pts) {
                    packet.set_pts(Some((pts - b_pts).max(0)));
                }
                if let (Some(dts), Some(b_dts)) = (packet.dts(), base_dts) {
                    packet.set_dts(Some((dts - b_dts).max(0)));
                }

                let out_tb = octx.stream(out_idx).unwrap().time_base();
                packet.rescale_ts(in_tb, out_tb);
                packet.set_position(-1);
                packet.set_stream(out_idx);
                let _ = packet.write_interleaved(&mut octx);
            }
        }
    }

    octx.write_trailer()
        .map_err(|e| ExportError::Ffmpeg(format!("Failed to finalize remux output: {}", e)))?;

    Ok(())
}
