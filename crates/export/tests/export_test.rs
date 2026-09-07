use fluxcut_export::{
    can_fast_path_remux, verify_exported_file, ExportController, ExportEvent, ExportMode,
};
use fluxcut_project::{AssetReference, CanvasRatio, ClipDefinition, Project, TimeRational};
use std::process::Command;
use std::time::Duration;

fn generate_synthetic_mp4(path: &std::path::Path, duration_sec: f64, width: u32, height: u32) {
    let status = Command::new("ffmpeg")
        .args([
            "-f",
            "lavfi",
            "-i",
            &format!(
                "testsrc=duration={:.1}:size={}x{}:rate=30",
                duration_sec, width, height
            ),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ffmpeg");
    assert!(status.status.success());
}

#[test]
fn test_fast_path_remux_scenario_b() {
    let temp_dir =
        std::env::temp_dir().join(format!("fluxcut_export_remux_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let input_path = temp_dir.join("source.mp4");
    let output_path = temp_dir.join("fast_cut.mp4");

    generate_synthetic_mp4(&input_path, 4.0, 1920, 1080);

    let mut project = Project::default();
    project.settings.width = 1920;
    project.settings.height = 1080;

    let asset = AssetReference {
        id: "asset-1".to_string(),
        file_path: input_path.clone(),
        display_name: "source.mp4".to_string(),
        duration: TimeRational::from_seconds(4.0, 1000),
        width: Some(1920),
        height: Some(1080),
        has_audio: false,
        has_video: true,
    };
    project.assets.push(asset);

    let clip = ClipDefinition {
        id: "c1".to_string(),
        asset_id: "asset-1".to_string(),
        timeline_start: TimeRational::ZERO,
        in_point: TimeRational::from_seconds(1.0, 1000),
        out_point: TimeRational::from_seconds(3.0, 1000), // 2.0s duration
        volume: 1.0,
        muted: false,
        speed: 1.0,
        ..Default::default()
    };
    project.tracks[0].clips.push(clip);

    // Verify fast path eligibility
    assert!(can_fast_path_remux(&project).is_some());

    // Execute background export via controller
    let controller = ExportController::start(project, output_path.clone());

    let mut finished = false;
    for _ in 0..50 {
        if let Some(event) = controller.try_recv() {
            match event {
                ExportEvent::Started { mode } => {
                    assert_eq!(mode, ExportMode::FastPathRemux);
                }
                ExportEvent::Finished { mode, report } => {
                    assert_eq!(mode, ExportMode::FastPathRemux);
                    assert_eq!(report.width, Some(1920));
                    assert_eq!(report.height, Some(1080));
                    assert!(report.duration_sec >= 1.8);
                    finished = true;
                    break;
                }
                ExportEvent::Error(err) => panic!("Export error: {}", err),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(finished, "Fast-path remux did not finish in time");
    assert!(output_path.exists());

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_social_vertical_transcode_scenario_a() {
    let temp_dir =
        std::env::temp_dir().join(format!("fluxcut_export_transcode_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let input_path = temp_dir.join("landscape_16x9.mp4");
    let output_path = temp_dir.join("vertical_9x16.mp4");

    generate_synthetic_mp4(&input_path, 2.0, 1920, 1080);

    // Scenario A: 9:16 vertical canvas (1080x1920)
    let mut project = Project::default();
    project.set_canvas_ratio(CanvasRatio::Vertical9x16);
    assert_eq!(project.settings.width, 1080);
    assert_eq!(project.settings.height, 1920);

    let asset = AssetReference {
        id: "asset-1".to_string(),
        file_path: input_path.clone(),
        display_name: "landscape_16x9.mp4".to_string(),
        duration: TimeRational::from_seconds(2.0, 1000),
        width: Some(1920),
        height: Some(1080),
        has_audio: false,
        has_video: true,
    };
    project.assets.push(asset);

    // Place and split clip: 0.0 to 0.8s, then 1.2 to 2.0s (deleted middle gap)
    project.tracks[0].clips.push(ClipDefinition {
        id: "c1_head".to_string(),
        asset_id: "asset-1".to_string(),
        timeline_start: TimeRational::ZERO,
        in_point: TimeRational::ZERO,
        out_point: TimeRational::from_seconds(0.8, 1000),
        volume: 1.0,
        muted: true, // Mute original audio
        speed: 1.0,
        ..Default::default()
    });
    project.tracks[0].clips.push(ClipDefinition {
        id: "c1_tail".to_string(),
        asset_id: "asset-1".to_string(),
        timeline_start: TimeRational::from_seconds(0.8, 1000),
        in_point: TimeRational::from_seconds(1.2, 1000),
        out_point: TimeRational::from_seconds(2.0, 1000),
        volume: 1.0,
        muted: true,
        speed: 1.0,
        ..Default::default()
    });

    // Add BGM music track at 70% volume (PRD Scenario A step 9-11)
    project.tracks[1].clips.push(ClipDefinition {
        id: "bgm_music".to_string(),
        asset_id: "asset-1".to_string(),
        timeline_start: TimeRational::ZERO,
        in_point: TimeRational::ZERO,
        out_point: TimeRational::from_seconds(1.6, 1000),
        volume: 0.70,
        muted: false,
        speed: 1.0,
        ..Default::default()
    });

    // Verify fast path is disabled because dimensions changed (1920x1080 -> 1080x1920)
    assert!(can_fast_path_remux(&project).is_none());

    // Execute transcode export
    let controller = ExportController::start(project, output_path.clone());

    let mut finished = false;
    let mut received_progress = false;

    for _ in 0..100 {
        if let Some(event) = controller.try_recv() {
            match event {
                ExportEvent::Started { mode } => {
                    assert_eq!(mode, ExportMode::Transcode);
                }
                ExportEvent::Progress(prog) => {
                    assert!(prog.progress_pct >= 0.0 && prog.progress_pct <= 1.0);
                    received_progress = true;
                }
                ExportEvent::Finished { mode, report } => {
                    assert_eq!(mode, ExportMode::Transcode);
                    assert_eq!(report.width, Some(1080));
                    assert_eq!(report.height, Some(1920));
                    assert!(report.duration_sec >= 1.5);
                    assert!(
                        report.audio_codec.is_some(),
                        "Expected audio stream with BGM music"
                    );
                    finished = true;
                    break;
                }
                ExportEvent::Error(err) => panic!("Transcode export error: {}", err),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(received_progress, "Did not receive export progress updates");
    assert!(finished, "Transcode export did not complete in time");

    // Post-export QA validation
    let qa = verify_exported_file(&output_path, Some(1080), Some(1920), 1.5)
        .expect("QA verification failed");
    assert_eq!(qa.width, Some(1080));
    assert_eq!(qa.height, Some(1920));
    assert!(qa.audio_codec.is_some());

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_audio_replacement_and_fade_scenario_c() {
    let temp_dir =
        std::env::temp_dir().join(format!("fluxcut_export_audio_c_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let video_input_path = temp_dir.join("original_video.mp4");
    let output_path = temp_dir.join("audio_replaced.mp4");

    generate_synthetic_mp4(&video_input_path, 2.0, 1280, 720);

    let mut project = Project::default();
    project.settings.width = 1280;
    project.settings.height = 720;

    let asset = AssetReference {
        id: "asset-video".to_string(),
        file_path: video_input_path.clone(),
        display_name: "original_video.mp4".to_string(),
        duration: TimeRational::from_seconds(2.0, 1000),
        width: Some(1280),
        height: Some(720),
        has_audio: true,
        has_video: true,
    };
    project.assets.push(asset);

    // Track 0 (Video): Original audio muted
    project.tracks[0].clips.push(ClipDefinition {
        id: "v_clip".to_string(),
        asset_id: "asset-video".to_string(),
        timeline_start: TimeRational::ZERO,
        in_point: TimeRational::ZERO,
        out_point: TimeRational::from_seconds(2.0, 1000),
        volume: 1.0,
        muted: true, // original audio muted
        speed: 1.0,
        ..Default::default()
    });

    // Track 1 (Audio): External audio with fade out
    project.tracks[1].clips.push(ClipDefinition {
        id: "a_clip".to_string(),
        asset_id: "asset-video".to_string(),
        timeline_start: TimeRational::ZERO,
        in_point: TimeRational::ZERO,
        out_point: TimeRational::from_seconds(2.0, 1000),
        volume: 0.85,
        muted: false,
        speed: 1.0,
        fade_out: Some(TimeRational::from_seconds(0.5, 1000)),
        ..Default::default()
    });

    let controller = ExportController::start(project, output_path.clone());
    let mut finished = false;

    for _ in 0..100 {
        if let Some(event) = controller.try_recv() {
            match event {
                ExportEvent::Finished { mode, report } => {
                    assert_eq!(mode, ExportMode::Transcode);
                    assert!(report.audio_codec.is_some());
                    assert_eq!(report.width, Some(1280));
                    assert_eq!(report.height, Some(720));
                    finished = true;
                    break;
                }
                ExportEvent::Error(err) => panic!("Export error: {}", err),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(finished, "Scenario C export did not finish in time");
    let qa = verify_exported_file(&output_path, Some(1280), Some(720), 1.5).unwrap();
    assert!(qa.audio_codec.is_some());

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_image_template_scenario_d() {
    let temp_dir =
        std::env::temp_dir().join(format!("fluxcut_export_template_d_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let output_path = temp_dir.join("image_template_output.mp4");

    let mut project = Project::default();
    project.settings.width = 1920;
    project.settings.height = 1080;

    // Add still image clip with template preset (CenterCard)
    let clip = ClipDefinition {
        id: "img_card".to_string(),
        asset_id: "img_asset".to_string(),
        timeline_start: TimeRational::ZERO,
        in_point: TimeRational::ZERO,
        out_point: TimeRational::from_seconds(1.5, 1000), // 1.5s duration
        volume: 1.0,
        muted: true,
        speed: 1.0,
        template: Some(fluxcut_project::ImageTemplate::CenterCard),
        ..Default::default()
    };
    project.tracks[0].clips.push(clip);

    let controller = ExportController::start(project, output_path.clone());
    let mut finished = false;

    for _ in 0..100 {
        if let Some(event) = controller.try_recv() {
            match event {
                ExportEvent::Finished { mode, report } => {
                    assert_eq!(mode, ExportMode::Transcode);
                    assert_eq!(report.width, Some(1920));
                    assert_eq!(report.height, Some(1080));
                    assert!(report.duration_sec >= 1.0);
                    finished = true;
                    break;
                }
                ExportEvent::Error(err) => panic!("Export error: {}", err),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(finished, "Scenario D export did not finish in time");
    let qa = verify_exported_file(&output_path, Some(1920), Some(1080), 1.0).unwrap();
    assert_eq!(qa.width, Some(1920));
    assert_eq!(qa.height, Some(1080));

    let _ = std::fs::remove_dir_all(temp_dir);
}
