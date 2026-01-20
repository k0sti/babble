//! Orchestrator for coordinating concurrent voice assistant processors
//!
//! This module provides the main orchestrator that coordinates all concurrent processes:
//! - Audio recording
//! - Speech-to-text (STT)
//! - Message handler with command detection
//! - LLM inference with streaming
//!
//! The orchestrator uses a shared `AppState` that can be queried by:
//! - UI for rendering
//! - TestRunner for assertions
//!
//! State changes are made by the orchestrator in response to:
//! - External commands (from UI or tests)
//! - Internal processor events (STT results, LLM tokens)

use crate::processor::{
    LLMCommand, LLMConfig, LLMEvent, LLMRunner, MessageCommand, MessageHandler,
    MessageHandlerCommand, MessageHandlerEvent, MessageHandlerWorker, STTCommand, STTConfig,
    STTEvent, STTProcessor, STTWorker,
};
use crate::state::{AppCommand, AppEvent, SharedAppState};
use crate::{ProtoError, Result};
use crossbeam_channel::{bounded, select, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Configuration for the orchestrator
#[derive(Clone, Debug)]
pub struct OrchestratorConfig {
    /// STT processor configuration
    pub stt: STTConfig,
    /// LLM runner configuration
    pub llm: LLMConfig,
    /// Channel buffer size
    pub channel_buffer_size: usize,
    /// Shutdown timeout in milliseconds
    pub shutdown_timeout_ms: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            stt: STTConfig::default(),
            llm: LLMConfig::default(),
            channel_buffer_size: 100,
            shutdown_timeout_ms: 5000,
        }
    }
}

impl OrchestratorConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the STT configuration
    pub fn with_stt(mut self, stt: STTConfig) -> Self {
        self.stt = stt;
        self
    }

    /// Set the LLM configuration
    pub fn with_llm(mut self, llm: LLMConfig) -> Self {
        self.llm = llm;
        self
    }

    /// Set the channel buffer size
    pub fn with_channel_buffer_size(mut self, size: usize) -> Self {
        self.channel_buffer_size = size;
        self
    }

    /// Set the shutdown timeout
    pub fn with_shutdown_timeout_ms(mut self, timeout: u64) -> Self {
        self.shutdown_timeout_ms = timeout;
        self
    }
}

/// Handle for controlling the orchestrator from the UI or tests
///
/// This provides the public interface for:
/// - Sending commands
/// - Receiving events (for UI updates)
/// - Querying state (via SharedAppState)
/// - Feeding audio data
pub struct OrchestratorHandle {
    /// Command sender for controlling the orchestrator
    command_tx: Sender<AppCommand>,
    /// Event receiver for UI notifications
    event_rx: Receiver<AppEvent>,
    /// Shared application state (for direct queries)
    state: SharedAppState,
    /// Audio sender for feeding audio data to STT
    audio_tx: Sender<Vec<f32>>,
}

impl OrchestratorHandle {
    /// Send a command to the orchestrator
    pub fn send_command(&self, cmd: AppCommand) -> Result<()> {
        self.command_tx
            .send(cmd)
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send command: {}", e)))
    }

    /// Start recording
    pub fn start_recording(&self) -> Result<()> {
        self.send_command(AppCommand::StartRecording)
    }

    /// Stop recording and process
    pub fn stop_recording(&self) -> Result<()> {
        self.send_command(AppCommand::StopRecording)
    }

    /// Cancel recording without processing
    pub fn cancel_recording(&self) -> Result<()> {
        self.send_command(AppCommand::CancelRecording)
    }

    /// Send text directly to LLM
    pub fn send_text(&self, text: String) -> Result<()> {
        self.send_command(AppCommand::SendText(text))
    }

    /// Stop current LLM generation
    pub fn stop_generation(&self) -> Result<()> {
        self.send_command(AppCommand::StopGeneration)
    }

    /// Clear conversation history
    pub fn clear_history(&self) -> Result<()> {
        self.send_command(AppCommand::ClearHistory)
    }

    /// Request shutdown
    pub fn shutdown(&self) -> Result<()> {
        self.send_command(AppCommand::Shutdown)
    }

    /// Try to receive an event (non-blocking)
    pub fn try_recv_event(&self) -> Option<AppEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive an event (blocking)
    pub fn recv_event(&self) -> Result<AppEvent> {
        self.event_rx
            .recv()
            .map_err(|e| ProtoError::ChannelError(format!("Failed to receive event: {}", e)))
    }

    /// Get the shared application state
    ///
    /// This can be used to query state directly without events.
    pub fn state(&self) -> &SharedAppState {
        &self.state
    }

    /// Get the audio sender for feeding audio data
    ///
    /// Audio should be mono f32 samples at 16kHz.
    pub fn audio_sender(&self) -> Sender<Vec<f32>> {
        self.audio_tx.clone()
    }

    // === Convenience state query methods ===

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.state.is_recording()
    }

    /// Check if processing (STT running)
    pub fn is_processing(&self) -> bool {
        self.state.is_processing()
    }

    /// Check if LLM is generating
    pub fn is_generating(&self) -> bool {
        self.state.is_generating()
    }

    /// Check if system is idle
    pub fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    /// Get last transcription text
    pub fn last_transcription(&self) -> Option<String> {
        self.state.last_transcription()
    }

    /// Get current response text
    pub fn current_response(&self) -> String {
        self.state.current_response()
    }
}

/// Main orchestrator that coordinates all concurrent processes
///
/// The orchestrator manages the lifecycle of:
/// - STT processor for speech-to-text
/// - Message handler for command detection
/// - LLM runner for response generation
///
/// It routes events between these components, updates shared state,
/// and emits events for UI notifications.
pub struct Orchestrator {
    config: OrchestratorConfig,

    // Shared state
    state: SharedAppState,

    // Channels for external communication
    command_rx: Receiver<AppCommand>,
    event_tx: Sender<AppEvent>,

    // Audio input channel
    audio_rx: Receiver<Vec<f32>>,

    // Sub-processor components (to be started)
    stt_processor: Option<STTProcessor>,
    stt_worker: Option<STTWorker>,
    handler: Option<MessageHandler>,
    handler_worker: Option<MessageHandlerWorker>,
    llm_runner: Option<LLMRunner>,
}

impl Orchestrator {
    /// Create a new orchestrator with the given configuration
    ///
    /// Returns the orchestrator and a handle for controlling it.
    /// The orchestrator must be started with `start()` to begin processing.
    pub fn new(config: OrchestratorConfig) -> Result<(Self, OrchestratorHandle)> {
        let buffer_size = config.channel_buffer_size;

        // Create shared state
        let state = SharedAppState::new();

        // Create external communication channels
        let (command_tx, command_rx) = bounded(buffer_size);
        let (event_tx, event_rx) = bounded(buffer_size);

        // Create audio input channel
        let (audio_tx, audio_rx) = bounded(buffer_size * 10); // Larger buffer for audio

        // Create STT processor
        let (stt_processor, stt_worker) = STTProcessor::new(config.stt.clone())?;

        // Create message handler
        let (handler, handler_worker) = MessageHandler::new();

        // Create LLM runner
        let llm_runner = LLMRunner::new(config.llm.clone());

        let handle = OrchestratorHandle {
            command_tx,
            event_rx,
            state: state.clone(),
            audio_tx,
        };

        let orchestrator = Self {
            config,
            state,
            command_rx,
            event_tx,
            audio_rx,
            stt_processor: Some(stt_processor),
            stt_worker: Some(stt_worker),
            handler: Some(handler),
            handler_worker: Some(handler_worker),
            llm_runner: Some(llm_runner),
        };

        Ok((orchestrator, handle))
    }

    /// Create an orchestrator with an existing shared state
    ///
    /// This is useful when you want to share state with other components
    /// (e.g., a test runner) that was created before the orchestrator.
    pub fn with_state(config: OrchestratorConfig, state: SharedAppState) -> Result<(Self, OrchestratorHandle)> {
        let buffer_size = config.channel_buffer_size;

        // Create external communication channels
        let (command_tx, command_rx) = bounded(buffer_size);
        let (event_tx, event_rx) = bounded(buffer_size);

        // Create audio input channel
        let (audio_tx, audio_rx) = bounded(buffer_size * 10);

        // Create STT processor
        let (stt_processor, stt_worker) = STTProcessor::new(config.stt.clone())?;

        // Create message handler
        let (handler, handler_worker) = MessageHandler::new();

        // Create LLM runner
        let llm_runner = LLMRunner::new(config.llm.clone());

        let handle = OrchestratorHandle {
            command_tx,
            event_rx,
            state: state.clone(),
            audio_tx,
        };

        let orchestrator = Self {
            config,
            state,
            command_rx,
            event_tx,
            audio_rx,
            stt_processor: Some(stt_processor),
            stt_worker: Some(stt_worker),
            handler: Some(handler),
            handler_worker: Some(handler_worker),
            llm_runner: Some(llm_runner),
        };

        Ok((orchestrator, handle))
    }

    /// Start the orchestrator and all sub-processors
    ///
    /// This consumes the orchestrator and returns join handles for all worker threads.
    /// The orchestrator runs in its own thread and coordinates all sub-processors.
    pub fn start(mut self) -> Result<Vec<JoinHandle<()>>> {
        let mut handles = Vec::new();

        // Start STT worker
        let stt_worker = self
            .stt_worker
            .take()
            .ok_or_else(|| ProtoError::STTError("STT worker already taken".into()))?;
        let stt_handle = stt_worker.start()?;
        handles.push(stt_handle);
        info!("STT worker started");

        // Start message handler worker
        let handler_worker = self
            .handler_worker
            .take()
            .ok_or_else(|| ProtoError::ChannelError("Handler worker already taken".into()))?;
        let handler_handle = handler_worker.start();
        handles.push(handler_handle);
        info!("Message handler worker started");

        // Start LLM worker
        let llm_runner = self
            .llm_runner
            .take()
            .ok_or_else(|| ProtoError::LLMError("LLM runner already taken".into()))?;
        let llm_handle = llm_runner.start_worker()?;
        info!("LLM worker started");

        // Get processor interfaces
        let stt_processor = self
            .stt_processor
            .take()
            .ok_or_else(|| ProtoError::STTError("STT processor already taken".into()))?;
        let handler = self
            .handler
            .take()
            .ok_or_else(|| ProtoError::ChannelError("Handler already taken".into()))?;

        // Start the main orchestrator loop
        let orchestrator_handle = self.run_orchestrator_loop(
            stt_processor,
            handler,
            llm_handle.command_tx,
            llm_handle.event_rx,
        );
        handles.push(orchestrator_handle);
        info!("Orchestrator loop started");

        Ok(handles)
    }

    /// Run the main orchestrator event loop
    fn run_orchestrator_loop(
        self,
        stt_processor: STTProcessor,
        handler: MessageHandler,
        llm_command_tx: Sender<LLMCommand>,
        llm_event_rx: Receiver<LLMEvent>,
    ) -> JoinHandle<()> {
        let state = self.state;
        let command_rx = self.command_rx;
        let event_tx = self.event_tx;
        let audio_rx = self.audio_rx;
        let shutdown_timeout = Duration::from_millis(self.config.shutdown_timeout_ms);

        // Get sub-processor channel interfaces
        let stt_command_tx = stt_processor.command_sender();
        let stt_event_rx = stt_processor.event_receiver();
        let handler_command_tx = handler.command_sender();
        let handler_event_rx = handler.event_receiver();

        thread::spawn(move || {
            info!("Orchestrator main loop starting");

            loop {
                select! {
                    // Handle external commands
                    recv(command_rx) -> cmd => {
                        match cmd {
                            Ok(AppCommand::StartRecording) => {
                                let can_start = state.read().recording.is_idle();
                                if can_start {
                                    state.write().start_recording();
                                    let _ = event_tx.send(AppEvent::StateChanged);
                                    debug!("Recording started");
                                } else {
                                    warn!("Cannot start recording: not in idle state");
                                }
                            }

                            Ok(AppCommand::StopRecording) => {
                                let can_stop = state.read().recording.is_recording();
                                if can_stop {
                                    state.write().stop_recording();
                                    let _ = event_tx.send(AppEvent::StateChanged);

                                    // Flush STT to process remaining audio
                                    if let Err(e) = stt_command_tx.send(STTCommand::Flush) {
                                        error!("Failed to send flush to STT: {}", e);
                                    }
                                    debug!("Recording stopped, flushing STT");
                                } else {
                                    warn!("Cannot stop recording: not in recording state");
                                }
                            }

                            Ok(AppCommand::CancelRecording) => {
                                let was_recording = state.read().recording.is_recording();
                                if was_recording {
                                    state.write().cancel_recording();
                                    let _ = event_tx.send(AppEvent::StateChanged);
                                    debug!("Recording cancelled");
                                }
                            }

                            Ok(AppCommand::SendText(text)) => {
                                debug!("Sending text directly to handler: {}", text);
                                if let Err(e) = handler_command_tx.send(MessageHandlerCommand::ProcessTranscription(text)) {
                                    error!("Failed to send text to handler: {}", e);
                                }
                            }

                            Ok(AppCommand::StopGeneration) => {
                                let is_generating = state.read().llm.is_generating();
                                if is_generating {
                                    if let Err(e) = llm_command_tx.send(LLMCommand::Stop) {
                                        error!("Failed to send stop to LLM: {}", e);
                                    }
                                    debug!("LLM stop requested");
                                }
                            }

                            Ok(AppCommand::ClearHistory) => {
                                // TODO: Implement conversation history clearing
                                debug!("Clear history requested");
                            }

                            Ok(AppCommand::Shutdown) => {
                                info!("Shutdown requested");

                                // Send shutdown to all sub-processors
                                let _ = stt_command_tx.send(STTCommand::Shutdown);
                                let _ = handler_command_tx.send(MessageHandlerCommand::Shutdown);
                                let _ = llm_command_tx.send(LLMCommand::Shutdown);

                                // Wait for shutdown events with timeout
                                let mut stt_shutdown = false;
                                let mut handler_shutdown = false;
                                let mut llm_shutdown = false;

                                let deadline = std::time::Instant::now() + shutdown_timeout;

                                while !(stt_shutdown && handler_shutdown && llm_shutdown) {
                                    if std::time::Instant::now() > deadline {
                                        warn!("Shutdown timeout reached, forcing exit");
                                        break;
                                    }

                                    if let Ok(event) = stt_event_rx.recv_timeout(Duration::from_millis(100)) {
                                        if matches!(event, STTEvent::Shutdown) {
                                            stt_shutdown = true;
                                            debug!("STT shutdown confirmed");
                                        }
                                    }

                                    if let Ok(event) = handler_event_rx.recv_timeout(Duration::from_millis(10)) {
                                        if matches!(event, MessageHandlerEvent::Shutdown) {
                                            handler_shutdown = true;
                                            debug!("Handler shutdown confirmed");
                                        }
                                    }

                                    if let Ok(event) = llm_event_rx.recv_timeout(Duration::from_millis(10)) {
                                        if matches!(event, LLMEvent::Shutdown) {
                                            llm_shutdown = true;
                                            debug!("LLM shutdown confirmed");
                                        }
                                    }
                                }

                                let _ = event_tx.send(AppEvent::Shutdown);
                                info!("Orchestrator shutdown complete");
                                return;
                            }

                            Err(_) => {
                                warn!("Command channel disconnected");
                                break;
                            }
                        }
                    }

                    // Handle incoming audio when recording
                    recv(audio_rx) -> audio => {
                        if let Ok(samples) = audio {
                            let is_recording = state.read().recording.is_recording();
                            if is_recording {
                                // Update audio buffer count
                                {
                                    let mut s = state.write();
                                    s.audio_buffer_samples += samples.len();
                                }

                                // Send audio to STT for processing
                                if let Err(e) = stt_command_tx.send(STTCommand::ProcessAudio(samples)) {
                                    error!("Failed to send audio to STT: {}", e);
                                }
                            }
                        }
                    }

                    // Handle STT events
                    recv(stt_event_rx) -> event => {
                        match event {
                            Ok(STTEvent::FirstWord(word)) => {
                                debug!("STT first word: {}", word);
                                {
                                    let mut s = state.write();
                                    s.transcription.set_first_word(word.clone());
                                }
                                let _ = event_tx.send(AppEvent::StateChanged);

                                // Send to handler for command detection
                                if let Err(e) = handler_command_tx.send(MessageHandlerCommand::CheckFirstWord(word)) {
                                    error!("Failed to send first word to handler: {}", e);
                                }
                            }

                            Ok(STTEvent::Final(result)) => {
                                debug!("STT final transcription: {}", result.text);
                                {
                                    let mut s = state.write();
                                    s.transcription.set_transcription(result.text.clone());
                                    s.finish_processing();
                                    s.audio_buffer_samples = 0; // Reset buffer count
                                }
                                let _ = event_tx.send(AppEvent::StateChanged);

                                // Send to handler for processing
                                if let Err(e) = handler_command_tx.send(MessageHandlerCommand::ProcessTranscription(result.text)) {
                                    error!("Failed to send transcription to handler: {}", e);
                                }
                            }

                            Ok(STTEvent::Partial(text)) => {
                                debug!("STT partial: {}", text);
                            }

                            Ok(STTEvent::Error(err)) => {
                                error!("STT error: {}", err);
                                {
                                    let mut s = state.write();
                                    s.set_error(format!("STT error: {}", err));
                                    s.finish_processing();
                                    s.audio_buffer_samples = 0;
                                }
                                let _ = event_tx.send(AppEvent::Error(format!("STT error: {}", err)));
                            }

                            Ok(STTEvent::Shutdown) => {
                                debug!("STT shutdown event received");
                            }

                            Err(_) => {
                                warn!("STT event channel disconnected");
                            }
                        }
                    }

                    // Handle message handler events
                    recv(handler_event_rx) -> event => {
                        match event {
                            Ok(MessageHandlerEvent::CommandDetected(cmd)) => {
                                match cmd {
                                    MessageCommand::Stop => {
                                        info!("Stop command detected");
                                        let is_generating = state.read().llm.is_generating();
                                        if is_generating {
                                            if let Err(e) = llm_command_tx.send(LLMCommand::Stop) {
                                                error!("Failed to send stop to LLM: {}", e);
                                            }
                                        }
                                    }
                                    MessageCommand::Continue => {
                                        // No action needed
                                    }
                                }
                            }

                            Ok(MessageHandlerEvent::TextReady(text)) => {
                                debug!("Text ready for LLM: {}", text);
                                {
                                    state.write().start_generation();
                                }
                                let _ = event_tx.send(AppEvent::StateChanged);

                                // Send to LLM for generation
                                if let Err(e) = llm_command_tx.send(LLMCommand::Generate(text)) {
                                    error!("Failed to send text to LLM: {}", e);
                                }
                            }

                            Ok(MessageHandlerEvent::Shutdown) => {
                                debug!("Handler shutdown event received");
                            }

                            Err(_) => {
                                warn!("Handler event channel disconnected");
                            }
                        }
                    }

                    // Handle LLM events
                    recv(llm_event_rx) -> event => {
                        match event {
                            Ok(LLMEvent::Started) => {
                                debug!("LLM generation started");
                            }

                            Ok(LLMEvent::Token(token)) => {
                                {
                                    state.write().response.append_token(&token);
                                }
                                let _ = event_tx.send(AppEvent::LLMToken(token));
                            }

                            Ok(LLMEvent::Complete { response: _, interrupted }) => {
                                {
                                    state.write().finish_generation(interrupted);
                                }
                                let _ = event_tx.send(AppEvent::StateChanged);
                                debug!("LLM generation complete (interrupted: {})", interrupted);
                            }

                            Ok(LLMEvent::Error(err)) => {
                                error!("LLM error: {}", err);
                                {
                                    let mut s = state.write();
                                    s.set_error(format!("LLM error: {}", err));
                                    s.finish_generation(true);
                                }
                                let _ = event_tx.send(AppEvent::Error(format!("LLM error: {}", err)));
                            }

                            Ok(LLMEvent::Shutdown) => {
                                debug!("LLM shutdown event received");
                            }

                            Err(_) => {
                                warn!("LLM event channel disconnected");
                            }
                        }
                    }

                    // Default timeout to prevent busy-waiting
                    default(Duration::from_millis(10)) => {
                        // No events, continue loop
                    }
                }
            }

            info!("Orchestrator main loop exiting");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.channel_buffer_size, 100);
        assert_eq!(config.shutdown_timeout_ms, 5000);
    }

    #[test]
    fn test_orchestrator_config_builder() {
        let config = OrchestratorConfig::new()
            .with_channel_buffer_size(200)
            .with_shutdown_timeout_ms(10000);

        assert_eq!(config.channel_buffer_size, 200);
        assert_eq!(config.shutdown_timeout_ms, 10000);
    }

    #[test]
    fn test_shared_state_is_accessible() {
        // This test verifies the design - state can be shared
        let state = SharedAppState::new();

        // Simulate orchestrator writing
        {
            state.write().start_recording();
        }

        // Simulate UI/test reading
        assert!(state.is_recording());

        // Simulate orchestrator finishing
        {
            state.write().stop_recording();
            state.write().finish_processing();
        }

        assert!(state.is_idle());
    }
}
