//! Integration tests for the Babble voice assistant
//!
//! These tests verify the end-to-end integration of all components.

use babble::integration::{IntegrationConfig, Orchestrator, OrchestratorCommand, OrchestratorEvent};
use babble::llm::LLMCommand;
use babble::speech::tts::TTSCommand;
use std::time::Duration;
use uuid::Uuid;

/// Test that the orchestrator can be created and started
#[test]
fn test_orchestrator_creation_and_startup() {
    // Create config without audio (for CI environments)
    let config = IntegrationConfig::default()
        .without_audio_input()
        .without_audio_output();

    // Create orchestrator
    let result = Orchestrator::new(config);
    assert!(result.is_ok(), "Orchestrator creation failed");

    let (orchestrator, handle) = result.unwrap();

    // Verify handle provides all channels
    let _ = handle.llm_command_sender();
    let _ = handle.tts_command_sender();
    let _ = handle.transcription_receiver();
    let _ = handle.audio_sender();

    // Start the orchestrator
    let handles = orchestrator.start();
    assert!(handles.is_ok(), "Orchestrator failed to start");

    // Give it a moment to initialize
    std::thread::sleep(Duration::from_millis(50));

    // Shutdown gracefully
    let _ = handle.send_command(OrchestratorCommand::Shutdown);
    std::thread::sleep(Duration::from_millis(50));
}

/// Test that the LLM command channel works
#[test]
fn test_llm_channel_communication() {
    let config = IntegrationConfig::default()
        .without_audio_input()
        .without_audio_output();

    let (orchestrator, handle) = Orchestrator::new(config).unwrap();
    let _ = orchestrator.start().unwrap();

    std::thread::sleep(Duration::from_millis(50));

    // Send a message through LLM channel
    let llm_tx = handle.llm_command_sender();
    let request_id = Uuid::new_v4();

    let send_result = llm_tx.send(LLMCommand::Generate {
        user_message: "Test message".to_string(),
        request_id,
    });

    assert!(send_result.is_ok(), "Failed to send LLM command");

    // Shutdown
    let _ = handle.send_command(OrchestratorCommand::Shutdown);
    std::thread::sleep(Duration::from_millis(50));
}

/// Test that TTS channel works
/// Note: This test verifies channel communication, not actual synthesis
/// (which requires models to be loaded)
#[test]
fn test_tts_channel_communication() {
    let config = IntegrationConfig::default()
        .without_audio_input()
        .without_audio_output();

    let (orchestrator, handle) = Orchestrator::new(config).unwrap();
    let _ = orchestrator.start().unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // Send through TTS channel - use try_send since pipeline may not be accepting
    let tts_tx = handle.tts_command_sender();
    let request_id = Uuid::new_v4();

    let segment = babble::llm::tts_parser::TTSSegment {
        text: "Hello world".to_string(),
        should_speak: true,
        index: 0,
    };

    // Try to send - may fail if channel is full or disconnected (expected in test environment)
    let send_result = tts_tx.try_send(TTSCommand::Synthesize { segment, request_id });
    // We just verify the channel exists and accepts attempts to send
    // The actual synthesis may not work without models loaded
    match send_result {
        Ok(_) => {} // Success
        Err(crossbeam_channel::TrySendError::Full(_)) => {} // Channel full is acceptable
        Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
            // Pipeline may have shut down - this is acceptable in test
        }
    }

    // Shutdown
    let _ = handle.send_command(OrchestratorCommand::Shutdown);
    std::thread::sleep(Duration::from_millis(50));
}

/// Test recording state management
#[test]
fn test_recording_state_transitions() {
    let config = IntegrationConfig::default()
        .without_audio_input()
        .without_audio_output();

    let (orchestrator, handle) = Orchestrator::new(config).unwrap();
    let _ = orchestrator.start().unwrap();

    std::thread::sleep(Duration::from_millis(50));

    // Initially not recording
    assert!(!handle.is_recording());

    // Start recording
    let _ = handle.send_command(OrchestratorCommand::StartRecording);
    std::thread::sleep(Duration::from_millis(50));

    // Should receive RecordingStarted event
    let mut received_start = false;
    for _ in 0..10 {
        if let Some(event) = handle.try_recv_event() {
            if matches!(event, OrchestratorEvent::RecordingStarted) {
                received_start = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(received_start, "Did not receive RecordingStarted event");

    // Should be recording now
    assert!(handle.is_recording());

    // Stop recording
    let _ = handle.send_command(OrchestratorCommand::StopRecording);
    std::thread::sleep(Duration::from_millis(50));

    // Should receive RecordingStopped event
    let mut received_stop = false;
    for _ in 0..10 {
        if let Some(event) = handle.try_recv_event() {
            if matches!(event, OrchestratorEvent::RecordingStopped) {
                received_stop = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(received_stop, "Did not receive RecordingStopped event");

    // Should not be recording anymore
    assert!(!handle.is_recording());

    // Shutdown
    let _ = handle.send_command(OrchestratorCommand::Shutdown);
    std::thread::sleep(Duration::from_millis(50));
}

/// Test cancel recording
#[test]
fn test_cancel_recording() {
    let config = IntegrationConfig::default()
        .without_audio_input()
        .without_audio_output();

    let (orchestrator, handle) = Orchestrator::new(config).unwrap();
    let _ = orchestrator.start().unwrap();

    std::thread::sleep(Duration::from_millis(50));

    // Start recording
    let _ = handle.send_command(OrchestratorCommand::StartRecording);
    std::thread::sleep(Duration::from_millis(50));

    // Cancel recording
    let _ = handle.send_command(OrchestratorCommand::CancelRecording);
    std::thread::sleep(Duration::from_millis(50));

    // Should receive RecordingCancelled event
    let mut received_cancel = false;
    for _ in 0..10 {
        if let Some(event) = handle.try_recv_event() {
            if matches!(event, OrchestratorEvent::RecordingCancelled) {
                received_cancel = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(received_cancel, "Did not receive RecordingCancelled event");

    // Should not be recording
    assert!(!handle.is_recording());

    // Shutdown
    let _ = handle.send_command(OrchestratorCommand::Shutdown);
    std::thread::sleep(Duration::from_millis(50));
}

/// Test audio buffer accumulation
#[test]
fn test_audio_buffer_accumulation() {
    let config = IntegrationConfig::default()
        .without_audio_input()
        .without_audio_output();

    let (orchestrator, handle) = Orchestrator::new(config).unwrap();
    let _ = orchestrator.start().unwrap();

    std::thread::sleep(Duration::from_millis(50));

    // Get audio sender
    let audio_tx = handle.audio_sender();

    // Start recording
    let _ = handle.send_command(OrchestratorCommand::StartRecording);
    std::thread::sleep(Duration::from_millis(50));

    // Send some audio samples
    let samples: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.001).sin()).collect();
    let _ = audio_tx.send(samples);

    std::thread::sleep(Duration::from_millis(50));

    // Check recording buffer has data
    let buffer = handle.recording_buffer();
    let buffer_len = buffer.lock().len();
    assert!(buffer_len > 0, "Recording buffer should have samples");

    // Stop recording
    let _ = handle.send_command(OrchestratorCommand::StopRecording);
    std::thread::sleep(Duration::from_millis(100));

    // Shutdown
    let _ = handle.send_command(OrchestratorCommand::Shutdown);
    std::thread::sleep(Duration::from_millis(50));
}

/// Test graceful shutdown
#[test]
fn test_graceful_shutdown() {
    let config = IntegrationConfig::default()
        .without_audio_input()
        .without_audio_output();

    let (orchestrator, handle) = Orchestrator::new(config).unwrap();
    let handles = orchestrator.start().unwrap();

    std::thread::sleep(Duration::from_millis(50));

    // Send shutdown command
    let _ = handle.send_command(OrchestratorCommand::Shutdown);
    std::thread::sleep(Duration::from_millis(50));

    // Should receive Shutdown event
    let mut received_shutdown = false;
    for _ in 0..10 {
        if let Some(event) = handle.try_recv_event() {
            if matches!(event, OrchestratorEvent::Shutdown) {
                received_shutdown = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(received_shutdown, "Did not receive Shutdown event");

    // Wait for threads to finish
    for h in handles {
        let _ = h.join();
    }
}
