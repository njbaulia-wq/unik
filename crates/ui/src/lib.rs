//! FluxCut UI Subsystem.
//!
//! Provides libadwaita/GTK4 desktop views, asynchronous dialogs,
//! capabilities inspector, and project window management.

pub mod dialogs;
pub mod window;

pub use dialogs::{show_about_dialog, show_capabilities_dialog, show_preferences_dialog};
pub use window::MainWindow;
