//! Asynchronous background worker for lazy media ingestion, probing, and caching.

use crate::asset::{AudioStreamInfo, MediaMetadata, VideoStreamInfo};
use crossbeam_channel::{bounded, Receiver, Sender};
use fluxcut_cache::DiskCacheManager;
use fluxcut_ffmpeg_core::{extract_thumbnail, extract_waveform, probe_file, WaveformEnvelope};
use fluxcut_project::TimeRational;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{error, info, warn};

/// Ingestion request sent from UI or project coordinator to the worker.
#[derive(Debug, Clone)]
pub struct IngestRequest {
    pub asset_id: String,
    pub path: PathBuf,
    pub modified_timestamp: u64,
    pub file_size: u64,
    pub extract_thumbnail: bool,
    pub extract_waveform: bool,
}

/// Completion payload sent from worker back to UI / caller.
#[derive(Debug, Clone)]
pub struct IngestResponse {
    pub asset_id: String,
    pub result: Result<MediaMetadata, String>,
    pub thumbnail_rgba: Option<Vec<u8>>,
    pub thumbnail_dimensions: Option<(u32, u32)>,
    pub waveform_envelope: Option<WaveformEnvelope>,
}

/// Handle to the background ingestion worker.
pub struct MediaWorkerPool {
    request_tx: Option<Sender<IngestRequest>>,
    response_rx: Receiver<IngestResponse>,
    shutdown_flag: Arc<AtomicBool>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl MediaWorkerPool {
    /// Spawn background ingestion threadpool with bounded channels.
    pub fn new(cache: DiskCacheManager) -> Self {
        let (request_tx, request_rx) = bounded::<IngestRequest>(64);
        let (response_tx, response_rx) = bounded::<IngestResponse>(64);
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&shutdown_flag);

        let worker_handle = thread::Builder::new()
            .name("fluxcut-media-worker".to_string())
            .spawn(move || {
                info!("FluxCut media ingestion background worker started");

                while !flag_clone.load(Ordering::Relaxed) {
                    match request_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                        Ok(req) => {
                            let response = process_ingest_request(&req, &cache);
                            if let Err(e) = response_tx.send(response) {
                                warn!("Failed to send ingest response to UI: {}", e);
                                break;
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            continue;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    }
                }

                info!("FluxCut media ingestion background worker exited cleanly");
            })
            .expect("Failed to spawn media worker thread");

        Self {
            request_tx: Some(request_tx),
            response_rx,
            shutdown_flag,
            worker_handle: Some(worker_handle),
        }
    }

    /// Submit a media file for asynchronous background analysis.
    pub fn submit(&self, req: IngestRequest) -> Result<(), String> {
        if let Some(ref tx) = self.request_tx {
            tx.send(req)
                .map_err(|e| format!("Worker channel full or closed: {}", e))
        } else {
            Err("Worker pool shut down".to_string())
        }
    }

    /// Non-blocking check for completed ingestion responses.
    pub fn try_recv(&self) -> Option<IngestResponse> {
        self.response_rx.try_recv().ok()
    }

    /// Receiver clone for integration into GTK glib event loop via `glib::MainContext`.
    pub fn receiver(&self) -> Receiver<IngestResponse> {
        self.response_rx.clone()
    }
}

impl Drop for MediaWorkerPool {
    fn drop(&mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        self.request_tx.take(); // explicitly close request channel
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

fn process_ingest_request(req: &IngestRequest, cache: &DiskCacheManager) -> IngestResponse {
    let probe_res = match probe_file(&req.path) {
        Ok(p) => p,
        Err(e) => {
            error!(path = %req.path.display(), error = %e, "Failed to probe media file");
            return IngestResponse {
                asset_id: req.asset_id.clone(),
                result: Err(e.to_string()),
                thumbnail_rgba: None,
                thumbnail_dimensions: None,
                waveform_envelope: None,
            };
        }
    };

    let duration = TimeRational::from_seconds(probe_res.duration_seconds, 1000);

    let video = probe_res.primary_video_stream().map(|v| VideoStreamInfo {
        codec: v.codec_name.clone(),
        width: v.width,
        height: v.height,
        frame_rate: v.frame_rate,
        pixel_format: v.pixel_format.clone(),
    });

    let audio = probe_res.primary_audio_stream().map(|a| AudioStreamInfo {
        codec: a.codec_name.clone(),
        sample_rate: a.sample_rate,
        channels: a.channels,
        channel_layout: a.channel_layout.clone(),
    });

    let metadata = MediaMetadata {
        format_name: probe_res.format_name,
        duration,
        video,
        audio,
    };

    // Lazy Thumbnail extraction & caching
    let mut thumbnail_rgba = None;
    let mut thumbnail_dimensions = None;

    if req.extract_thumbnail && metadata.video.is_some() {
        let thumb_key = DiskCacheManager::generate_key(
            &req.path,
            req.modified_timestamp,
            req.file_size,
            "thumb_w160_h90_t0.5",
        );

        if let Some(cached_data) = cache.get_thumbnail(&thumb_key) {
            thumbnail_rgba = Some(cached_data);
            thumbnail_dimensions = Some((160, 90));
        } else {
            // Seek to 0.5s or 0.0s for thumbnail
            let target_time = if probe_res.duration_seconds > 1.0 {
                0.5
            } else {
                0.0
            };
            if let Ok(img) = extract_thumbnail(&req.path, target_time, 160, 90) {
                let _ = cache.put_thumbnail(&thumb_key, &img.data);
                thumbnail_dimensions = Some((img.width, img.height));
                thumbnail_rgba = Some(img.data);
            }
        }
    }

    // Lazy Waveform extraction & caching
    let mut waveform_envelope = None;

    if req.extract_waveform && metadata.audio.is_some() {
        let wave_key = DiskCacheManager::generate_key(
            &req.path,
            req.modified_timestamp,
            req.file_size,
            "wave_bins50",
        );

        if let Some(cached_bytes) = cache.get_waveform(&wave_key) {
            if let Ok(env) = serde_json::from_slice(&cached_bytes) {
                waveform_envelope = Some(env);
            }
        }

        if waveform_envelope.is_none() {
            if let Ok(env) = extract_waveform(&req.path, 50) {
                if let Ok(json_bytes) = serde_json::to_vec(&env) {
                    let _ = cache.put_waveform(&wave_key, &json_bytes);
                }
                waveform_envelope = Some(env);
            }
        }
    }

    IngestResponse {
        asset_id: req.asset_id.clone(),
        result: Ok(metadata),
        thumbnail_rgba,
        thumbnail_dimensions,
        waveform_envelope,
    }
}
