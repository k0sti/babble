//! Orchestrator for end-to-end voice assistant pipeline
//!
//! Connects all components: Voice -> STT -> LLM -> TTS -> Playback

use crate::audio::preprocessor::preprocess_for_whisper;
use crate::integration::config::IntegrationConfig;
use crate::llm::pipeline::{LLMCommand, LLMEvent, LLMPipeline};
use crate::speech::tts::{AudioQueue, TTSCommand, TTSEvent, TTSPipeline};
use crate::Result;
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tracing::{debug, info, warn};

/// Commands that can be sent to the orchestrator
#[derive(Debug, Clone)]
pub enum OrchestratorCommand {
    /// Start recording voice input
    StartRecording,

    /// Stop recording and process the audio
    StopRecording,

    /// Cancel recording without processing
    CancelRecording,

    /// Send a text message directly to the LLM
    SendText(String),

    /// Clear conversation history
    ClearHistory,

    /// Shutdown the orchestrator
    Shutdown,
}

/// Events emitted by the orchestrator
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// Recording has started
    RecordingStarted,

    /// Recording has stopped, processing audio
    RecordingStopped,

    /// Recording was cancelled
    RecordingCancelled,

    /// Transcription result from STT
    Transcription(String),

    /// LLM started generating response
    GenerationStarted,

    /// Token received from LLM
    Token(String),

    /// LLM finished generating response
    GenerationComplete {
        response: String,
        first_token_ms: u64,
        total_ms: u64,
    },

    /// TTS audio is ready for playback
    AudioReady { duration_secs: f32 },

    /// Playback started
    PlaybackStarted,

    /// Playback completed
    PlaybackComplete,

    /// An error occurred
    Error(String),

    /// Orchestrator has shut down
    Shutdown,
}

/// Handle for controlling the orchestrator from the UI
pub struct OrchestratorHandle {
    /// Command sender
    command_tx: Sender<OrchestratorCommand>,

    /// Event receiver
    event_rx: Receiver<OrchestratorEvent>,

    /// LLM command sender (for direct LLM access)
    llm_command_tx: Sender<LLMCommand>,

    /// LLM event receiver (for direct LLM events)
    llm_event_rx: Receiver<LLMEvent>,

    /// TTS command sender
    tts_command_tx: Sender<TTSCommand>,

    /// TTS event receiver
    tts_event_rx: Receiver<TTSEvent>,

    /// Transcription result receiver
    transcription_rx: Receiver<String>,

    /// Audio sender for recording
    audio_tx: Sender<Vec<f32>>,

    /// Playback audio sender
    playback_tx: Sender<Vec<f32>>,

    /// Shared audio queue for TTS output
    audio_queue: AudioQueue,

    /// Whether recording is active
    is_recording: Arc<AtomicBool>,

    /// Recording buffer for waveform visualization
    recording_buffer: Arc<Mutex<Vec<f32>>>,
}

impl OrchestratorHandle {
    /// Send a command to the orchestrator
    pub fn send_command(&self, cmd: OrchestratorCommand) -> Result<()> {
        self.command_tx
            .send(cmd)
            .map_err(|e| crate::BabbleError::ConfigError(format!("Failed to send command: {}", e)))
    }

    /// Try to receive an event from the orchestrator
    pub fn try_recv_event(&self) -> Option<OrchestratorEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get the LLM command sender for direct access
    pub fn llm_command_sender(&self) -> Sender<LLMCommand> {
        self.llm_command_tx.clone()
    }

    /// Get the LLM event receiver for direct access
    pub fn llm_event_receiver(&self) -> Receiver<LLMEvent> {
        self.llm_event_rx.clone()
    }

    /// Get the TTS command sender
    pub fn tts_command_sender(&self) -> Sender<TTSCommand> {
        self.tts_command_tx.clone()
    }

    /// Get the TTS event receiver
    pub fn tts_event_receiver(&self) -> Receiver<TTSEvent> {
        self.tts_event_rx.clone()
    }

    /// Get the transcription receiver
    pub fn transcription_receiver(&self) -> Receiver<String> {
        self.transcription_rx.clone()
    }

    /// Get the audio sender for recording
    pub fn audio_sender(&self) -> Sender<Vec<f32>> {
        self.audio_tx.clone()
    }

    /// Get the playback audio sender
    pub fn playback_sender(&self) -> Sender<Vec<f32>> {
        self.playback_tx.clone()
    }

    /// Get the audio queue
    pub fn audio_queue(&self) -> &AudioQueue {
        &self.audio_queue
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Get the recording buffer for visualization
    pub fn recording_buffer(&self) -> Arc<Mutex<Vec<f32>>> {
        Arc::clone(&self.recording_buffer)
    }
}

/// Main orchestrator that coordinates all components
pub struct Orchestrator {
    /// Configuration
    config: IntegrationConfig,

    /// Command receiver
    command_rx: Receiver<OrchestratorCommand>,

    /// Event sender
    event_tx: Sender<OrchestratorEvent>,

    /// LLM pipeline
    llm_pipeline: Option<LLMPipeline>,

    /// TTS pipeline
    tts_pipeline: Option<TTSPipeline>,

    /// Recording state
    is_recording: Arc<AtomicBool>,

    /// Recording buffer
    recording_buffer: Arc<Mutex<Vec<f32>>>,

    /// Raw audio receiver
    audio_rx: Receiver<Vec<f32>>,

    /// Transcription result sender (to UI)
    transcription_tx: Sender<String>,

    /// Playback audio receiver
    playback_rx: Receiver<Vec<f32>>,
}

impl Orchestrator {
    /// Create a new orchestrator with the given configuration
    pub fn new(config: IntegrationConfig) -> Result<(Self, OrchestratorHandle)> {
        let (command_tx, command_rx) = bounded(100);
        let (event_tx, event_rx) = bounded(100);
        let (audio_tx, audio_rx) = bounded(1000);
        let (transcription_tx, transcription_rx) = bounded(100);
        let (playback_tx, playback_rx) = bounded(1000);

        let is_recording = Arc::new(AtomicBool::new(false));
        let recording_buffer = Arc::new(Mutex::new(Vec::with_capacity(16000 * 30))); // 30 seconds

        // Create LLM pipeline
        let llm_pipeline = LLMPipeline::new(config.llm.clone());
        let llm_command_tx = llm_pipeline.command_sender();
        let llm_event_rx = llm_pipeline.event_receiver();

        // Create TTS pipeline
        let tts_pipeline = TTSPipeline::new(config.tts.clone());
        let tts_command_tx = tts_pipeline.command_sender();
        let tts_event_rx = tts_pipeline.event_receiver();

        // Create audio queue
        let audio_queue = AudioQueue::new();

        let handle = OrchestratorHandle {
            command_tx,
            event_rx,
            llm_command_tx,
            llm_event_rx,
            tts_command_tx,
            tts_event_rx,
            transcription_rx,
            audio_tx,
            playback_tx,
            audio_queue,
            is_recording: Arc::clone(&is_recording),
            recording_buffer: Arc::clone(&recording_buffer),
        };

        let orchestrator = Self {
            config,
            command_rx,
            event_tx,
            llm_pipeline: Some(llm_pipeline),
            tts_pipeline: Some(tts_pipeline),
            is_recording,
            recording_buffer,
            audio_rx,
            transcription_tx,
            playback_rx,
        };

        Ok((orchestrator, handle))
    }

    /// Start the orchestrator and all pipelines
    ///
    /// This consumes the orchestrator and returns join handles for the worker threads.
    pub fn start(mut self) -> Result<Vec<JoinHandle<()>>> {
        let mut handles = Vec::new();

        // Start LLM pipeline
        if let Some(llm_pipeline) = self.llm_pipeline.take() {
            llm_pipeline.start_worker()?;
            info!("LLM pipeline started");
        }

        // Start TTS pipeline
        if let Some(tts_pipeline) = self.tts_pipeline.take() {
            tts_pipeline.start_worker()?;
            info!("TTS pipeline started");
        }

        // Start the main orchestrator loop
        let event_tx = self.event_tx.clone();
        let command_rx = self.command_rx.clone();
        let audio_rx = self.audio_rx.clone();
        let _transcription_tx = self.transcription_tx.clone();
        let is_recording = Arc::clone(&self.is_recording);
        let recording_buffer = Arc::clone(&self.recording_buffer);
        let input_sample_rate = self.config.input_sample_rate;

        let orchestrator_handle = thread::spawn(move || {
            info!("Orchestrator started");

            // Audio accumulation for STT
            let mut audio_accumulator: Vec<f32> = Vec::with_capacity(16000 * 30);

            loop {
                // Check for commands (non-blocking)
                match command_rx.try_recv() {
                    Ok(OrchestratorCommand::StartRecording) => {
                        is_recording.store(true, Ordering::SeqCst);
                        recording_buffer.lock().clear();
                        audio_accumulator.clear();
                        let _ = event_tx.send(OrchestratorEvent::RecordingStarted);
                        debug!("Recording started");
                    }
                    Ok(OrchestratorCommand::StopRecording) => {
                        is_recording.store(false, Ordering::SeqCst);
                        let _ = event_tx.send(OrchestratorEvent::RecordingStopped);
                        debug!("Recording stopped, accumulated {} samples", audio_accumulator.len());

                        // Process accumulated audio
                        if !audio_accumulator.is_empty() {
                            // Preprocess audio for STT (resample to 16kHz)
                            match preprocess_for_whisper(&audio_accumulator, input_sample_rate, false) {
                                Ok(processed) => {
                                    // TODO: Send to STT pipeline when available
                                    // For now, log that we would process
                                    info!(
                                        "Would process {} samples ({:.2}s) through STT",
                                        processed.len(),
                                        processed.len() as f32 / 16000.0
                                    );
                                }
                                Err(e) => {
                                    warn!("Failed to preprocess audio: {}", e);
                                }
                            }
                        }

                        audio_accumulator.clear();
                    }
                    Ok(OrchestratorCommand::CancelRecording) => {
                        is_recording.store(false, Ordering::SeqCst);
                        recording_buffer.lock().clear();
                        audio_accumulator.clear();
                        let _ = event_tx.send(OrchestratorEvent::RecordingCancelled);
                        debug!("Recording cancelled");
                    }
                    Ok(OrchestratorCommand::SendText(text)) => {
                        // Text is sent directly through the LLM handle
                        debug!("Text message received: {}", text);
                    }
                    Ok(OrchestratorCommand::ClearHistory) => {
                        // History is cleared through the LLM handle
                        debug!("Clear history requested");
                    }
                    Ok(OrchestratorCommand::Shutdown) => {
                        info!("Orchestrator shutdown requested");
                        let _ = event_tx.send(OrchestratorEvent::Shutdown);
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        warn!("Command channel disconnected");
                        break;
                    }
                }

                // Collect audio if recording
                if is_recording.load(Ordering::SeqCst) {
                    while let Ok(samples) = audio_rx.try_recv() {
                        // Store for visualization
                        {
                            let mut buffer = recording_buffer.lock();
                            buffer.extend_from_slice(&samples);
                            // Keep last 2 seconds for visualization
                            let max_samples = 16000 * 2;
                            let len = buffer.len();
                            if len > max_samples {
                                buffer.drain(0..len - max_samples);
                            }
                        }

                        // Accumulate for STT
                        audio_accumulator.extend_from_slice(&samples);
                    }
                }

                // Small sleep to avoid busy-waiting
                thread::sleep(std::time::Duration::from_millis(10));
            }

            info!("Orchestrator stopped");
        });

        handles.push(orchestrator_handle);

        Ok(handles)
    }
}

/// Builder for creating an orchestrator
pub struct OrchestratorBuilder {
    config: IntegrationConfig,
}

impl OrchestratorBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: IntegrationConfig::default(),
        }
    }

    /// Set the complete configuration
    pub fn with_config(mut self, config: IntegrationConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the Whisper model path
    pub fn with_whisper_model(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.config.stt.model_path = path.into();
        self
    }

    /// Set the TTS model paths
    pub fn with_tts_model(
        mut self,
        model_path: impl Into<String>,
        tokens_path: impl Into<String>,
    ) -> Self {
        self.config.tts.model_path = model_path.into();
        self.config.tts.tokens_path = tokens_path.into();
        self
    }

    /// Set the LLM model
    pub fn with_llm_model(mut self, model_id: impl Into<String>) -> Self {
        self.config.llm.model_id = model_id.into();
        self
    }

    /// Disable audio input
    pub fn without_audio_input(mut self) -> Self {
        self.config.enable_audio_input = false;
        self
    }

    /// Disable audio output
    pub fn without_audio_output(mut self) -> Self {
        self.config.enable_audio_output = false;
        self
    }

    /// Build the orchestrator
    pub fn build(self) -> Result<(Orchestrator, OrchestratorHandle)> {
        Orchestrator::new(self.config)
    }
}

impl Default for OrchestratorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let config = IntegrationConfig::default()
            .without_audio_input()
            .without_audio_output();

        let result = Orchestrator::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder() {
        let builder = OrchestratorBuilder::new()
            .with_llm_model("test-model")
            .without_audio_input();

        // Just verify it builds
        let _ = builder;
    }

    #[test]
    fn test_handle_methods() {
        let config = IntegrationConfig::default()
            .without_audio_input()
            .without_audio_output();

        let (_, handle) = Orchestrator::new(config).unwrap();

        // Test that all methods are accessible
        let _ = handle.llm_command_sender();
        let _ = handle.tts_command_sender();
        let _ = handle.audio_sender();
        assert!(!handle.is_recording());
    }
}
