use fluxcut_project::{
    ClipDefinition, Project, ProjectHistory, TimeRational, TrackDefinition, TrackKind,
};
use fluxcut_timeline::TimelineWidget;

#[test]
fn test_timeline_widget_operations_and_history() {
    // Attempt gtk4::init (skip widget test if headless display server is not present)
    if gtk4::init().is_err() {
        eprintln!("Skipping GTK widget test: No display server available");
        return;
    }

    let mut project = Project::default();
    project.tracks.clear();
    project.tracks.push(TrackDefinition {
        id: "v1".to_string(),
        name: "Video 1".to_string(),
        kind: TrackKind::Video,
        muted: false,
        volume: 1.0,
        clips: vec![ClipDefinition {
            id: "clip-main".to_string(),
            asset_id: "asset-1".to_string(),
            timeline_start: TimeRational::ZERO,
            in_point: TimeRational::ZERO,
            out_point: TimeRational::from_seconds(10.0, 1000),
            volume: 1.0,
            muted: false,
            speed: 1.0,
            ..Default::default()
        }],
    });

    let proj_rc = std::rc::Rc::new(std::cell::RefCell::new(project));
    let hist_rc = std::rc::Rc::new(std::cell::RefCell::new(ProjectHistory::default()));

    let widget = TimelineWidget::new();
    widget.bind_project(proj_rc, hist_rc);

    // 1. Playhead seek
    widget.set_playhead_time(TimeRational::from_seconds(4.0, 1000));
    assert_eq!(widget.playhead_time().to_seconds(), 4.0);

    // 2. Split at playhead
    let did_split = widget.split_at_playhead();
    assert!(did_split);
    assert_eq!(
        widget.selected_clip_id(),
        Some("clip-main_split_4000".to_string())
    );

    // 3. Delete selected clip
    let did_delete = widget.delete_selected();
    assert!(did_delete);
    assert_eq!(widget.selected_clip_id(), None);

    // 4. Undo delete
    let did_undo = widget.undo();
    assert!(did_undo);

    // 5. Undo split
    let did_undo_split = widget.undo();
    assert!(did_undo_split);

    // 6. Redo split
    let did_redo = widget.redo();
    assert!(did_redo);

    // 7. Zoom controls
    widget.zoom_in();
    widget.zoom_out();
    widget.zoom_fit();
}
