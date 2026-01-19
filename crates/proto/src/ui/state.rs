//! Application state management
//!
//! This module provides the central state for the Proto UI.

/// Recording state for voice input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingState {
    /// Not recording
    #[default]
    Idle,
    /// Currently recording audio
    Recording,
    /// Processing recorded audio (transcription)
    Processing,
}

/// Central application state for Proto
pub struct AppState {
    /// Recording state
    pub recording_state: RecordingState,

    /// Waveform data for visualization (recent audio samples)
    pub waveform_data: Vec<f32>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self {
            recording_state: RecordingState::Idle,
            waveform_data: Vec::with_capacity(1024),
        }
    }

    /// Start recording audio
    pub fn start_recording(&mut self) {
        self.recording_state = RecordingState::Recording;
        self.waveform_data.clear();
    }

    /// Stop recording and process audio
    pub fn stop_recording(&mut self) {
        if self.recording_state != RecordingState::Recording {
            return;
        }
        self.recording_state = RecordingState::Processing;
    }

    /// Cancel recording without processing
    pub fn cancel_recording(&mut self) {
        self.recording_state = RecordingState::Idle;
        self.waveform_data.clear();
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.recording_state == RecordingState::Recording
    }

    /// Check if currently processing
    pub fn is_processing(&self) -> bool {
        self.recording_state == RecordingState::Processing
    }
}
