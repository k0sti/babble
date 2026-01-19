//! UI components module
//!
//! This module provides reusable UI components for the Proto application.

pub mod record_button;
pub mod waveform;

pub use record_button::{RecordButton, StandaloneRecordButton};
pub use waveform::{StateWaveform, Waveform};
