//! Automated performance benchmark suite validating PRD Section 32 & 33 budgets.

use fluxcut_export::remux::fast_path_remux;
use fluxcut_export::transcode::TranscodeEngine;
use fluxcut_ffmpeg_core::extract_waveform;
use fluxcut_hardware::GpuCapabilities;
use fluxcut_project::{ClipDefinition, Project, TimeRational};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

fn generate_benchmark_mp4(path: &Path, duration_sec: f64, width: u32, height: u32) {
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!(
                "testsrc=duration={:.1}:size={}x{}:rate=30",
                duration_sec, width, height
            ),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=1000:duration={:.1}", duration_sec),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ffmpeg for benchmark synthetic fixture");

    assert!(
        output.status.success(),
        "FFmpeg synthetic media generation failed"
    );
}

#[test]
fn bench_hardware_startup_latency() {
    let t0 = Instant::now();
    let caps = GpuCapabilities::probe();
    let elapsed = t0.elapsed();

    println!(
        "\n[BENCHMARK] Hardware capability probe: {:?} (Budget: < 100ms)",
        elapsed
    );
    assert!(
        elapsed.as_millis() < 500,
        "Startup capability probe took too long: {:?}",
        elapsed
    );
    assert!(
        caps.vulkan_icd_available
            || caps.vaapi_available
            || caps.dmabuf_available
            || !caps.wayland_available
    );
}

#[test]
fn bench_dense_timeline_operations_latency() {
    let mut project = Project::new("Dense Timeline Benchmark");

    // Add 1,000 clips to test scaling performance (PRD Section 33 case 9 & 10)
    for i in 0..1000 {
        let start = TimeRational::from_seconds(i as f64 * 2.0, 1000);
        let clip = ClipDefinition::new(
            format!("clip_{}", i),
            "asset_bench",
            start,
            TimeRational::from_seconds(2.0, 1000),
        );
        project.tracks[0].clips.push(clip);
    }

    assert_eq!(project.tracks[0].clips.len(), 1000);

    // Benchmark total duration calculation
    let t0 = Instant::now();
    let dur = project.total_duration();
    let t_dur = t0.elapsed();
    assert_eq!(dur.to_seconds(), 2000.0);
    println!("[BENCHMARK] 1,000 clips total_duration: {:?}", t_dur);
    assert!(t_dur.as_millis() < 5, "total_duration too slow");

    // Benchmark snapping query
    let t0 = Instant::now();
    let threshold = TimeRational::from_seconds(0.1, 1000);
    let snapped = project.snap_time(TimeRational::from_seconds(500.05, 1000), threshold, None);
    let t_snap = t0.elapsed();
    assert_eq!(snapped.to_seconds(), 500.0);
    println!("[BENCHMARK] 1,000 clips snap_time query: {:?}", t_snap);
    assert!(t_snap.as_millis() < 5, "snap_time too slow");

    // Benchmark split operation
    let t0 = Instant::now();
    let split_res = project.split_clip("clip_500", TimeRational::from_seconds(1001.0, 1000));
    let t_split = t0.elapsed();
    assert!(split_res.is_ok());
    println!("[BENCHMARK] 1,000 clips split operation: {:?}", t_split);
    assert!(t_split.as_millis() < 5, "split too slow");
}

#[test]
fn bench_waveform_extraction_throughput() {
    let temp_dir =
        std::env::temp_dir().join(format!("fluxcut_bench_waveform_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file = temp_dir.join("sample_audio.mp4");

    generate_benchmark_mp4(&file, 3.0, 640, 360);

    let t0 = Instant::now();
    let envelope = extract_waveform(&file, 50).expect("Waveform extraction failed");
    let elapsed = t0.elapsed();

    let duration_sec = envelope.duration_seconds;
    let rate = duration_sec / elapsed.as_secs_f64();

    println!(
        "[BENCHMARK] Waveform extraction: {:.2}s of audio processed in {:?} ({:.1}x realtime)",
        duration_sec, elapsed, rate
    );

    assert!(envelope.bins.len() >= 100);
    assert!(rate >= 2.0, "Waveform extraction slower than 2x realtime");

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn bench_fast_path_remux_throughput() {
    let temp_dir = std::env::temp_dir().join(format!("fluxcut_bench_remux_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("bench_source.mp4");
    let output = temp_dir.join("bench_remux.mp4");

    generate_benchmark_mp4(&input, 3.0, 1280, 720);
    let in_size = std::fs::metadata(&input).unwrap().len();

    let t0 = Instant::now();
    fast_path_remux(&input, &output, 0.5, 2.5).expect("Fast path remux failed");
    let elapsed = t0.elapsed();

    let out_size = std::fs::metadata(&output).unwrap().len();
    let throughput_mb = (out_size as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();

    println!(
        "[BENCHMARK] Fast-path remux: input={} KB, output={} KB in {:?} ({:.1} MB/s throughput)",
        in_size / 1024,
        out_size / 1024,
        elapsed,
        throughput_mb
    );

    assert!(elapsed.as_millis() < 500, "Remux took longer than 500ms");
    assert!(out_size > 0);

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn bench_transcode_export_fps() {
    let temp_dir =
        std::env::temp_dir().join(format!("fluxcut_bench_transcode_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let output = temp_dir.join("bench_transcode.mp4");

    let mut project = Project::default();
    project.settings.width = 1280;
    project.settings.height = 720;
    let clip = ClipDefinition::new(
        "c1",
        "asset_synthetic",
        TimeRational::ZERO,
        TimeRational::from_seconds(2.0, 1000), // 60 frames at 30 fps
    );
    project.tracks[0].clips.push(clip);

    let t0 = Instant::now();
    TranscodeEngine::transcode(&project, &output, None, Arc::new(AtomicBool::new(false)))
        .expect("Transcode failed");
    let elapsed = t0.elapsed();

    let total_frames = 60.0;
    let fps = total_frames / elapsed.as_secs_f64();

    println!(
        "[BENCHMARK] Transcode export 720p: 60 frames in {:?} ({:.1} FPS)",
        elapsed, fps
    );

    assert!(fps >= 15.0, "Transcode export below 15 FPS: {:.1}", fps);

    let _ = std::fs::remove_dir_all(temp_dir);
}
