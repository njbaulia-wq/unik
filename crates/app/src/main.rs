//! FluxCut Application Entry Point.
//!
//! Handles CLI argument dispatch, logging initialization,
//! hardware detection, and libadwaita application lifecycle.

use fluxcut_diagnostics::{init_logging, DiagnosticReport};
use fluxcut_hardware::GpuCapabilities;
use fluxcut_project::{AppConfig, Project, ThemePreference};
use fluxcut_ui::MainWindow;
use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;
use std::env;
use std::process;
use tracing::{error, info};

const APP_ID: &str = "org.fluxcut.FluxCut";

fn print_help() {
    println!(
        "FluxCut — Native Linux Desktop Video Editor (v{})\n\n\
         Usage:\n  fluxcut [OPTIONS] [FILE]\n\n\
         Options:\n\
           -d, --diagnostics    Print hardware & environment diagnostics report and exit\n\
           -v, --version        Print application version and exit\n\
           -h, --help           Display this help message\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Fast-path CLI handlers without GUI initialization
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        process::exit(0);
    }

    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("FluxCut {}", env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    // Initialize structured logging
    if let Err(e) = init_logging(None) {
        eprintln!("Warning: Failed to initialize logging: {}", e);
    }

    info!("Starting FluxCut v{}", env!("CARGO_PKG_VERSION"));

    // Collect system and hardware diagnostics
    let diag = DiagnosticReport::collect();
    let caps = GpuCapabilities::probe();

    if args.iter().any(|a| a == "-d" || a == "--diagnostics") {
        println!("{}", diag.format_markdown());
        println!("{}", caps.summary_markdown());
        process::exit(0);
    }

    // Load user preferences
    let config = AppConfig::load_or_default();

    // Initialize libadwaita application
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    let diag_clone = diag.clone();
    let caps_clone = caps.clone();
    let config_clone = config.clone();

    app.connect_startup(move |_| {
        // Clear deprecated GtkSettings property if present from environment/distro config
        if let Some(settings) = gtk4::Settings::default() {
            settings.reset_property("gtk-application-prefer-dark-theme");
        }

        // Apply libadwaita color scheme preference
        let style_manager = adw::StyleManager::default();
        match config_clone.theme {
            ThemePreference::Dark => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
            ThemePreference::Light => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
            ThemePreference::System => style_manager.set_color_scheme(adw::ColorScheme::Default),
        }
    });

    let main_window_slot: std::rc::Rc<std::cell::RefCell<Option<MainWindow>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot_clone = std::rc::Rc::clone(&main_window_slot);

    app.connect_activate(move |application| {
        let project = Project::new("Untitled Project");
        let window = MainWindow::new(
            application,
            project,
            config.clone(),
            caps_clone.clone(),
            diag_clone.clone(),
        );
        window.present();
        *slot_clone.borrow_mut() = Some(window);
    });

    let exit_code = app.run_with_args(&args);
    if exit_code != gtk4::glib::ExitCode::SUCCESS {
        error!(
            code = exit_code.get(),
            "Application exited with non-zero code"
        );
    }
}
