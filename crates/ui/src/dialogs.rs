//! Dialogs for FluxCut UI: Capabilities inspector, Preferences, and About.

use fluxcut_diagnostics::DiagnosticReport;
use fluxcut_hardware::GpuCapabilities;
use fluxcut_project::{AppConfig, CanvasRatio, ThemePreference};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

/// Show the System & GPU Capabilities dialog.
pub fn show_capabilities_dialog(
    parent: &impl IsA<gtk4::Window>,
    caps: &GpuCapabilities,
    diag: &DiagnosticReport,
) {
    let window = adw::PreferencesWindow::builder()
        .title("System & Hardware Capabilities")
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .default_height(480)
        .build();

    let page = adw::PreferencesPage::new();

    // Environment Group
    let env_group = adw::PreferencesGroup::builder()
        .title("Desktop & Display Environment")
        .description("Detected Linux display server and OS session details.")
        .build();

    let os_row = adw::ActionRow::builder()
        .title("Operating System")
        .subtitle(&diag.os_name)
        .build();
    os_row.add_prefix(&gtk4::Image::from_icon_name("drive-harddisk-symbolic"));
    env_group.add(&os_row);

    let wayland_status = if caps.wayland_available {
        "Active (Wayland native)"
    } else {
        "Inactive (X11 fallback session)"
    };
    let wayland_row = adw::ActionRow::builder()
        .title("Wayland Display")
        .subtitle(wayland_status)
        .build();
    let wayland_icon = if caps.wayland_available {
        "emblem-default-symbolic"
    } else {
        "dialog-warning-symbolic"
    };
    wayland_row.add_prefix(&gtk4::Image::from_icon_name(wayland_icon));
    env_group.add(&wayland_row);

    let session_row = adw::ActionRow::builder()
        .title("Session Type")
        .subtitle(diag.session_type.as_deref().unwrap_or("Unknown"))
        .build();
    session_row.add_prefix(&gtk4::Image::from_icon_name("preferences-system-symbolic"));
    env_group.add(&session_row);

    page.add(&env_group);

    // Hardware & Graphics Group
    let hw_group = adw::PreferencesGroup::builder()
        .title("Graphics & Video Acceleration")
        .description("Hardware decode, presentation, and GPU capabilities.")
        .build();

    let dri_sub = if caps.dri_available {
        format!("{} device nodes available", caps.dri_nodes.len())
    } else {
        "No DRI nodes detected in /dev/dri".to_string()
    };
    let dri_row = adw::ActionRow::builder()
        .title("Direct Rendering Infrastructure (DRI)")
        .subtitle(&dri_sub)
        .build();
    dri_row.add_prefix(&gtk4::Image::from_icon_name("video-display-symbolic"));
    hw_group.add(&dri_row);

    let vulkan_sub = if caps.vulkan_icd_available {
        format!("ICD manifests: {}", caps.vulkan_icd_files.join(", "))
    } else {
        "No Vulkan ICD manifests detected".to_string()
    };
    let vulkan_row = adw::ActionRow::builder()
        .title("Vulkan Support")
        .subtitle(&vulkan_sub)
        .build();
    vulkan_row.add_prefix(&gtk4::Image::from_icon_name(
        "applications-graphics-symbolic",
    ));
    hw_group.add(&vulkan_row);

    let vaapi_sub = if caps.vaapi_available {
        format!("Driver: {}", caps.vaapi_drivers.join(", "))
    } else {
        "No VA-API drivers found (Software fallback will be used)".to_string()
    };
    let vaapi_row = adw::ActionRow::builder()
        .title("VA-API Hardware Decode")
        .subtitle(&vaapi_sub)
        .build();
    vaapi_row.add_prefix(&gtk4::Image::from_icon_name(
        "media-playback-start-symbolic",
    ));
    hw_group.add(&vaapi_row);

    let dmabuf_sub = if caps.dmabuf_available {
        "Eligible for GdkDmabufTextureBuilder zero-copy direct scanout"
    } else {
        "Fallback to GdkMemoryTexture (CPU frame copy)"
    };
    let dmabuf_row = adw::ActionRow::builder()
        .title("DMA-BUF Zero-Copy Presentation")
        .subtitle(dmabuf_sub)
        .build();
    dmabuf_row.add_prefix(&gtk4::Image::from_icon_name("system-run-symbolic"));
    hw_group.add(&dmabuf_row);

    page.add(&hw_group);
    window.add(&page);
    window.present();
}

/// Show the About dialog.
pub fn show_about_dialog(parent: &impl IsA<gtk4::Widget>) {
    let dialog = adw::AboutDialog::builder()
        .application_name("FluxCut")
        .application_icon("video-x-generic-symbolic")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("FluxCut Contributors")
        .comments("A lightweight, native Linux desktop video editor built with GTK4, libadwaita, Rust, and FFmpeg.")
        .website("https://github.com/fluxcut/fluxcut")
        .issue_url("https://github.com/fluxcut/fluxcut/issues")
        .license_type(gtk4::License::Gpl30)
        .build();

    dialog.present(Some(parent));
}

/// Show the Preferences dialog.
pub fn show_preferences_dialog(parent: &impl IsA<gtk4::Window>, config: &AppConfig) {
    let window = adw::PreferencesWindow::builder()
        .title("Preferences")
        .transient_for(parent)
        .modal(true)
        .default_width(480)
        .default_height(400)
        .build();

    let page = adw::PreferencesPage::new();

    // Appearance Group
    let app_group = adw::PreferencesGroup::builder()
        .title("Appearance")
        .description("Configure application visual theme.")
        .build();

    let theme_row = adw::ComboRow::builder()
        .title("Theme Mode")
        .subtitle("Follow system setting or force Dark/Light mode")
        .model(&gtk4::StringList::new(&[
            "System Default",
            "Light Theme",
            "Dark Theme",
        ]))
        .selected(match config.theme {
            ThemePreference::System => 0,
            ThemePreference::Light => 1,
            ThemePreference::Dark => 2,
        })
        .build();
    theme_row.add_prefix(&gtk4::Image::from_icon_name("weather-clear-night-symbolic"));
    app_group.add(&theme_row);
    page.add(&app_group);

    // Project Defaults Group
    let proj_group = adw::PreferencesGroup::builder()
        .title("Project Defaults")
        .description("Default settings for newly created projects.")
        .build();

    let ratio_row = adw::ComboRow::builder()
        .title("Default Canvas Aspect Ratio")
        .model(&gtk4::StringList::new(&[
            "16:9 Landscape (1920x1080)",
            "9:16 Vertical (1080x1920)",
            "1:1 Square (1080x1080)",
            "4:3 Classic (1440x1080)",
        ]))
        .selected(match config.default_canvas {
            CanvasRatio::Landscape16x9 => 0,
            CanvasRatio::Vertical9x16 => 1,
            CanvasRatio::Square1x1 => 2,
            CanvasRatio::Classic4x3 => 3,
            CanvasRatio::Custom { .. } => 0,
        })
        .build();
    proj_group.add(&ratio_row);

    let hw_switch = adw::SwitchRow::builder()
        .title("Enable Hardware Acceleration")
        .subtitle("Use VA-API and DMA-BUF when supported by GPU and driver")
        .active(config.hardware_accel_enabled)
        .build();
    proj_group.add(&hw_switch);

    page.add(&proj_group);
    window.add(&page);
    window.present();
}
