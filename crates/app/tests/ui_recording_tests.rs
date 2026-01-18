//! UI recording state tests
//!
//! These tests verify the recording state machine and transitions.

use babble::ui::{AppState, RecordingState};
use std::time::Duration;

#[test]
fn test_initial_state_is_idle() {
    let state = AppState::new();
    assert_eq!(
        state.recording_state,
        RecordingState::Idle,
        "Initial state should be Idle"
    );
}

#[test]
fn test_start_recording_transitions_to_recording() {
    let mut state = AppState::new();

    state.start_recording();

    assert_eq!(
        state.recording_state,
        RecordingState::Recording,
        "State should be Recording after start_recording()"
    );
}

#[test]
fn test_stop_recording_transitions_to_processing() {
    let mut state = AppState::new();

    // Must be recording first
    state.start_recording();
    state.stop_recording();

    assert_eq!(
        state.recording_state,
        RecordingState::Processing,
        "State should be Processing after stop_recording()"
    );
}

#[test]
fn test_cancel_recording_transitions_to_idle() {
    let mut state = AppState::new();

    state.start_recording();
    state.cancel_recording();

    assert_eq!(
        state.recording_state,
        RecordingState::Idle,
        "State should be Idle after cancel_recording()"
    );
}

#[test]
fn test_stop_recording_only_works_when_recording() {
    let mut state = AppState::new();

    // When Idle, stop_recording should do nothing
    state.stop_recording();
    assert_eq!(
        state.recording_state,
        RecordingState::Idle,
        "stop_recording when Idle should keep Idle state"
    );

    // When Processing, stop_recording should do nothing
    state.start_recording();
    state.stop_recording(); // Now Processing
    state.stop_recording(); // Should do nothing
    assert_eq!(
        state.recording_state,
        RecordingState::Processing,
        "stop_recording when Processing should keep Processing state"
    );
}

#[test]
fn test_waveform_data_cleared_on_start() {
    let mut state = AppState::new();

    // Add some fake waveform data
    state.waveform_data = vec![0.1, 0.2, 0.3, 0.4, 0.5];

    state.start_recording();

    assert!(
        state.waveform_data.is_empty(),
        "Waveform data should be cleared when recording starts"
    );
}

#[test]
fn test_waveform_data_cleared_on_cancel() {
    let mut state = AppState::new();

    state.start_recording();
    // Simulate some waveform data during recording
    state.waveform_data = vec![0.1, 0.2, 0.3];

    state.cancel_recording();

    assert!(
        state.waveform_data.is_empty(),
        "Waveform data should be cleared when recording is cancelled"
    );
}

#[test]
fn test_recording_buffer_cleared_on_start() {
    let mut state = AppState::new();

    // Add some data to the recording buffer
    {
        let mut buffer = state.recording_buffer.lock();
        buffer.extend_from_slice(&[0.1, 0.2, 0.3, 0.4, 0.5]);
    }

    state.start_recording();

    let buffer_len = state.recording_buffer.lock().len();
    assert_eq!(
        buffer_len, 0,
        "Recording buffer should be cleared when recording starts"
    );
}

#[test]
fn test_recording_buffer_cleared_on_cancel() {
    let mut state = AppState::new();

    state.start_recording();

    // Add some data to the recording buffer
    {
        let mut buffer = state.recording_buffer.lock();
        buffer.extend_from_slice(&[0.1, 0.2, 0.3]);
    }

    state.cancel_recording();

    let buffer_len = state.recording_buffer.lock().len();
    assert_eq!(
        buffer_len, 0,
        "Recording buffer should be cleared when recording is cancelled"
    );
}

#[test]
fn test_processing_timeout() {
    let mut state = AppState::new();

    // Start and stop recording to enter Processing state
    state.start_recording();
    state.stop_recording();

    assert_eq!(state.recording_state, RecordingState::Processing);

    // Wait for timeout (2 seconds + margin)
    std::thread::sleep(Duration::from_millis(2500));

    // Poll events to trigger the timeout check
    state.poll_events();

    assert_eq!(
        state.recording_state,
        RecordingState::Idle,
        "Should return to Idle after processing timeout"
    );
}

#[test]
fn test_update_waveform() {
    let mut state = AppState::new();

    // Initially empty
    assert!(state.waveform_data.is_empty());

    // Add samples
    let samples: Vec<f32> = (0..500).map(|i| (i as f32 * 0.01).sin()).collect();
    state.update_waveform(&samples);

    assert!(!state.waveform_data.is_empty());
    assert!(state.waveform_data.len() <= 1024, "Waveform should be limited to 1024 samples");
}

#[test]
fn test_update_waveform_downsamples_large_input() {
    let mut state = AppState::new();

    // Add many samples (more than MAX_SAMPLES = 1024)
    let samples: Vec<f32> = (0..5000).map(|i| (i as f32 * 0.001).sin()).collect();
    state.update_waveform(&samples);

    assert!(
        state.waveform_data.len() <= 1024,
        "Waveform should be downsampled to max 1024 samples, got {}",
        state.waveform_data.len()
    );
}

#[test]
fn test_state_machine_full_cycle() {
    let mut state = AppState::new();

    // Idle -> Recording
    assert_eq!(state.recording_state, RecordingState::Idle);
    state.start_recording();
    assert_eq!(state.recording_state, RecordingState::Recording);

    // Recording -> Processing
    state.stop_recording();
    assert_eq!(state.recording_state, RecordingState::Processing);

    // Processing -> Idle (via timeout simulation)
    state.recording_state = RecordingState::Idle; // Simulate transcription completed

    // Idle -> Recording -> Idle (via cancel)
    state.start_recording();
    assert_eq!(state.recording_state, RecordingState::Recording);
    state.cancel_recording();
    assert_eq!(state.recording_state, RecordingState::Idle);
}

#[test]
fn test_debug_info_logs_on_state_changes() {
    let mut state = AppState::new();

    let initial_log_count = state.debug_info.log_messages.len();

    state.start_recording();
    assert!(
        state.debug_info.log_messages.len() > initial_log_count,
        "start_recording should add a log message"
    );

    let after_start_count = state.debug_info.log_messages.len();

    state.stop_recording();
    assert!(
        state.debug_info.log_messages.len() > after_start_count,
        "stop_recording should add a log message"
    );
}

#[test]
fn test_processing_start_time_set_on_stop() {
    let mut state = AppState::new();

    state.start_recording();
    state.stop_recording();

    // Check that processing_start_time is set (we can't access private field directly,
    // but we can verify the timeout mechanism works)
    assert_eq!(state.recording_state, RecordingState::Processing);

    // If we poll immediately, it should still be Processing (no timeout yet)
    state.poll_events();
    assert_eq!(
        state.recording_state,
        RecordingState::Processing,
        "Should still be Processing immediately after stop"
    );
}
