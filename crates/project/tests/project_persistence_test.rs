use fluxcut_project::{
    AssetReference, CanvasRatio, ClipDefinition, Project, ProjectError, TimeRational,
    TrackDefinition, TrackKind, CURRENT_FORMAT_VERSION,
};
use std::path::PathBuf;

#[test]
fn test_project_lifecycle_and_file_io() {
    let temp_dir = std::env::temp_dir().join(format!("fluxcut_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let project_path = temp_dir.join("test_project.fluxcut");

    let mut project = Project::new("Integration Test Project");
    project.set_canvas_ratio(CanvasRatio::Vertical9x16);

    let asset = AssetReference {
        id: "asset-1".to_string(),
        file_path: PathBuf::from("/mock/path/video.mp4"),
        display_name: "video.mp4".to_string(),
        duration: TimeRational::new(300, 1),
        width: Some(1920),
        height: Some(1080),
        has_audio: true,
        has_video: true,
    };
    project.assets.push(asset);

    let clip = ClipDefinition {
        id: "clip-1".to_string(),
        asset_id: "asset-1".to_string(),
        timeline_start: TimeRational::ZERO,
        in_point: TimeRational::new(5, 1),
        out_point: TimeRational::new(15, 1),
        volume: 0.8,
        muted: false,
        speed: 1.0,
        ..Default::default()
    };

    project.tracks.push(TrackDefinition {
        id: "track-v2".to_string(),
        name: "Overlay Track".to_string(),
        kind: TrackKind::Video,
        muted: false,
        volume: 1.0,
        clips: vec![clip],
    });

    // Save project
    project
        .save_to_file(&project_path)
        .expect("Failed to save project file");
    assert!(project_path.exists());

    // Load project back
    let loaded = Project::load_from_file(&project_path).expect("Failed to load project file");

    assert_eq!(loaded.name, "Integration Test Project");
    assert_eq!(loaded.format_version, CURRENT_FORMAT_VERSION);
    assert_eq!(loaded.settings.width, 1080);
    assert_eq!(loaded.settings.height, 1920);
    assert_eq!(loaded.assets.len(), 1);
    assert_eq!(loaded.tracks.len(), 3); // 2 default tracks + 1 added
    assert_eq!(
        loaded.tracks[2].clips[0].duration(),
        TimeRational::new(10, 1)
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_unsupported_project_version_rejection() {
    let temp_dir = std::env::temp_dir().join(format!("fluxcut_test_ver_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let project_path = temp_dir.join("future_project.fluxcut");

    let future_json = r##"{
        "format_version": 9999,
        "name": "Future Project",
        "settings": {
            "width": 1920,
            "height": 1080,
            "fps_num": 30,
            "fps_den": 1,
            "background_color": "#000000",
            "default_image_duration_sec": 5.0
        },
        "assets": [],
        "tracks": []
    }"##;

    std::fs::write(&project_path, future_json).expect("Failed to write future project json");

    let load_result = Project::load_from_file(&project_path);
    assert!(load_result.is_err());
    match load_result.unwrap_err() {
        ProjectError::UnsupportedVersion { found, supported } => {
            assert_eq!(found, 9999);
            assert_eq!(supported, CURRENT_FORMAT_VERSION);
        }
        other => panic!("Expected UnsupportedVersion error, got: {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
