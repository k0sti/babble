//! UI components module
//!
//! This module provides reusable UI components for the Proto application.

pub mod debug_panel;
pub mod record_button;
pub mod response_display;
pub mod waveform;

pub use debug_panel::{CollapsibleDebugPanel, DebugPanel, DebugPanelStandalone};
pub use record_button::{RecordButton, StandaloneRecordButton};
pub use response_display::{ResponseDisplay, ResponseDisplayStandalone};
pub use waveform::{StateWaveform, Waveform};
