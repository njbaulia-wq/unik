//! FluxCut Playback and Frame Decoding Subsystem.
//!
//! Provides the background decoding worker, play/pause state machine,
//! frame pacing, and coalesced seeking engine.

pub mod frame;
pub mod worker;

pub use frame::DecodedVideoFrame;
pub use worker::{AudioSink, PlaybackCommand, PlaybackController, PlaybackEvent, PlaybackState};
