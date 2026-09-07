use fluxcut_decode::{PlaybackController, PlaybackEvent, PlaybackState};
use std::process::Command;
use std::time::Duration;

fn generate_test_media(path: &std::path::Path) {
    let output = match Command::new("ffmpeg")
        .args([
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=2:size=320x240:rate=30",
            "-pix_fmt",
            "yuv420p",
            "-y",
            path.to_str().unwrap(),
        ])
        .output()
    {
        Ok(out) => out,
        Err(_) => {
            eprintln!("ffmpeg binary not found; skipping test media generation");
            return;
        }
    };

    if !output.status.success() {
        eprintln!("ffmpeg fixture generator failed; skipping test media generation");
    }
}

#[test]
fn test_playback_controller_lifecycle_and_seeking() {
    let temp_dir =
        std::env::temp_dir().join(format!("fluxcut_playback_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let video_path = temp_dir.join("playback_clip.mp4");

    generate_test_media(&video_path);

    let controller = PlaybackController::new();
    controller
        .load(video_path)
        .expect("Failed to submit load command");

    // 1. Wait for Loaded event
    let mut loaded = false;
    let mut first_frame_received = false;

    for _ in 0..50 {
        if let Some(event) = controller.try_recv() {
            match event {
                PlaybackEvent::Loaded {
                    duration_sec,
                    width,
                    height,
                    fps,
                } => {
                    assert!((1.8..=2.2).contains(&duration_sec));
                    assert_eq!(width, 320);
                    assert_eq!(height, 240);
                    assert_eq!(fps.round(), 30.0);
                    loaded = true;
                }
                PlaybackEvent::Frame(frame) => {
                    assert_eq!(frame.width, 320);
                    assert_eq!(frame.height, 240);
                    assert!(!frame.rgba_data.is_empty());
                    first_frame_received = true;
                    if loaded {
                        break;
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(loaded, "Timed out waiting for PlaybackEvent::Loaded");
    assert!(first_frame_received, "Timed out waiting for initial frame");

    // 2. Test Step Frame
    controller.step_frame(true).expect("Failed to step frame");
    let mut stepped_frame = false;
    for _ in 0..30 {
        if let Some(PlaybackEvent::Frame(f)) = controller.try_recv() {
            assert!(f.pts_sec >= 0.0);
            stepped_frame = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(stepped_frame, "Did not receive frame after step_frame");

    // 3. Test Coalesced Seeking (Rapid seeks)
    controller.seek(0.2).unwrap();
    controller.seek(0.5).unwrap();
    controller.seek(1.2).unwrap();

    let mut seek_frame = None;
    for _ in 0..50 {
        if let Some(PlaybackEvent::Frame(f)) = controller.try_recv() {
            seek_frame = Some(f);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(
        seek_frame.is_some(),
        "Did not receive frame after coalesced seek"
    );
    let sf = seek_frame.unwrap();
    assert!(
        sf.pts_sec >= 0.5,
        "Frame PTS should have advanced to near latest seek target"
    );

    // 4. Test Play & Pause
    controller.play(1.0).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    controller.pause().unwrap();

    let mut paused_state = false;
    for _ in 0..20 {
        if let Some(PlaybackEvent::StateChanged(PlaybackState::Paused)) = controller.try_recv() {
            paused_state = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(paused_state, "Expected Paused state event");

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
