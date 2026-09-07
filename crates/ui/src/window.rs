//! Main application window layout, media workspace, video playback viewport, and timeline editor using libadwaita.

use crate::dialogs::{show_about_dialog, show_capabilities_dialog, show_preferences_dialog};
use fluxcut_audio::AudioMonitor;
use fluxcut_cache::DiskCacheManager;
use fluxcut_decode::{PlaybackController, PlaybackEvent, PlaybackState};
use fluxcut_diagnostics::DiagnosticReport;
use fluxcut_export::{ExportController, ExportEvent};
use fluxcut_hardware::GpuCapabilities;
use fluxcut_media::{IngestRequest, MediaAsset, MediaWorkerPool};
use fluxcut_project::{
    AppConfig, AssetReference, CanvasRatio, ClipDefinition, Project, ProjectHistory, TimeRational,
};
use fluxcut_render::VideoPresentationBridge;
use fluxcut_timeline::TimelineWidget;
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::{error, info};

struct ActiveExportState {
    controller: ExportController,
    dialog: adw::Window,
    progress_bar: gtk4::ProgressBar,
    mode_label: gtk4::Label,
    status_label: gtk4::Label,
    eta_label: gtk4::Label,
    start_time: std::time::Instant,
}

pub struct MainWindow {
    pub window: adw::ApplicationWindow,
    pub toast_overlay: adw::ToastOverlay,
    #[allow(dead_code)]
    project: Rc<RefCell<Project>>,
    #[allow(dead_code)]
    history: Rc<RefCell<ProjectHistory>>,
    #[allow(dead_code)]
    config: Rc<RefCell<AppConfig>>,
    #[allow(dead_code)]
    caps: GpuCapabilities,
    #[allow(dead_code)]
    diag: DiagnosticReport,
    #[allow(dead_code)]
    worker_pool: Rc<MediaWorkerPool>,
    #[allow(dead_code)]
    cache: DiskCacheManager,
    #[allow(dead_code)]
    playback: Rc<PlaybackController>,
    #[allow(dead_code)]
    audio_monitor: Rc<AudioMonitor>,
    #[allow(dead_code)]
    render_bridge: Rc<RefCell<VideoPresentationBridge>>,
    pub timeline_widget: TimelineWidget,
    #[allow(dead_code)]
    active_export: Rc<RefCell<Option<ActiveExportState>>>,
}

impl MainWindow {
    pub fn new(
        app: &adw::Application,
        project: Project,
        config: AppConfig,
        caps: GpuCapabilities,
        diag: DiagnosticReport,
    ) -> Self {
        let project = Rc::new(RefCell::new(project));
        let history = Rc::new(RefCell::new(ProjectHistory::default()));
        let config = Rc::new(RefCell::new(config));

        // Initialize media disk cache and worker pool
        let cache = DiskCacheManager::new().unwrap_or_else(|_| {
            DiskCacheManager::with_base_dir(std::env::temp_dir().join("fluxcut_cache")).unwrap()
        });
        let worker_pool = Rc::new(MediaWorkerPool::new(cache.clone()));

        // Initialize background video playback controller
        let playback = Rc::new(PlaybackController::new());
        let audio_monitor = Rc::new(AudioMonitor::new());
        // Attach direct audio output sink to background playback worker for low-latency A/V monitoring
        let _ = playback.attach_audio_sink(audio_monitor.sink_handle());

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("FluxCut")
            .default_width(1280)
            .default_height(840)
            .build();

        let toast_overlay = adw::ToastOverlay::new();
        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        // Header bar
        let header_bar = adw::HeaderBar::new();

        // Project New / Open / Save buttons on the left
        let left_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);

        let new_btn = gtk4::Button::builder()
            .icon_name("document-new-symbolic")
            .tooltip_text("New Project")
            .build();

        let open_btn = gtk4::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Open Project")
            .build();

        let save_btn = gtk4::Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Save Project")
            .build();

        left_box.append(&new_btn);
        left_box.append(&open_btn);
        left_box.append(&save_btn);
        header_bar.pack_start(&left_box);

        // Canvas aspect ratio preset dropdown in center
        let ratio_dropdown = gtk4::DropDown::from_strings(&[
            "16:9 Landscape (1920x1080)",
            "9:16 Vertical (1080x1920)",
            "1:1 Square (1080x1080)",
            "4:3 Classic (1440x1080)",
        ]);
        ratio_dropdown.set_tooltip_text(Some("Project Canvas Aspect Ratio"));
        header_bar.set_title_widget(Some(&ratio_dropdown));

        // Right side: Export, Capabilities, Preferences, Menu
        let right_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);

        let export_btn = gtk4::Button::builder()
            .label("Export")
            .icon_name("document-send-symbolic")
            .tooltip_text("Export Rendered Project (MP4)")
            .css_classes(["suggested-action"])
            .build();

        let caps_btn = gtk4::Button::builder()
            .icon_name("utilities-system-monitor-symbolic")
            .tooltip_text("Hardware & System Capabilities")
            .build();

        let menu_btn = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Main Menu")
            .build();

        // Menu model
        let menu_model = gio::Menu::new();
        menu_model.append(Some("Preferences"), Some("win.preferences"));
        menu_model.append(Some("System Capabilities"), Some("win.capabilities"));
        menu_model.append(Some("About FluxCut"), Some("win.about"));
        menu_btn.set_menu_model(Some(&menu_model));

        right_box.append(&export_btn);
        right_box.append(&caps_btn);
        right_box.append(&menu_btn);
        header_bar.pack_end(&right_box);

        main_box.append(&header_bar);

        // 3-Pane NLE Workspace Layout:
        // Horizontal paned: [Media Library (Left)] | [Preview & Timeline (Right)]
        let h_paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
        h_paned.set_position(340);
        h_paned.set_vexpand(true);

        // --- Left Pane: Media Library ---
        let media_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        media_box.set_margin_start(8);
        media_box.set_margin_end(8);
        media_box.set_margin_top(8);
        media_box.set_margin_bottom(8);

        let media_header = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let media_title = gtk4::Label::builder()
            .label("Media Library")
            .css_classes(["title-4"])
            .hexpand(true)
            .halign(gtk4::Align::Start)
            .build();

        let import_btn = gtk4::Button::builder()
            .icon_name("list-add-symbolic")
            .label("Import")
            .tooltip_text("Import video, audio, or image files")
            .css_classes(["suggested-action"])
            .build();

        media_header.append(&media_title);
        media_header.append(&import_btn);
        media_box.append(&media_header);

        let media_list_box = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();

        let media_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .child(&media_list_box)
            .build();

        media_box.append(&media_scroll);
        h_paned.set_start_child(Some(&media_box));

        // --- Right Pane: Preview Viewport & Timeline ---
        let v_paned = gtk4::Paned::new(gtk4::Orientation::Vertical);
        v_paned.set_position(450);
        v_paned.set_vexpand(true);

        // Top Preview Viewport
        let preview_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        preview_box.set_margin_start(12);
        preview_box.set_margin_end(12);
        preview_box.set_margin_top(8);
        preview_box.set_margin_bottom(6);

        let preview_frame = gtk4::Frame::builder()
            .vexpand(true)
            .hexpand(true)
            .css_classes(["view"])
            .build();

        let preview_picture = gtk4::Picture::builder()
            .can_shrink(true)
            .content_fit(gtk4::ContentFit::Contain)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let preview_overlay = gtk4::Overlay::builder().child(&preview_picture).build();

        let preview_status = gtk4::Label::builder()
            .label("No Media Loaded — Import clips to start previewing")
            .css_classes(["dim-label"])
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        preview_overlay.add_overlay(&preview_status);
        preview_frame.set_child(Some(&preview_overlay));
        preview_box.append(&preview_frame);

        // Playback Transport Controls
        let transport_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        transport_box.set_halign(gtk4::Align::Center);
        transport_box.set_margin_top(4);
        transport_box.set_margin_bottom(4);

        let step_back_btn = gtk4::Button::builder()
            .icon_name("media-seek-backward-symbolic")
            .tooltip_text("Step Backward (Left Arrow)")
            .build();

        let play_btn = gtk4::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .tooltip_text("Play / Pause (Space)")
            .css_classes(["suggested-action", "circular"])
            .build();

        let step_fwd_btn = gtk4::Button::builder()
            .icon_name("media-seek-forward-symbolic")
            .tooltip_text("Step Forward (Right Arrow)")
            .build();

        let timecode_label = gtk4::Label::builder()
            .label("00:00.00 / 00:00.00")
            .css_classes(["monospace", "dim-label"])
            .margin_start(12)
            .build();

        let scrubber = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 1.0, 0.01);
        scrubber.set_hexpand(true);
        scrubber.set_margin_start(12);
        scrubber.set_margin_end(12);

        transport_box.append(&step_back_btn);
        transport_box.append(&play_btn);
        transport_box.append(&step_fwd_btn);
        transport_box.append(&timecode_label);
        transport_box.append(&scrubber);
        preview_box.append(&transport_box);

        v_paned.set_start_child(Some(&preview_box));

        // Bottom Timeline Viewport with Timeline Toolbar and Custom GSK TimelineWidget
        let timeline_container = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        timeline_container.set_margin_start(12);
        timeline_container.set_margin_end(12);
        timeline_container.set_margin_bottom(12);

        let timeline_header = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let tl_label = gtk4::Label::builder()
            .label("Timeline")
            .css_classes(["heading"])
            .build();
        timeline_header.append(&tl_label);

        // Timeline Toolbar Buttons
        let split_btn = gtk4::Button::builder()
            .icon_name("edit-cut-symbolic")
            .tooltip_text("Split Clip at Playhead (S)")
            .build();

        let delete_btn = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Delete Selected Clip (Delete)")
            .build();

        let ripple_btn = gtk4::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text("Ripple Delete Selected Clip (Shift+Delete)")
            .build();

        let undo_btn = gtk4::Button::builder()
            .icon_name("edit-undo-symbolic")
            .tooltip_text("Undo (Ctrl+Z)")
            .build();

        let redo_btn = gtk4::Button::builder()
            .icon_name("edit-redo-symbolic")
            .tooltip_text("Redo (Ctrl+Shift+Z)")
            .build();

        let zoom_out_btn = gtk4::Button::builder()
            .icon_name("zoom-out-symbolic")
            .tooltip_text("Zoom Out (-)")
            .build();

        let zoom_fit_btn = gtk4::Button::builder()
            .icon_name("zoom-fit-best-symbolic")
            .tooltip_text("Zoom to Fit (Ctrl+0)")
            .build();

        let zoom_in_btn = gtk4::Button::builder()
            .icon_name("zoom-in-symbolic")
            .tooltip_text("Zoom In (+)")
            .build();

        let mute_clip_btn = gtk4::Button::builder()
            .icon_name("audio-volume-muted-symbolic")
            .tooltip_text("Mute / Unmute Selected Clip (M)")
            .build();

        let tl_tools_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        tl_tools_box.set_hexpand(true);
        tl_tools_box.set_halign(gtk4::Align::End);
        tl_tools_box.append(&split_btn);
        tl_tools_box.append(&delete_btn);
        tl_tools_box.append(&ripple_btn);
        tl_tools_box.append(&mute_clip_btn);
        tl_tools_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        tl_tools_box.append(&undo_btn);
        tl_tools_box.append(&redo_btn);
        tl_tools_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        tl_tools_box.append(&zoom_out_btn);
        tl_tools_box.append(&zoom_fit_btn);
        tl_tools_box.append(&zoom_in_btn);

        timeline_header.append(&tl_tools_box);
        timeline_container.append(&timeline_header);

        // Timeline Widget
        let timeline_widget = TimelineWidget::new();
        timeline_widget.bind_project(Rc::clone(&project), Rc::clone(&history));

        let timeline_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&timeline_widget)
            .css_classes(["view"])
            .build();

        timeline_container.append(&timeline_scroll);
        v_paned.set_end_child(Some(&timeline_container));

        h_paned.set_end_child(Some(&v_paned));
        main_box.append(&h_paned);

        toast_overlay.set_child(Some(&main_box));
        window.set_content(Some(&toast_overlay));

        // Presentation bridge for rendering decoded frames
        let render_bridge = Rc::new(RefCell::new(VideoPresentationBridge::new(
            preview_picture.clone(),
            caps.clone(),
        )));

        // Shared state
        let is_playing = Rc::new(Cell::new(false));
        let total_duration_sec = Rc::new(Cell::new(0.0f64));
        let is_user_scrubbing = Rc::new(Cell::new(false));
        let active_rows: Rc<RefCell<HashMap<String, (adw::ActionRow, PathBuf)>>> =
            Rc::new(RefCell::new(HashMap::new()));

        // Header bar New / Save / Open Actions
        let proj_new = Rc::clone(&project);
        let hist_new = Rc::clone(&history);
        let tl_new = timeline_widget.clone();
        let toast_new = toast_overlay.clone();
        new_btn.connect_clicked(move |_| {
            *proj_new.borrow_mut() = Project::default();
            *hist_new.borrow_mut() = ProjectHistory::default();
            tl_new.refresh();
            toast_new.add_toast(adw::Toast::new("Created new project"));
        });

        let proj_save = Rc::clone(&project);
        let win_save = window.clone();
        let toast_save = toast_overlay.clone();
        save_btn.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Save FluxCut Project")
                .initial_name("project.fluxcut")
                .modal(true)
                .build();
            let p_ref = Rc::clone(&proj_save);
            let t_ref = toast_save.clone();
            file_dialog.save(Some(&win_save), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        if let Err(e) = p_ref.borrow().save_to_file(&path) {
                            t_ref.add_toast(adw::Toast::new(&format!("Error saving: {}", e)));
                        } else {
                            t_ref.add_toast(adw::Toast::new("Project saved successfully"));
                        }
                    }
                }
            });
        });

        let proj_open = Rc::clone(&project);
        let tl_open = timeline_widget.clone();
        let win_open = window.clone();
        let toast_open = toast_overlay.clone();
        open_btn.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Open FluxCut Project")
                .modal(true)
                .build();
            let p_ref = Rc::clone(&proj_open);
            let tl_ref = tl_open.clone();
            let t_ref = toast_open.clone();
            file_dialog.open(Some(&win_open), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        match Project::load_from_file(&path) {
                            Ok(loaded) => {
                                *p_ref.borrow_mut() = loaded;
                                tl_ref.refresh();
                                t_ref.add_toast(adw::Toast::new("Project loaded successfully"));
                            }
                            Err(e) => {
                                t_ref.add_toast(adw::Toast::new(&format!("Error opening: {}", e)));
                            }
                        }
                    }
                }
            });
        });

        // Ratio preset dropdown change handler
        let proj_ratio = Rc::clone(&project);
        let tl_ratio = timeline_widget.clone();
        let toast_ratio = toast_overlay.clone();
        ratio_dropdown.connect_selected_notify(move |dd| {
            let ratio = match dd.selected() {
                1 => CanvasRatio::Vertical9x16,
                2 => CanvasRatio::Square1x1,
                3 => CanvasRatio::Classic4x3,
                _ => CanvasRatio::Landscape16x9,
            };
            proj_ratio.borrow_mut().set_canvas_ratio(ratio);
            tl_ratio.refresh();
            toast_ratio.add_toast(adw::Toast::new(&format!(
                "Canvas aspect ratio set to: {}",
                ratio.label()
            )));
        });

        // Active background export controller
        let active_export: Rc<RefCell<Option<ActiveExportState>>> = Rc::new(RefCell::new(None));
        let proj_exp = Rc::clone(&project);
        let win_exp = window.clone();
        let active_exp_ref = Rc::clone(&active_export);
        let toast_exp = toast_overlay.clone();

        export_btn.connect_clicked(move |_| {
            if proj_exp.borrow().total_duration().to_seconds() <= 0.001 {
                toast_exp.add_toast(adw::Toast::new(
                    "Cannot export empty project: add a clip to the timeline first",
                ));
                return;
            }

            let file_dialog = gtk4::FileDialog::builder()
                .title("Export Video File")
                .initial_name("output.mp4")
                .modal(true)
                .build();
            let p_ref = Rc::clone(&proj_exp);
            let act_ref = Rc::clone(&active_exp_ref);
            let t_ref = toast_exp.clone();
            let parent_win = win_exp.clone();

            file_dialog.save(Some(&win_exp), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        let export_dialog = adw::Window::builder()
                            .transient_for(&parent_win)
                            .modal(true)
                            .title("Exporting Video…")
                            .default_width(440)
                            .default_height(220)
                            .resizable(false)
                            .build();

                        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
                        vbox.set_margin_top(20);
                        vbox.set_margin_bottom(20);
                        vbox.set_margin_start(24);
                        vbox.set_margin_end(24);

                        let file_title = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "output.mp4".to_string());
                        let heading = gtk4::Label::builder()
                            .label(format!("Exporting: {}", file_title))
                            .css_classes(["title-3"])
                            .halign(gtk4::Align::Start)
                            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
                            .build();

                        let mode_lbl = gtk4::Label::builder()
                            .label("Preparing render engine…")
                            .css_classes(["caption", "dim-label"])
                            .halign(gtk4::Align::Start)
                            .build();

                        let pbar = gtk4::ProgressBar::builder()
                            .fraction(0.0)
                            .show_text(true)
                            .text("0.0%")
                            .margin_top(8)
                            .margin_bottom(4)
                            .build();

                        let status_lbl = gtk4::Label::builder()
                            .label("Rendering timeline…")
                            .css_classes(["body"])
                            .halign(gtk4::Align::Start)
                            .build();

                        let eta_lbl = gtk4::Label::builder()
                            .label("Elapsed: 00:00 • Remaining: Calculating…")
                            .css_classes(["caption", "dim-label"])
                            .halign(gtk4::Align::Start)
                            .build();

                        let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                        btn_box.set_halign(gtk4::Align::End);
                        btn_box.set_margin_top(12);

                        let cancel_btn = gtk4::Button::builder()
                            .label("Cancel Export")
                            .css_classes(["destructive-action"])
                            .build();

                        btn_box.append(&cancel_btn);

                        vbox.append(&heading);
                        vbox.append(&mode_lbl);
                        vbox.append(&pbar);
                        vbox.append(&status_lbl);
                        vbox.append(&eta_lbl);
                        vbox.append(&btn_box);

                        export_dialog.set_content(Some(&vbox));
                        export_dialog.present();

                        let ctrl = ExportController::start(p_ref.borrow().clone(), path);

                        let act_cancel = Rc::clone(&act_ref);
                        cancel_btn.connect_clicked(move |b| {
                            b.set_sensitive(false);
                            if let Some(ref state) = *act_cancel.borrow() {
                                state.controller.cancel();
                                state.status_label.set_label("Cancelling export…");
                            }
                        });

                        *act_ref.borrow_mut() = Some(ActiveExportState {
                            controller: ctrl,
                            dialog: export_dialog,
                            progress_bar: pbar,
                            mode_label: mode_lbl,
                            status_label: status_lbl,
                            eta_label: eta_lbl,
                            start_time: std::time::Instant::now(),
                        });

                        t_ref.add_toast(adw::Toast::new("Started background export…"));
                    }
                }
            });
        });

        // Timeline Toolbar Button Click Handlers
        let tl_split = timeline_widget.clone();
        split_btn.connect_clicked(move |_| {
            tl_split.split_at_playhead();
        });

        let tl_del = timeline_widget.clone();
        delete_btn.connect_clicked(move |_| {
            tl_del.delete_selected();
        });

        let tl_rip = timeline_widget.clone();
        ripple_btn.connect_clicked(move |_| {
            tl_rip.ripple_delete_selected();
        });

        let tl_mute = timeline_widget.clone();
        let toast_mute = toast_overlay.clone();
        mute_clip_btn.connect_clicked(move |_| {
            if let Some(muted) = tl_mute.toggle_mute_selected() {
                let msg = if muted {
                    "Selected clip audio muted"
                } else {
                    "Selected clip audio unmuted"
                };
                toast_mute.add_toast(adw::Toast::new(msg));
            } else {
                toast_mute.add_toast(adw::Toast::new("Click a clip first to mute/unmute"));
            }
        });

        let tl_undo = timeline_widget.clone();
        undo_btn.connect_clicked(move |_| {
            tl_undo.undo();
        });

        let tl_redo = timeline_widget.clone();
        redo_btn.connect_clicked(move |_| {
            tl_redo.redo();
        });

        let tl_zo = timeline_widget.clone();
        zoom_out_btn.connect_clicked(move |_| {
            tl_zo.zoom_out();
        });

        let tl_zf = timeline_widget.clone();
        zoom_fit_btn.connect_clicked(move |_| {
            tl_zf.zoom_fit();
        });

        let tl_zi = timeline_widget.clone();
        zoom_in_btn.connect_clicked(move |_| {
            tl_zi.zoom_in();
        });

        // Connect seek signal from TimelineWidget to PlaybackController
        let playback_tl_seek = Rc::clone(&playback);
        timeline_widget.connect_seek(move |time| {
            let _ = playback_tl_seek.seek(time.to_seconds());
        });

        // Connect project modification signal from TimelineWidget
        let total_dur_proj = Rc::clone(&total_duration_sec);
        let scrubber_proj = scrubber.clone();
        let proj_ref = Rc::clone(&project);
        timeline_widget.connect_project_changed(move || {
            let dur = proj_ref.borrow().total_duration().to_seconds();
            if dur > 0.0 {
                total_dur_proj.set(dur);
                scrubber_proj.set_range(0.0, dur.max(0.01));
            }
        });

        // Connect action signals
        let win_clone = window.clone();
        let caps_clone = caps.clone();
        let diag_clone = diag.clone();
        caps_btn.connect_clicked(move |_| {
            show_capabilities_dialog(&win_clone, &caps_clone, &diag_clone);
        });

        let action_caps = gio::SimpleAction::new("capabilities", None);
        let win_clone2 = window.clone();
        let caps_clone2 = caps.clone();
        let diag_clone2 = diag.clone();
        action_caps.connect_activate(move |_, _| {
            show_capabilities_dialog(&win_clone2, &caps_clone2, &diag_clone2);
        });
        window.add_action(&action_caps);

        let action_about = gio::SimpleAction::new("about", None);
        let win_clone3 = window.clone();
        action_about.connect_activate(move |_, _| {
            show_about_dialog(&win_clone3);
        });
        window.add_action(&action_about);

        let action_prefs = gio::SimpleAction::new("preferences", None);
        let win_clone4 = window.clone();
        let config_clone = config.clone();
        action_prefs.connect_activate(move |_, _| {
            show_preferences_dialog(&win_clone4, &config_clone.borrow());
        });
        window.add_action(&action_prefs);

        // Async file chooser for Media Import
        let win_clone5 = window.clone();
        let toast_clone = toast_overlay.clone();
        let worker_tx_clone = Rc::clone(&worker_pool);
        let active_rows_clone = Rc::clone(&active_rows);
        let list_box_clone = media_list_box.clone();

        import_btn.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Select Video, Audio, or Image Files")
                .modal(true)
                .build();

            let toast_sender = toast_clone.clone();
            let worker_sender = Rc::clone(&worker_tx_clone);
            let rows_ref = Rc::clone(&active_rows_clone);
            let list_box = list_box_clone.clone();

            file_dialog.open_multiple(Some(&win_clone5), gio::Cancellable::NONE, move |result| {
                match result {
                    Ok(files) => {
                        let count = files.n_items();
                        info!("User selected {} media files", count);

                        for i in 0..count {
                            if let Some(item) = files.item(i) {
                                if let Ok(file) = item.downcast::<gio::File>() {
                                    if let Some(path) = file.path() {
                                        dispatch_media_import(
                                            path,
                                            &worker_sender,
                                            &list_box,
                                            &rows_ref,
                                            &toast_sender,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if !e.matches(gtk4::DialogError::Dismissed) {
                            tracing::warn!("File dialog error: {}", e);
                        }
                    }
                }
            });
        });

        // Load clip into playback when clicked in media library
        let playback_load_clone = Rc::clone(&playback);
        let active_rows_click_clone = Rc::clone(&active_rows);
        let preview_status_click = preview_status.clone();

        media_list_box.connect_row_activated(move |_lb, row| {
            for (r, path) in active_rows_click_clone.borrow().values() {
                if r.upcast_ref::<gtk4::ListBoxRow>() == row {
                    info!(path = %path.display(), "Loading clip for playback preview");
                    let _ = playback_load_clone.load(path.clone());
                    preview_status_click.set_visible(false);
                    break;
                }
            }
        });

        // Transport button controls
        let playback_play_clone = Rc::clone(&playback);
        let is_playing_clone = Rc::clone(&is_playing);
        play_btn.connect_clicked(move |_| {
            if is_playing_clone.get() {
                let _ = playback_play_clone.pause();
            } else {
                let _ = playback_play_clone.play(1.0);
            }
        });

        let playback_back_clone = Rc::clone(&playback);
        step_back_btn.connect_clicked(move |_| {
            let _ = playback_back_clone.step_frame(false);
        });

        let playback_fwd_clone = Rc::clone(&playback);
        step_fwd_btn.connect_clicked(move |_| {
            let _ = playback_fwd_clone.step_frame(true);
        });

        // Scrubber seeking control (coalesced seeking)
        let playback_seek_clone = Rc::clone(&playback);
        let is_scrubbing_clone = Rc::clone(&is_user_scrubbing);
        scrubber.connect_value_changed(move |sc| {
            if is_scrubbing_clone.get() {
                let pos = sc.value();
                let _ = playback_seek_clone.seek(pos);
            }
        });

        // Keyboard navigation (Space, J/K/L, Left/Right, Mute, Split, Delete)
        let key_controller = gtk4::EventControllerKey::new();
        let playback_key_clone = Rc::clone(&playback);
        let is_playing_key_clone = Rc::clone(&is_playing);
        let tl_key_clone = timeline_widget.clone();
        let toast_key_clone = toast_overlay.clone();

        key_controller.connect_key_pressed(move |_ctrl, key, _keycode, _modifier| match key {
            gdk::Key::space => {
                if is_playing_key_clone.get() {
                    let _ = playback_key_clone.pause();
                } else {
                    let _ = playback_key_clone.play(1.0);
                }
                glib::Propagation::Stop
            }
            gdk::Key::j | gdk::Key::J => {
                let _ = playback_key_clone.step_frame(false);
                glib::Propagation::Stop
            }
            gdk::Key::k | gdk::Key::K => {
                let _ = playback_key_clone.pause();
                glib::Propagation::Stop
            }
            gdk::Key::l | gdk::Key::L => {
                let _ = playback_key_clone.play(1.0);
                glib::Propagation::Stop
            }
            gdk::Key::m | gdk::Key::M => {
                if let Some(muted) = tl_key_clone.toggle_mute_selected() {
                    let msg = if muted {
                        "Selected clip audio muted"
                    } else {
                        "Selected clip audio unmuted"
                    };
                    toast_key_clone.add_toast(adw::Toast::new(msg));
                }
                glib::Propagation::Stop
            }
            gdk::Key::s | gdk::Key::S => {
                tl_key_clone.split_at_playhead();
                glib::Propagation::Stop
            }
            gdk::Key::Delete | gdk::Key::BackSpace => {
                tl_key_clone.delete_selected();
                glib::Propagation::Stop
            }
            gdk::Key::Left => {
                let _ = playback_key_clone.step_frame(false);
                glib::Propagation::Stop
            }
            gdk::Key::Right => {
                let _ = playback_key_clone.step_frame(true);
                glib::Propagation::Stop
            }
            gdk::Key::Home => {
                let _ = playback_key_clone.seek(0.0);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        window.add_controller(key_controller);

        // Background event loop: Ingestion responses + Playback frame delivery
        let worker_poll_clone = Rc::clone(&worker_pool);
        let active_rows_poll = Rc::clone(&active_rows);
        let project_poll = Rc::clone(&project);
        let history_poll = Rc::clone(&history);
        let timeline_poll = timeline_widget.clone();
        let toast_poll = toast_overlay.clone();
        let preview_status_clone = preview_status.clone();

        let playback_poll_clone = Rc::clone(&playback);
        let audio_monitor_poll = Rc::clone(&audio_monitor);
        let render_bridge_clone = Rc::clone(&render_bridge);
        let play_btn_clone = play_btn.clone();
        let scrubber_clone = scrubber.clone();
        let timecode_clone = timecode_label.clone();
        let total_dur_clone = Rc::clone(&total_duration_sec);
        let is_playing_state_clone = Rc::clone(&is_playing);
        let is_scrubbing_flag = Rc::clone(&is_user_scrubbing);
        let timeline_widget_clone = timeline_widget.clone();
        let active_exp_poll = Rc::clone(&active_export);
        let playback_for_ingest = Rc::clone(&playback);

        glib::timeout_add_local(std::time::Duration::from_millis(20), move || {
            // 1. Process media ingestion responses
            while let Some(resp) = worker_poll_clone.try_recv() {
                handle_ingest_response(
                    resp,
                    &active_rows_poll,
                    &project_poll,
                    &history_poll,
                    &timeline_poll,
                    &playback_for_ingest,
                    &toast_poll,
                    &preview_status_clone,
                );
            }

            // 2. Process playback events (Frames, Loaded metadata, State changes, Audio samples)
            while let Some(event) = playback_poll_clone.try_recv() {
                match event {
                    PlaybackEvent::Loaded {
                        duration_sec,
                        width,
                        height,
                        fps,
                    } => {
                        total_dur_clone.set(duration_sec);
                        scrubber_clone.set_range(0.0, duration_sec.max(0.01));
                        timecode_clone
                            .set_label(&format!("00:00.00 / {}", format_time(duration_sec)));
                        toast_poll.add_toast(adw::Toast::new(&format!(
                            "Playback loaded: {}x{} ({:.0} fps)",
                            width, height, fps
                        )));
                    }
                    PlaybackEvent::Frame(frame) => {
                        render_bridge_clone.borrow_mut().present_frame(&frame);
                        let dur = total_dur_clone.get();
                        timecode_clone.set_label(&format!(
                            "{} / {}",
                            format_time(frame.pts_sec),
                            format_time(dur)
                        ));

                        // Sync playhead with decoded video frame
                        timeline_widget_clone
                            .set_playhead_time(TimeRational::from_seconds(frame.pts_sec, 1000));

                        // Update scrubber without triggering seek feedback loop
                        is_scrubbing_flag.set(false);
                        scrubber_clone.set_value(frame.pts_sec);
                        is_scrubbing_flag.set(true);
                    }
                    PlaybackEvent::AudioSamples(samples) => {
                        audio_monitor_poll.push_samples(&samples);
                    }
                    PlaybackEvent::Seeked { .. } => {
                        audio_monitor_poll.clear();
                    }
                    PlaybackEvent::StateChanged(state) => match state {
                        PlaybackState::Playing => {
                            is_playing_state_clone.set(true);
                            play_btn_clone.set_icon_name("media-playback-pause-symbolic");
                            audio_monitor_poll.set_active(true);
                        }
                        PlaybackState::Paused | PlaybackState::Idle => {
                            is_playing_state_clone.set(false);
                            play_btn_clone.set_icon_name("media-playback-start-symbolic");
                            audio_monitor_poll.set_active(false);
                            audio_monitor_poll.clear();
                        }
                        PlaybackState::Seeking => {
                            audio_monitor_poll.clear();
                        }
                    },
                    PlaybackEvent::Eof => {
                        is_playing_state_clone.set(false);
                        play_btn_clone.set_icon_name("media-playback-start-symbolic");
                        audio_monitor_poll.set_active(false);
                        audio_monitor_poll.clear();
                    }
                    PlaybackEvent::Error(err) => {
                        error!("Playback error: {}", err);
                        toast_poll.add_toast(adw::Toast::new(&format!("Playback error: {}", err)));
                    }
                }
            }

            // 3. Process background export events
            let mut export_done = false;
            if let Some(ref mut exp_state) = *active_exp_poll.borrow_mut() {
                while let Some(event) = exp_state.controller.try_recv() {
                    match event {
                        ExportEvent::Started { mode } => {
                            exp_state.mode_label.set_label(mode.label());
                            toast_poll.add_toast(adw::Toast::new(&format!(
                                "Export started: {}",
                                mode.label()
                            )));
                        }
                        ExportEvent::Progress(prog) => {
                            let fraction = prog.progress_pct.clamp(0.0, 1.0) as f64;
                            exp_state.progress_bar.set_fraction(fraction);
                            exp_state
                                .progress_bar
                                .set_text(Some(&format!("{:.1}%", fraction * 100.0)));

                            let total_fr = prog.total_frames.max(1);
                            let cur_fr = prog.current_frame.min(total_fr);
                            exp_state.status_label.set_label(&format!(
                                "Frame: {}/{} ({:.1}s / {:.1}s)",
                                cur_fr, total_fr, prog.current_sec, prog.total_sec
                            ));

                            let elapsed = exp_state.start_time.elapsed().as_secs_f64();
                            let eta_str = if elapsed > 0.5
                                && prog.current_sec > 0.05
                                && prog.current_sec < prog.total_sec
                            {
                                let rate = prog.current_sec / elapsed;
                                let rem_sec = (prog.total_sec - prog.current_sec) / rate;
                                format!(
                                    "Elapsed: {} • Remaining: ~{}",
                                    format_time(elapsed),
                                    format_time(rem_sec)
                                )
                            } else {
                                format!("Elapsed: {}", format_time(elapsed))
                            };
                            exp_state.eta_label.set_label(&eta_str);
                        }
                        ExportEvent::Finished { mode: _, report } => {
                            exp_state.progress_bar.set_fraction(1.0);
                            exp_state.progress_bar.set_text(Some("100.0%"));
                            exp_state.dialog.close();

                            let mb = report.file_size_bytes as f64 / (1024.0 * 1024.0);
                            let dims = match (report.width, report.height) {
                                (Some(w), Some(h)) => format!(" [{}x{}]", w, h),
                                _ => String::new(),
                            };
                            toast_poll.add_toast(adw::Toast::new(&format!(
                                "Export complete{} ({:.1} MB, {:.1}s)",
                                dims, mb, report.duration_sec
                            )));
                            export_done = true;
                        }
                        ExportEvent::Cancelled => {
                            exp_state.dialog.close();
                            toast_poll.add_toast(adw::Toast::new("Export was cancelled"));
                            export_done = true;
                        }
                        ExportEvent::Error(err) => {
                            exp_state.dialog.close();
                            error!("Export failed: {}", err);
                            toast_poll
                                .add_toast(adw::Toast::new(&format!("Export failed: {}", err)));
                            export_done = true;
                        }
                    }
                }
            }
            if export_done {
                *active_exp_poll.borrow_mut() = None;
            }

            glib::ControlFlow::Continue
        });

        Self {
            window,
            toast_overlay,
            project,
            history,
            config,
            caps,
            diag,
            worker_pool,
            cache,
            playback,
            audio_monitor,
            render_bridge,
            timeline_widget,
            active_export,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

fn format_time(seconds: f64) -> String {
    let s = seconds.max(0.0);
    let mins = (s / 60.0).floor() as u64;
    let secs = s % 60.0;
    format!("{:02}:{:05.2}", mins, secs)
}

fn dispatch_media_import(
    path: PathBuf,
    worker: &MediaWorkerPool,
    list_box: &gtk4::ListBox,
    active_rows: &Rc<RefCell<HashMap<String, (adw::ActionRow, PathBuf)>>>,
    toast: &adw::ToastOverlay,
) {
    match MediaAsset::from_path(&path) {
        Ok(asset) => {
            let file_name = asset.file_name.clone();
            let asset_id = asset.id.clone();

            let row = adw::ActionRow::builder()
                .title(&file_name)
                .subtitle("Analyzing media metadata in background…")
                .activatable(true)
                .build();

            let spinner = gtk4::Spinner::builder()
                .spinning(true)
                .margin_start(4)
                .margin_end(8)
                .build();
            row.add_prefix(&spinner);

            list_box.append(&row);
            active_rows
                .borrow_mut()
                .insert(asset_id.clone(), (row, path.clone()));

            let req = IngestRequest {
                asset_id,
                path,
                modified_timestamp: asset.modified_timestamp,
                file_size: asset.file_size_bytes,
                extract_thumbnail: true,
                extract_waveform: false,
            };

            if let Err(e) = worker.submit(req) {
                error!("Failed to queue media import: {}", e);
                toast.add_toast(adw::Toast::new(&format!(
                    "Error importing {}: {}",
                    file_name, e
                )));
            } else {
                toast.add_toast(adw::Toast::new(&format!("Importing {}…", file_name)));
            }
        }
        Err(e) => {
            error!(path = %path.display(), error = %e, "Cannot access media file");
            toast.add_toast(adw::Toast::new(&format!("Cannot open file: {}", e)));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_ingest_response(
    resp: fluxcut_media::IngestResponse,
    active_rows: &Rc<RefCell<HashMap<String, (adw::ActionRow, PathBuf)>>>,
    project: &Rc<RefCell<Project>>,
    history: &Rc<RefCell<ProjectHistory>>,
    timeline_widget: &TimelineWidget,
    playback: &Rc<PlaybackController>,
    toast: &adw::ToastOverlay,
    preview_status: &gtk4::Label,
) {
    if let Some((row, path)) = active_rows.borrow().get(&resp.asset_id) {
        match resp.result {
            Ok(metadata) => {
                let mut subtitle_parts = Vec::new();
                subtitle_parts.push(format!("{:.1}s", metadata.duration.to_seconds()));

                if let Some(ref v) = metadata.video {
                    subtitle_parts.push(format!(
                        "{}x{} ({} fps, {})",
                        v.width,
                        v.height,
                        v.frame_rate.to_seconds().round(),
                        v.codec
                    ));
                }
                if let Some(ref a) = metadata.audio {
                    subtitle_parts.push(format!("Audio: {} {}Hz", a.codec, a.sample_rate));
                }

                row.set_subtitle(&subtitle_parts.join(" • "));

                // Replace spinner with thumbnail if available
                let mut thumb_added = false;
                if let (Some(rgba_bytes), Some((w, h))) =
                    (resp.thumbnail_rgba, resp.thumbnail_dimensions)
                {
                    let expected_size = (w * h * 4) as usize;
                    if w > 0 && h > 0 && rgba_bytes.len() >= expected_size {
                        let glib_bytes = glib::Bytes::from(&rgba_bytes[..expected_size]);
                        let texture = gdk::MemoryTexture::new(
                            w as i32,
                            h as i32,
                            gdk::MemoryFormat::R8g8b8a8,
                            &glib_bytes,
                            (w * 4) as usize,
                        );
                        let picture = gtk4::Picture::for_paintable(&texture);
                        picture.set_size_request(64, 36);
                        picture.set_margin_start(4);
                        picture.set_margin_end(8);

                        row.add_prefix(&picture);
                        thumb_added = true;
                    }
                }
                if !thumb_added {
                    let icon = gtk4::Image::from_icon_name("video-x-generic-symbolic");
                    row.add_prefix(&icon);
                }

                // Register asset in project state with real path
                let asset_ref = AssetReference {
                    id: resp.asset_id.clone(),
                    file_path: path.clone(),
                    display_name: row.title().to_string(),
                    duration: metadata.duration,
                    width: metadata.video.as_ref().map(|v| v.width),
                    height: metadata.video.as_ref().map(|v| v.height),
                    has_audio: metadata.audio.is_some(),
                    has_video: metadata.video.is_some(),
                };
                project.borrow_mut().assets.push(asset_ref);

                // Add "Add to Timeline" button on action row
                let add_tl_btn = gtk4::Button::builder()
                    .icon_name("list-add-symbolic")
                    .tooltip_text("Add clip to Timeline")
                    .css_classes(["flat", "circular"])
                    .build();

                let proj_add = Rc::clone(project);
                let hist_add = Rc::clone(history);
                let tl_add = timeline_widget.clone();
                let asset_id_add = resp.asset_id.clone();
                let dur_add = metadata.duration;
                let has_video = metadata.video.is_some();
                let toast_add = toast.clone();
                let title_add = row.title().to_string();

                add_tl_btn.connect_clicked(move |_| {
                    let mut p = proj_add.borrow_mut();
                    hist_add.borrow_mut().commit(&p, "Add Clip to Timeline");
                    let track_id = if has_video { "track-v1" } else { "track-a1" };
                    let start_pos = p
                        .tracks
                        .iter()
                        .find(|t| t.id == track_id)
                        .and_then(|t| t.clips.iter().map(|c| c.timeline_end()).max())
                        .unwrap_or(TimeRational::ZERO);
                    let clip = ClipDefinition {
                        id: format!("clip_{}_{}", asset_id_add, p.total_duration().num),
                        asset_id: asset_id_add.clone(),
                        timeline_start: start_pos,
                        in_point: TimeRational::ZERO,
                        out_point: dur_add,
                        volume: 1.0,
                        muted: false,
                        speed: 1.0,
                        ..Default::default()
                    };
                    if let Ok(()) = p.add_clip(track_id, clip) {
                        drop(p);
                        tl_add.refresh();
                        toast_add.add_toast(adw::Toast::new(&format!(
                            "Added '{}' to timeline",
                            title_add
                        )));
                    }
                });
                row.add_suffix(&add_tl_btn);

                // Auto-place first clip if timeline is currently empty, and configure project resolution
                {
                    let mut p = project.borrow_mut();
                    let is_empty = p.tracks.iter().all(|t| t.clips.is_empty());
                    if is_empty {
                        history.borrow_mut().commit(&p, "Add First Clip");
                        if let Some(ref v) = metadata.video {
                            p.settings.width = v.width;
                            p.settings.height = v.height;
                            let fps = v.frame_rate.to_seconds().round() as u32;
                            if fps > 0 {
                                p.settings.fps_num = fps;
                                p.settings.fps_den = 1;
                            }
                        }
                        let track_id = if metadata.video.is_some() {
                            "track-v1"
                        } else {
                            "track-a1"
                        };
                        let clip = ClipDefinition {
                            id: format!("clip_{}_0", resp.asset_id),
                            asset_id: resp.asset_id.clone(),
                            timeline_start: TimeRational::ZERO,
                            in_point: TimeRational::ZERO,
                            out_point: metadata.duration,
                            volume: 1.0,
                            muted: false,
                            speed: 1.0,
                            ..Default::default()
                        };
                        let _ = p.add_clip(track_id, clip);
                        drop(p);
                        timeline_widget.refresh();

                        // Automatically load the clip into playback preview!
                        let _ = playback.load(path.clone());
                        preview_status.set_visible(false);
                    }
                }

                toast.add_toast(adw::Toast::new(&format!("Ready: {}", row.title())));
            }
            Err(e) => {
                row.set_subtitle(&format!("Error: {}", e));
                let warn_icon = gtk4::Image::from_icon_name("dialog-warning-symbolic");
                row.add_prefix(&warn_icon);
                toast.add_toast(adw::Toast::new(&format!(
                    "Failed analyzing: {}",
                    row.title()
                )));
            }
        }
    }
}
