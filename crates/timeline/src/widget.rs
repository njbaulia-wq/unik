//! Timeline custom widget implementation with GSK snapshot rendering and gestures.

use crate::coords::{TimelineCoords, DEFAULT_PX_PER_SEC};
use crate::interaction::{DragState, HitTarget, TimelineInteractionState};
use crate::renderer::TimelineRenderer;
use fluxcut_project::{Project, ProjectHistory, TimeRational};
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tracing::info;

type SeekCallback = Box<dyn Fn(TimeRational) + 'static>;
type SelectionCallback = Box<dyn Fn(Option<String>) + 'static>;
type ProjectCallback = Box<dyn Fn() + 'static>;

mod imp {
    use super::*;

    pub struct TimelineWidget {
        pub project: Rc<RefCell<Project>>,
        pub history: Rc<RefCell<ProjectHistory>>,
        pub coords: Rc<RefCell<TimelineCoords>>,
        pub state: Rc<RefCell<TimelineInteractionState>>,
        pub playhead_time: Rc<Cell<TimeRational>>,
        pub seek_callbacks: Rc<RefCell<Vec<SeekCallback>>>,
        pub selection_callbacks: Rc<RefCell<Vec<SelectionCallback>>>,
        pub project_callbacks: Rc<RefCell<Vec<ProjectCallback>>>,
    }

    impl Default for TimelineWidget {
        fn default() -> Self {
            Self {
                project: Rc::new(RefCell::new(Project::default())),
                history: Rc::new(RefCell::new(ProjectHistory::default())),
                coords: Rc::new(RefCell::new(TimelineCoords::default())),
                state: Rc::new(RefCell::new(TimelineInteractionState::default())),
                playhead_time: Rc::new(Cell::new(TimeRational::ZERO)),
                seek_callbacks: Rc::new(RefCell::new(Vec::new())),
                selection_callbacks: Rc::new(RefCell::new(Vec::new())),
                project_callbacks: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimelineWidget {
        const NAME: &'static str = "FluxCutTimelineWidget";
        type Type = super::TimelineWidget;
        type ParentType = gtk4::Widget;
    }

    impl ObjectImpl for TimelineWidget {
        fn constructed(&self) {
            self.parent_constructed();
            let widget = self.obj();
            widget.set_focusable(true);
            widget.set_can_focus(true);
            widget.set_focus_on_click(true);
            widget.set_hexpand(true);
            widget.set_vexpand(true);
        }
    }

    impl WidgetImpl for TimelineWidget {
        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let widget = self.obj();
            let width = widget.width() as f32;
            let height = widget.height() as f32;

            if width <= 0.0 || height <= 0.0 {
                return;
            }

            let pango_ctx = widget.pango_context();
            let project = self.project.borrow();
            let coords = self.coords.borrow();
            let state = self.state.borrow();
            let playhead = self.playhead_time.get();

            let params = crate::renderer::RenderParams {
                project: &project,
                coords: &coords,
                state: &state,
                playhead_time: playhead,
                width,
                height,
                pango_ctx: &pango_ctx,
            };

            TimelineRenderer::render(snapshot, &params);
        }

        fn measure(&self, orientation: gtk4::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let project = self.project.borrow();
            let coords = self.coords.borrow();

            match orientation {
                gtk4::Orientation::Horizontal => {
                    let total_dur = project.total_duration();
                    let w = coords.total_width(total_dur) as i32;
                    (w.max(600), w.max(800), -1, -1)
                }
                gtk4::Orientation::Vertical => {
                    let h = coords.total_height(project.tracks.len()) as i32;
                    (h.max(160), h.max(160), -1, -1)
                }
                _ => (100, 100, -1, -1),
            }
        }
    }
}

glib::wrapper! {
    pub struct TimelineWidget(ObjectSubclass<imp::TimelineWidget>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl Default for TimelineWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelineWidget {
    pub fn new() -> Self {
        let widget: Self = glib::Object::builder().build();
        widget.setup_controllers();
        widget
    }

    pub fn with_project(project: Rc<RefCell<Project>>) -> Self {
        let widget: Self = glib::Object::builder().build();
        let imp = widget.imp();
        imp.project.replace(project.borrow().clone());
        widget.setup_controllers();
        widget
    }

    /// Attach shared project and history references
    pub fn bind_project(
        &self,
        project: Rc<RefCell<Project>>,
        history: Rc<RefCell<ProjectHistory>>,
    ) {
        let imp = self.imp();
        imp.project.replace(project.borrow().clone());
        imp.history.replace(history.borrow().clone());
        self.queue_resize();
        self.queue_draw();
    }

    /// Synchronize external project changes into timeline
    pub fn sync_from_project(&self, project: &Project) {
        let imp = self.imp();
        *imp.project.borrow_mut() = project.clone();
        self.queue_resize();
        self.queue_draw();
    }

    pub fn set_playhead_time(&self, time: TimeRational) {
        let imp = self.imp();
        imp.playhead_time.set(time);
        self.queue_draw();
    }

    pub fn playhead_time(&self) -> TimeRational {
        self.imp().playhead_time.get()
    }

    pub fn selected_clip_id(&self) -> Option<String> {
        self.imp().state.borrow().selected_clip_id.clone()
    }

    pub fn zoom_in(&self) {
        self.imp().coords.borrow_mut().zoom_in();
        self.queue_resize();
        self.queue_draw();
    }

    pub fn zoom_out(&self) {
        self.imp().coords.borrow_mut().zoom_out();
        self.queue_resize();
        self.queue_draw();
    }

    pub fn zoom_fit(&self) {
        let imp = self.imp();
        let dur = imp.project.borrow().total_duration().to_seconds();
        let available_w = (self.width() as f32 - imp.coords.borrow().header_width).max(100.0);
        let new_px = if dur > 0.05 {
            (available_w / (dur as f32 * 1.1)).clamp(5.0, 2000.0)
        } else {
            DEFAULT_PX_PER_SEC
        };
        imp.coords.borrow_mut().px_per_sec = new_px;
        self.queue_resize();
        self.queue_draw();
    }

    pub fn split_at_playhead(&self) -> bool {
        let imp = self.imp();
        let playhead = imp.playhead_time.get();
        let mut proj = imp.project.borrow_mut();

        // 1. If a clip is selected, try splitting it
        let target_clip_id = imp.state.borrow().selected_clip_id.clone();
        let clip_to_split = if let Some(ref sel_id) = target_clip_id {
            if let Some((_, c)) = proj.find_clip(sel_id) {
                if playhead > c.timeline_start && playhead < c.timeline_end() {
                    Some(sel_id.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            // Find any clip under playhead
            let mut found = None;
            for track in &proj.tracks {
                for clip in &track.clips {
                    if playhead > clip.timeline_start && playhead < clip.timeline_end() {
                        found = Some(clip.id.clone());
                        break;
                    }
                }
            }
            found
        };

        if let Some(clip_id) = clip_to_split {
            imp.history.borrow_mut().commit(&proj, "Split Clip");
            if let Ok((_head, tail)) = proj.split_clip(&clip_id, playhead) {
                info!(clip_id = %clip_id, split_at = %playhead, "Split clip at playhead");
                drop(proj);
                imp.state.borrow_mut().selected_clip_id = Some(tail.id);
                self.notify_project_changed();
                self.queue_resize();
                self.queue_draw();
                return true;
            }
        }
        false
    }

    pub fn delete_selected(&self) -> bool {
        let imp = self.imp();
        let sel_id = imp.state.borrow().selected_clip_id.clone();
        if let Some(clip_id) = sel_id {
            let mut proj = imp.project.borrow_mut();
            imp.history.borrow_mut().commit(&proj, "Delete Clip");
            if proj.remove_clip(&clip_id).is_ok() {
                info!(clip_id = %clip_id, "Deleted clip");
                drop(proj);
                imp.state.borrow_mut().selected_clip_id = None;
                self.notify_selection_changed(None);
                self.notify_project_changed();
                self.queue_resize();
                self.queue_draw();
                return true;
            }
        }
        false
    }

    pub fn ripple_delete_selected(&self) -> bool {
        let imp = self.imp();
        let sel_id = imp.state.borrow().selected_clip_id.clone();
        if let Some(clip_id) = sel_id {
            let mut proj = imp.project.borrow_mut();
            imp.history.borrow_mut().commit(&proj, "Ripple Delete Clip");
            if proj.ripple_delete_clip(&clip_id).is_ok() {
                info!(clip_id = %clip_id, "Ripple deleted clip");
                drop(proj);
                imp.state.borrow_mut().selected_clip_id = None;
                self.notify_selection_changed(None);
                self.notify_project_changed();
                self.queue_resize();
                self.queue_draw();
                return true;
            }
        }
        false
    }

    /// Toggle mute on the currently selected clip (FR-AUDIO-002).
    pub fn toggle_mute_selected(&self) -> Option<bool> {
        let imp = self.imp();
        let clip_id = imp.state.borrow().selected_clip_id.clone()?;
        let mut proj = imp.project.borrow_mut();
        for track in &mut proj.tracks {
            for clip in &mut track.clips {
                if clip.id == clip_id {
                    clip.muted = !clip.muted;
                    let is_muted = clip.muted;
                    imp.history.borrow_mut().commit(
                        &proj,
                        if is_muted {
                            "Mute Clip Audio"
                        } else {
                            "Unmute Clip Audio"
                        },
                    );
                    drop(proj);
                    self.notify_project_changed();
                    self.queue_draw();
                    return Some(is_muted);
                }
            }
        }
        None
    }

    /// Set volume multiplier on the currently selected clip (FR-AUDIO-004).
    pub fn set_selected_clip_volume(&self, volume: f32) -> bool {
        let imp = self.imp();
        let Some(clip_id) = imp.state.borrow().selected_clip_id.clone() else {
            return false;
        };
        let mut proj = imp.project.borrow_mut();
        for track in &mut proj.tracks {
            for clip in &mut track.clips {
                if clip.id == clip_id {
                    clip.volume = volume.clamp(0.0, 2.0);
                    drop(proj);
                    self.notify_project_changed();
                    self.queue_draw();
                    return true;
                }
            }
        }
        false
    }

    pub fn undo(&self) -> bool {
        let imp = self.imp();
        let mut proj = imp.project.borrow_mut();
        if imp.history.borrow_mut().undo(&mut proj) {
            drop(proj);
            imp.state.borrow_mut().selected_clip_id = None;
            self.notify_project_changed();
            self.queue_resize();
            self.queue_draw();
            return true;
        }
        false
    }

    pub fn redo(&self) -> bool {
        let imp = self.imp();
        let mut proj = imp.project.borrow_mut();
        if imp.history.borrow_mut().redo(&mut proj) {
            drop(proj);
            imp.state.borrow_mut().selected_clip_id = None;
            self.notify_project_changed();
            self.queue_resize();
            self.queue_draw();
            return true;
        }
        false
    }

    pub fn connect_seek<F: Fn(TimeRational) + 'static>(&self, f: F) {
        self.imp().seek_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_selection_changed<F: Fn(Option<String>) + 'static>(&self, f: F) {
        self.imp()
            .selection_callbacks
            .borrow_mut()
            .push(Box::new(f));
    }

    pub fn connect_project_changed<F: Fn() + 'static>(&self, f: F) {
        self.imp().project_callbacks.borrow_mut().push(Box::new(f));
    }

    fn notify_seek(&self, time: TimeRational) {
        self.imp().playhead_time.set(time);
        for cb in self.imp().seek_callbacks.borrow().iter() {
            cb(time);
        }
        self.queue_draw();
    }

    fn notify_selection_changed(&self, clip_id: Option<String>) {
        for cb in self.imp().selection_callbacks.borrow().iter() {
            cb(clip_id.clone());
        }
    }

    fn notify_project_changed(&self) {
        for cb in self.imp().project_callbacks.borrow().iter() {
            cb();
        }
    }

    fn setup_controllers(&self) {
        // 1. Click Gesture
        let click_gesture = gtk4::GestureClick::new();
        let widget_ref = self.clone();

        click_gesture.connect_pressed(move |gesture, _n_press, x, y| {
            let imp = widget_ref.imp();
            let coords = *imp.coords.borrow();
            let proj = imp.project.borrow();
            let hit = imp
                .state
                .borrow()
                .hit_test(x as f32, y as f32, &coords, &proj);

            widget_ref.grab_focus();

            match hit {
                HitTarget::Ruler => {
                    let time = coords.x_to_time(x as f32);
                    let threshold = coords.snap_threshold_time(10.0);
                    let snapped = proj.snap_time(time, threshold, None);
                    imp.state.borrow_mut().drag_state = DragState::ScrubbingPlayhead;
                    widget_ref.notify_seek(snapped);
                }
                HitTarget::ClipBody { clip_id, .. } => {
                    let (_, clip) = proj.find_clip(&clip_id).unwrap();
                    imp.state.borrow_mut().selected_clip_id = Some(clip_id.clone());
                    imp.state.borrow_mut().drag_state = DragState::MovingClip {
                        clip_id: clip_id.clone(),
                        initial_mouse_x: x as f32,
                        initial_start: clip.timeline_start,
                    };
                    widget_ref.notify_selection_changed(Some(clip_id));
                    widget_ref.queue_draw();
                }
                HitTarget::ClipTrimLeft { clip_id, .. } => {
                    let (_, clip) = proj.find_clip(&clip_id).unwrap();
                    imp.state.borrow_mut().selected_clip_id = Some(clip_id.clone());
                    imp.state.borrow_mut().drag_state = DragState::TrimmingLeft {
                        clip_id: clip_id.clone(),
                        initial_mouse_x: x as f32,
                        initial_start: clip.timeline_start,
                        initial_in: clip.in_point,
                    };
                    widget_ref.notify_selection_changed(Some(clip_id));
                    widget_ref.queue_draw();
                }
                HitTarget::ClipTrimRight { clip_id, .. } => {
                    let (_, clip) = proj.find_clip(&clip_id).unwrap();
                    imp.state.borrow_mut().selected_clip_id = Some(clip_id.clone());
                    imp.state.borrow_mut().drag_state = DragState::TrimmingRight {
                        clip_id: clip_id.clone(),
                        initial_mouse_x: x as f32,
                        initial_end: clip.timeline_end(),
                        initial_out: clip.out_point,
                    };
                    widget_ref.notify_selection_changed(Some(clip_id));
                    widget_ref.queue_draw();
                }
                HitTarget::None => {
                    // Click on empty space deselects and seeks
                    imp.state.borrow_mut().selected_clip_id = None;
                    widget_ref.notify_selection_changed(None);
                    if x as f32 >= coords.header_width {
                        let time = coords.x_to_time(x as f32);
                        let threshold = coords.snap_threshold_time(10.0);
                        let snapped = proj.snap_time(time, threshold, None);
                        widget_ref.notify_seek(snapped);
                    }
                    widget_ref.queue_draw();
                }
            }
            gesture.set_state(gtk4::EventSequenceState::Claimed);
        });

        let widget_ref_release = self.clone();
        click_gesture.connect_released(move |_gesture, _n_press, _x, _y| {
            let imp = widget_ref_release.imp();
            let mut state = imp.state.borrow_mut();
            if state.drag_state != DragState::None {
                state.drag_state = DragState::None;
                drop(state);
                widget_ref_release.notify_project_changed();
                widget_ref_release.queue_draw();
            }
        });
        self.add_controller(click_gesture);

        // 2. Drag Gesture for scrubbing, clip move, and trimming
        let drag_gesture = gtk4::GestureDrag::new();
        let widget_ref_drag = self.clone();

        drag_gesture.connect_drag_update(move |gesture, offset_x, _offset_y| {
            let imp = widget_ref_drag.imp();
            let coords = *imp.coords.borrow();
            let current_drag = imp.state.borrow().drag_state.clone();

            match current_drag {
                DragState::ScrubbingPlayhead => {
                    let start_point = gesture.start_point().unwrap_or((0.0, 0.0));
                    let current_x = (start_point.0 + offset_x) as f32;
                    let target_time = coords.x_to_time(current_x);
                    let proj = imp.project.borrow();
                    let threshold = coords.snap_threshold_time(10.0);
                    let snapped = proj.snap_time(target_time, threshold, None);
                    drop(proj);
                    widget_ref_drag.notify_seek(snapped);
                }
                DragState::MovingClip {
                    ref clip_id,
                    initial_mouse_x,
                    initial_start,
                } => {
                    let current_mouse_x = initial_mouse_x + offset_x as f32;
                    let delta_x = current_mouse_x - initial_mouse_x;
                    let delta_sec = (delta_x / coords.px_per_sec) as f64;
                    let delta_time = TimeRational::from_seconds(delta_sec, 1000);
                    let target_start = (initial_start + delta_time).max(TimeRational::ZERO);

                    let mut proj = imp.project.borrow_mut();
                    let threshold = coords.snap_threshold_time(10.0);
                    let snapped = proj.snap_time(target_start, threshold, Some(clip_id));

                    if let Some((_, clip)) = proj.find_clip(clip_id) {
                        let current_clip_start = clip.timeline_start;
                        if current_clip_start != snapped {
                            let track_id = proj.find_clip(clip_id).unwrap().0.id.clone();
                            let _ = proj.move_clip(clip_id, &track_id, snapped);
                            drop(proj);
                            widget_ref_drag.queue_resize();
                            widget_ref_drag.queue_draw();
                        }
                    }
                }
                DragState::TrimmingLeft {
                    ref clip_id,
                    initial_mouse_x,
                    initial_start: _,
                    initial_in: _,
                } => {
                    let current_mouse_x = initial_mouse_x + offset_x as f32;
                    let target_time = coords.x_to_time(current_mouse_x);
                    let mut proj = imp.project.borrow_mut();
                    let threshold = coords.snap_threshold_time(10.0);
                    let snapped = proj.snap_time(target_time, threshold, Some(clip_id));

                    if let Ok(()) = proj.trim_clip_start(clip_id, snapped) {
                        drop(proj);
                        widget_ref_drag.queue_resize();
                        widget_ref_drag.queue_draw();
                    }
                }
                DragState::TrimmingRight {
                    ref clip_id,
                    initial_mouse_x,
                    initial_end: _,
                    initial_out: _,
                } => {
                    let current_mouse_x = initial_mouse_x + offset_x as f32;
                    let target_time = coords.x_to_time(current_mouse_x);
                    let mut proj = imp.project.borrow_mut();
                    let threshold = coords.snap_threshold_time(10.0);
                    let snapped = proj.snap_time(target_time, threshold, Some(clip_id));

                    if let Ok(()) = proj.trim_clip_end(clip_id, snapped) {
                        drop(proj);
                        widget_ref_drag.queue_resize();
                        widget_ref_drag.queue_draw();
                    }
                }
                DragState::None => {}
            }
        });
        self.add_controller(drag_gesture);

        // 3. Motion controller for interactive hover cursor
        let motion_ctrl = gtk4::EventControllerMotion::new();
        let widget_ref_motion = self.clone();

        motion_ctrl.connect_motion(move |_ctrl, x, y| {
            let imp = widget_ref_motion.imp();
            let coords = *imp.coords.borrow();
            let proj = imp.project.borrow();
            let hit = imp
                .state
                .borrow()
                .hit_test(x as f32, y as f32, &coords, &proj);

            match hit {
                HitTarget::ClipTrimLeft { .. } | HitTarget::ClipTrimRight { .. } => {
                    widget_ref_motion.set_cursor_from_name(Some("ew-resize"));
                }
                HitTarget::Ruler => {
                    widget_ref_motion.set_cursor_from_name(Some("pointer"));
                }
                HitTarget::ClipBody { .. } => {
                    widget_ref_motion.set_cursor_from_name(Some("grab"));
                }
                HitTarget::None => {
                    widget_ref_motion.set_cursor_from_name(None);
                }
            }
        });
        self.add_controller(motion_ctrl);

        // 4. Scroll controller (Ctrl + scroll to zoom)
        let scroll_ctrl = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL
                | gtk4::EventControllerScrollFlags::HORIZONTAL,
        );
        let widget_ref_scroll = self.clone();

        scroll_ctrl.connect_scroll(move |ctrl, _dx, dy| {
            let mods = ctrl.current_event_state();
            if mods.contains(gdk::ModifierType::CONTROL_MASK) {
                if dy < 0.0 {
                    widget_ref_scroll.zoom_in();
                } else if dy > 0.0 {
                    widget_ref_scroll.zoom_out();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.add_controller(scroll_ctrl);

        // 5. Keyboard shortcuts on timeline
        let key_ctrl = gtk4::EventControllerKey::new();
        let widget_ref_key = self.clone();

        key_ctrl.connect_key_pressed(move |_ctrl, key, _keycode, mods| {
            match key {
                gdk::Key::s | gdk::Key::S => {
                    if widget_ref_key.split_at_playhead() {
                        return glib::Propagation::Stop;
                    }
                }
                gdk::Key::Delete | gdk::Key::BackSpace => {
                    if mods.contains(gdk::ModifierType::SHIFT_MASK) {
                        if widget_ref_key.ripple_delete_selected() {
                            return glib::Propagation::Stop;
                        }
                    } else if widget_ref_key.delete_selected() {
                        return glib::Propagation::Stop;
                    }
                }
                gdk::Key::z | gdk::Key::Z => {
                    if mods.contains(gdk::ModifierType::CONTROL_MASK) {
                        if mods.contains(gdk::ModifierType::SHIFT_MASK) {
                            widget_ref_key.redo();
                        } else {
                            widget_ref_key.undo();
                        }
                        return glib::Propagation::Stop;
                    }
                }
                gdk::Key::y | gdk::Key::Y => {
                    if mods.contains(gdk::ModifierType::CONTROL_MASK) {
                        widget_ref_key.redo();
                        return glib::Propagation::Stop;
                    }
                }
                gdk::Key::plus | gdk::Key::equal => {
                    widget_ref_key.zoom_in();
                    return glib::Propagation::Stop;
                }
                gdk::Key::minus => {
                    widget_ref_key.zoom_out();
                    return glib::Propagation::Stop;
                }
                gdk::Key::_0 if mods.contains(gdk::ModifierType::CONTROL_MASK) => {
                    widget_ref_key.zoom_fit();
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
            glib::Propagation::Proceed
        });
        self.add_controller(key_ctrl);
    }
}
