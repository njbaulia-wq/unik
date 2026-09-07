use fluxcut_cache::DiskCacheManager;
use fluxcut_ffmpeg_core::{extract_thumbnail, extract_waveform, probe_file};
use fluxcut_media::{IngestRequest, MediaWorkerPool};
use std::process::Command;
use std::time::Duration;

fn generate_test_media(path: &std::path::Path) {
    // Generate a 1-second 320x240 H.264 video with 1kHz AAC audio tone
    let status = Command::new("ffmpeg")
        .args([
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1.0:size=320x240:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=1.0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-y",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ffmpeg to generate test media fixture");

    assert!(status.status.success(), "ffmpeg test generator failed");
}

#[test]
fn test_media_probing_and_core_extraction() {
    let temp_dir = std::env::temp_dir().join(format!("fluxcut_media_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let video_path = temp_dir.join("synthetic_clip.mp4");

    generate_test_media(&video_path);

    // 1. Test probe
    let probe = probe_file(&video_path).expect("Failed to probe synthetic clip");
    assert!(probe.duration_seconds >= 0.9 && probe.duration_seconds <= 1.1);
    assert_eq!(probe.video_streams.len(), 1);
    assert_eq!(probe.audio_streams.len(), 1);

    let v = &probe.video_streams[0];
    assert_eq!(v.width, 320);
    assert_eq!(v.height, 240);
    assert!(v.codec_name.contains("h264"));

    let a = &probe.audio_streams[0];
    assert_eq!(a.channels, 1);
    assert!(a.codec_name.contains("aac"));

    // 2. Test thumbnail extraction
    let thumb = extract_thumbnail(&video_path, 0.5, 160, 120).expect("Failed to extract thumbnail");
    assert_eq!(thumb.width, 160);
    assert_eq!(thumb.height, 120);
    assert_eq!(thumb.data.len(), (160 * 120 * 4) as usize);

    // 3. Test waveform extraction
    let wave = extract_waveform(&video_path, 50).expect("Failed to extract waveform");
    assert_eq!(wave.bins_per_second, 50);
    assert!(!wave.bins.is_empty());
    assert!(wave.bins[0].rms > 0.0);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_media_worker_pool_async_ingest_and_caching() {
    let temp_dir = std::env::temp_dir().join(format!("fluxcut_worker_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let video_path = temp_dir.join("worker_clip.mp4");
    let cache_dir = temp_dir.join("cache");

    generate_test_media(&video_path);

    let cache = DiskCacheManager::with_base_dir(cache_dir).expect("Failed to init cache");
    let pool = MediaWorkerPool::new(cache);

    let req = IngestRequest {
        asset_id: "test-asset-1".to_string(),
        path: video_path.clone(),
        modified_timestamp: 100,
        file_size: 5000,
        extract_thumbnail: true,
        extract_waveform: true,
    };

    pool.submit(req).expect("Failed to submit ingest request");

    // Poll for response with timeout
    let mut received_response = None;
    for _ in 0..50 {
        if let Some(resp) = pool.try_recv() {
            received_response = Some(resp);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    assert!(
        received_response.is_some(),
        "Timed out waiting for ingest response"
    );
    let resp = received_response.unwrap();
    assert_eq!(resp.asset_id, "test-asset-1");
    assert!(resp.result.is_ok());

    let meta = resp.result.unwrap();
    assert!(meta.video.is_some());
    assert!(meta.audio.is_some());
    assert!(resp.thumbnail_rgba.is_some());
    assert!(resp.waveform_envelope.is_some());

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
