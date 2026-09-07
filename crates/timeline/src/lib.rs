//! FluxCut Timeline Engine.
//!
//! Provides a high-performance custom GTK4 timeline widget rendered via GSK
//! scene graphs with viewport virtualization, playhead scrubbing, clip
//! selection, non-destructive trimming, split-at-playhead, and snapping.

pub mod coords;
pub mod interaction;
pub mod renderer;
pub mod widget;

pub use coords::{TimelineCoords, DEFAULT_PX_PER_SEC, MAX_PX_PER_SEC, MIN_PX_PER_SEC};
pub use interaction::{DragState, HitTarget, TimelineInteractionState};
pub use renderer::{RenderParams, TimelineRenderer};
pub use widget::TimelineWidget;
