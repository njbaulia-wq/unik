//! FluxCut Presentation Bridge and Viewport Rendering.
//!
//! Connects background decoded frames to GTK4 textures and Wayland compositors.

pub mod surface;

pub use surface::VideoPresentationBridge;
