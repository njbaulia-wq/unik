//! Post-export verification and QA assertion.
//!
//! Validates that the rendered output file is playable, has valid stream durations,
//! and strictly matches target dimensions and codecs.

use crate::error::ExportError;
use fluxcut_ffmpeg_core::probe_file;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ExportValidationReport {
    pub duration_sec: f64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub file_size_bytes: u64,
}

pub fn verify_exported_file(
    path: &Path,
    expected_width: Option<u32>,
    expected_height: Option<u32>,
    min_duration_sec: f64,
) -> Result<ExportValidationReport, ExportError> {
    if !path.exists() {
        return Err(ExportError::Validation(format!(
            "Exported file does not exist: {}",
            path.display()
        )));
    }

    let meta = std::fs::metadata(path).map_err(|e| ExportError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    if meta.len() == 0 {
        return Err(ExportError::Validation(
            "Exported file is empty (0 bytes)".to_string(),
        ));
    }

    let probed = probe_file(path).map_err(|e| {
        ExportError::Validation(format!("Failed to probe exported media file: {}", e))
    })?;

    let dur_sec = probed.duration_seconds;
    if dur_sec < min_duration_sec {
        return Err(ExportError::Validation(format!(
            "Exported duration {:.2}s is below expected minimum {:.2}s",
            dur_sec, min_duration_sec
        )));
    }

    let primary_v = probed.primary_video_stream();

    if let (Some(exp_w), Some(exp_h)) = (expected_width, expected_height) {
        if let Some(v) = primary_v {
            if v.width != exp_w || v.height != exp_h {
                return Err(ExportError::Validation(format!(
                    "Exported dimensions {}x{} do not match target {}x{}",
                    v.width, v.height, exp_w, exp_h
                )));
            }
        } else {
            return Err(ExportError::Validation(
                "Exported video stream is missing".to_string(),
            ));
        }
    }

    let primary_a = probed.primary_audio_stream();

    Ok(ExportValidationReport {
        duration_sec: dur_sec,
        width: primary_v.map(|v| v.width),
        height: primary_v.map(|v| v.height),
        video_codec: primary_v.map(|v| v.codec_name.clone()),
        audio_codec: primary_a.map(|a| a.codec_name.clone()),
        file_size_bytes: meta.len(),
    })
}
