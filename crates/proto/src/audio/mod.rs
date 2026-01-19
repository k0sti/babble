//! Audio input and recording module
//!
//! This module handles audio capture from the microphone and manages
//! the audio input stream for real-time speech processing.

mod buffer;
mod input;

pub use buffer::AudioRingBuffer;
pub use input::{list_input_devices, AudioDeviceInfo, AudioRecorder};

/// Recording state for the audio input system
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RecordingState {
    /// No recording in progress
    #[default]
    Idle,
    /// Actively recording audio
    Recording,
    /// Recording stopped, processing audio data
    Processing,
}

impl RecordingState {
    /// Check if currently in a recording state
    pub fn is_recording(&self) -> bool {
        matches!(self, RecordingState::Recording)
    }

    /// Check if in an active state (recording or processing)
    pub fn is_active(&self) -> bool {
        !matches!(self, RecordingState::Idle)
    }
}

impl std::fmt::Display for RecordingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordingState::Idle => write!(f, "Idle"),
            RecordingState::Recording => write!(f, "Recording"),
            RecordingState::Processing => write!(f, "Processing"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_state_default() {
        let state = RecordingState::default();
        assert_eq!(state, RecordingState::Idle);
    }

    #[test]
    fn test_is_recording() {
        assert!(!RecordingState::Idle.is_recording());
        assert!(RecordingState::Recording.is_recording());
        assert!(!RecordingState::Processing.is_recording());
    }

    #[test]
    fn test_is_active() {
        assert!(!RecordingState::Idle.is_active());
        assert!(RecordingState::Recording.is_active());
        assert!(RecordingState::Processing.is_active());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", RecordingState::Idle), "Idle");
        assert_eq!(format!("{}", RecordingState::Recording), "Recording");
        assert_eq!(format!("{}", RecordingState::Processing), "Processing");
    }
}
