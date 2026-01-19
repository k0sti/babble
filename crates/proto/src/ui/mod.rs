//! UI components and application module
//!
//! This module provides the egui/eframe-based user interface for Proto.

mod app;
pub mod components;
mod state;
mod theme;

pub use app::ProtoApp;
pub use components::{RecordButton, StateWaveform, Waveform};
pub use state::{AppState, RecordingState};
pub use theme::Theme;
