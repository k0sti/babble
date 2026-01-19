//! Streaming speech-to-text processor with first-word detection
//!
//! This module provides concurrent STT processing that receives audio buffers
//! and produces transcribed text with streaming first-word detection for
//! command processing.

use crate::{ProtoError, Result};
use babble::audio::vad::VoiceActivityDetector;
use babble::speech::stt::{AudioSegment, TranscriptionResult, WhisperConfig, WhisperEngine};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info, warn};

/// Configuration for the STT processor
#[derive(Clone, Debug)]
pub struct STTConfig {
    /// Path to the Whisper model file
    pub model_path: PathBuf,

    /// Language to transcribe (None for auto-detection)
    pub language: Option<String>,

    /// Number of threads to use for transcription
    pub n_threads: i32,

    /// Minimum speech segment duration in seconds
    pub min_segment_duration: f32,

    /// Maximum speech segment duration in seconds
    pub max_segment_duration: f32,

    /// Silence duration threshold to trigger transcription (seconds)
    pub silence_threshold: f32,

    /// VAD probability threshold for speech detection (0.0-1.0)
    pub vad_threshold: f32,
}

impl Default for STTConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/ggml-base.en.bin"),
            language: Some("en".to_string()),
            n_threads: 4,
            min_segment_duration: 0.5,
            max_segment_duration: 30.0,
            silence_threshold: 0.5,
            vad_threshold: 0.5,
        }
    }
}

impl STTConfig {
    /// Convert to WhisperConfig for the underlying engine
    fn to_whisper_config(&self) -> WhisperConfig {
        WhisperConfig {
            model_path: self.model_path.clone(),
            language: self.language.clone(),
            n_threads: self.n_threads,
            translate: false,
            print_timestamps: false,
            min_segment_duration: self.min_segment_duration,
            max_segment_duration: self.max_segment_duration,
            silence_threshold: self.silence_threshold,
        }
    }
}

/// Events emitted by the STT processor
#[derive(Clone, Debug)]
pub enum STTEvent {
    /// First word detected from speech - useful for command detection
    FirstWord(String),

    /// Partial transcription (streaming update)
    Partial(String),

    /// Final transcription when speech segment ends
    Final(TranscriptionResult),

    /// Error occurred during processing
    Error(String),

    /// Worker has shut down
    Shutdown,
}

/// Commands that can be sent to the STT processor
#[derive(Debug)]
pub enum STTCommand {
    /// Process audio samples (mono, f32, 16kHz) through VAD
    ProcessAudio(Vec<f32>),

    /// Directly transcribe audio without VAD (for batch processing)
    TranscribeDirect(Vec<f32>),

    /// Flush any buffered audio and transcribe
    Flush,

    /// Shutdown the processor
    Shutdown,
}

/// Concurrent speech-to-text processor with streaming first-word detection
///
/// The processor runs transcription in a dedicated thread, receives audio
/// through a command channel, and emits transcription events including
/// first-word detection for command processing.
pub struct STTProcessor {
    #[allow(dead_code)]
    config: STTConfig,
    command_tx: Sender<STTCommand>,
    event_rx: Receiver<STTEvent>,
}

impl STTProcessor {
    /// Create a new STT processor
    ///
    /// This sets up the channels for communication but doesn't start the worker.
    /// Call `start_worker()` to begin processing.
    pub fn new(config: STTConfig) -> Result<(Self, STTWorker)> {
        let (command_tx, command_rx) = bounded(100);
        let (event_tx, event_rx) = bounded(100);

        let processor = Self {
            config: config.clone(),
            command_tx,
            event_rx,
        };

        let worker = STTWorker {
            config,
            command_rx,
            event_tx,
        };

        Ok((processor, worker))
    }

    /// Get a sender for commands
    pub fn command_sender(&self) -> Sender<STTCommand> {
        self.command_tx.clone()
    }

    /// Get a receiver for events
    pub fn event_receiver(&self) -> Receiver<STTEvent> {
        self.event_rx.clone()
    }

    /// Send audio for processing through VAD (streaming mode)
    pub fn send_audio(&self, audio: Vec<f32>) -> Result<()> {
        self.command_tx
            .send(STTCommand::ProcessAudio(audio))
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send audio: {}", e)))
    }

    /// Send audio for direct transcription without VAD (batch mode)
    pub fn transcribe_direct(&self, audio: Vec<f32>) -> Result<()> {
        self.command_tx
            .send(STTCommand::TranscribeDirect(audio))
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send audio: {}", e)))
    }

    /// Request to flush buffered audio
    pub fn flush(&self) -> Result<()> {
        self.command_tx
            .send(STTCommand::Flush)
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send flush: {}", e)))
    }

    /// Request shutdown
    pub fn shutdown(&self) -> Result<()> {
        self.command_tx
            .send(STTCommand::Shutdown)
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send shutdown: {}", e)))
    }

    /// Try to receive an event (non-blocking)
    pub fn try_recv_event(&self) -> Option<STTEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive an event (blocking)
    pub fn recv_event(&self) -> Result<STTEvent> {
        self.event_rx
            .recv()
            .map_err(|e| ProtoError::ChannelError(format!("Failed to receive event: {}", e)))
    }
}

/// Worker that runs the STT processing in a dedicated thread
pub struct STTWorker {
    config: STTConfig,
    command_rx: Receiver<STTCommand>,
    event_tx: Sender<STTEvent>,
}

impl STTWorker {
    /// Start the worker thread
    ///
    /// Returns a JoinHandle for the worker thread.
    pub fn start(self) -> Result<JoinHandle<()>> {
        let handle = thread::spawn(move || {
            if let Err(e) = self.run() {
                error!("STT worker error: {}", e);
            }
        });

        Ok(handle)
    }

    /// Main worker loop
    fn run(self) -> Result<()> {
        info!("STT worker starting");

        // Initialize the Whisper engine
        let whisper_config = self.config.to_whisper_config();
        let engine = match WhisperEngine::new(whisper_config) {
            Ok(e) => e,
            Err(e) => {
                error!("Failed to initialize Whisper engine: {}", e);
                let _ = self
                    .event_tx
                    .send(STTEvent::Error(format!("Model load failed: {}", e)));
                let _ = self.event_tx.send(STTEvent::Shutdown);
                return Err(ProtoError::STTError(format!(
                    "Failed to initialize Whisper: {}",
                    e
                )));
            }
        };

        // Initialize VAD
        let mut vad = match VoiceActivityDetector::new(16000, self.config.vad_threshold) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to initialize VAD: {}", e);
                let _ = self
                    .event_tx
                    .send(STTEvent::Error(format!("VAD init failed: {}", e)));
                let _ = self.event_tx.send(STTEvent::Shutdown);
                return Err(ProtoError::STTError(format!(
                    "Failed to initialize VAD: {}",
                    e
                )));
            }
        };

        info!("STT worker initialized successfully");

        // Processing state
        let mut state = ProcessingState::new(
            self.config.min_segment_duration,
            self.config.max_segment_duration,
            self.config.silence_threshold,
        );

        // Main processing loop
        loop {
            match self.command_rx.recv() {
                Ok(STTCommand::ProcessAudio(audio)) => {
                    if let Some(event) =
                        state.process_audio(&audio, &mut vad, &engine, &self.event_tx)
                    {
                        if let Err(e) = self.event_tx.send(event) {
                            error!("Failed to send event: {}", e);
                            break;
                        }
                    }
                }
                Ok(STTCommand::TranscribeDirect(audio)) => {
                    // Direct transcription without VAD - for batch processing
                    let duration = audio.len() as f32 / 16000.0;
                    info!(
                        "Direct transcription requested: {:.2}s of audio ({} samples)",
                        duration,
                        audio.len()
                    );

                    if duration < 0.1 {
                        warn!("Audio too short for transcription ({:.2}s)", duration);
                        continue;
                    }

                    // Create an audio segment and transcribe directly
                    let segment = AudioSegment::new(audio, true, 0.0);
                    match engine.transcribe(&segment) {
                        Ok(result) => {
                            info!("Direct transcription result: '{}'", result.text);
                            if !result.text.trim().is_empty() {
                                if let Err(e) = self.event_tx.send(STTEvent::Final(result)) {
                                    error!("Failed to send transcription result: {}", e);
                                    break;
                                }
                            } else {
                                debug!("Transcription was empty, not sending event");
                            }
                        }
                        Err(e) => {
                            error!("Direct transcription failed: {}", e);
                            let _ = self
                                .event_tx
                                .send(STTEvent::Error(format!("Transcription failed: {}", e)));
                        }
                    }
                }
                Ok(STTCommand::Flush) => {
                    if let Some(event) = state.flush(&engine, &self.event_tx) {
                        if let Err(e) = self.event_tx.send(event) {
                            error!("Failed to send event: {}", e);
                            break;
                        }
                    }
                }
                Ok(STTCommand::Shutdown) => {
                    info!("STT worker received shutdown command");
                    let _ = self.event_tx.send(STTEvent::Shutdown);
                    break;
                }
                Err(e) => {
                    error!("Command channel error: {}", e);
                    break;
                }
            }
        }

        info!("STT worker stopped");
        Ok(())
    }
}

/// Processing state for debugging and monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingPhase {
    /// Idle, waiting for speech
    Idle,
    /// Recording speech
    Recording,
    /// Accumulated silence, may trigger transcription
    SilenceDetected,
    /// Transcribing audio
    Transcribing,
    /// First word detection in progress
    DetectingFirstWord,
}

impl std::fmt::Display for ProcessingPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingPhase::Idle => write!(f, "IDLE"),
            ProcessingPhase::Recording => write!(f, "RECORDING"),
            ProcessingPhase::SilenceDetected => write!(f, "SILENCE_DETECTED"),
            ProcessingPhase::Transcribing => write!(f, "TRANSCRIBING"),
            ProcessingPhase::DetectingFirstWord => write!(f, "DETECTING_FIRST_WORD"),
        }
    }
}

/// Internal state for audio processing with VAD-based segmentation
struct ProcessingState {
    /// Accumulated audio buffer
    audio_buffer: Vec<f32>,

    /// Timestamp tracking
    buffer_start_time: f64,
    current_time: f64,

    /// Speech detection state
    is_in_speech: bool,
    silence_duration: f32,

    /// Whether we've already sent the first word for this segment
    first_word_sent: bool,

    /// Configuration
    min_segment_duration: f32,
    max_segment_duration: f32,
    silence_threshold: f32,

    /// Current processing phase for debugging
    phase: ProcessingPhase,

    /// Total audio chunks processed
    chunks_processed: u64,

    /// Total speech chunks detected
    speech_chunks: u64,
}

impl ProcessingState {
    fn new(min_segment_duration: f32, max_segment_duration: f32, silence_threshold: f32) -> Self {
        info!(
            "ProcessingState initialized: min_segment={:.2}s, max_segment={:.2}s, silence_threshold={:.2}s",
            min_segment_duration, max_segment_duration, silence_threshold
        );
        Self {
            audio_buffer: Vec::new(),
            buffer_start_time: 0.0,
            current_time: 0.0,
            is_in_speech: false,
            silence_duration: 0.0,
            first_word_sent: false,
            min_segment_duration,
            max_segment_duration,
            silence_threshold,
            phase: ProcessingPhase::Idle,
            chunks_processed: 0,
            speech_chunks: 0,
        }
    }

    fn set_phase(&mut self, new_phase: ProcessingPhase) {
        if self.phase != new_phase {
            debug!(
                "Phase transition: {} -> {} (chunks: {}, speech_chunks: {}, buffer: {:.2}s)",
                self.phase,
                new_phase,
                self.chunks_processed,
                self.speech_chunks,
                self.audio_buffer.len() as f32 / 16000.0
            );
            self.phase = new_phase;
        }
    }

    /// Process an audio chunk with VAD
    ///
    /// Returns an optional event to send (Final transcription result).
    /// First word and partial events are sent directly through the event_tx.
    fn process_audio(
        &mut self,
        audio: &[f32],
        vad: &mut VoiceActivityDetector,
        engine: &WhisperEngine,
        event_tx: &Sender<STTEvent>,
    ) -> Option<STTEvent> {
        self.chunks_processed += 1;
        let chunk_duration = audio.len() as f32 / 16000.0;
        self.current_time += chunk_duration as f64;

        // Log every 50 chunks (~1.6s at 32ms chunks) for monitoring
        if self.chunks_processed % 50 == 0 {
            info!(
                "STT Stats: phase={}, chunks={}, speech_chunks={}, buffer={:.2}s, time={:.2}s",
                self.phase,
                self.chunks_processed,
                self.speech_chunks,
                self.audio_buffer.len() as f32 / 16000.0,
                self.current_time
            );
        }

        // Run VAD on the audio chunk
        let is_speech = match vad.is_speech(audio) {
            Ok(speech) => speech,
            Err(e) => {
                warn!("VAD error at chunk {}: {}", self.chunks_processed, e);
                false
            }
        };

        if is_speech {
            self.speech_chunks += 1;

            if !self.is_in_speech {
                // Transition from silence to speech
                self.is_in_speech = true;
                self.buffer_start_time = self.current_time - chunk_duration as f64;
                self.audio_buffer.clear();
                self.first_word_sent = false;
                self.set_phase(ProcessingPhase::Recording);
                info!(
                    "Speech STARTED at {:.2}s (chunk {})",
                    self.buffer_start_time, self.chunks_processed
                );
            }

            // Accumulate audio
            self.audio_buffer.extend_from_slice(audio);
            self.silence_duration = 0.0;

            // Try to detect first word early for command detection
            if !self.first_word_sent {
                let segment_duration = self.audio_buffer.len() as f32 / 16000.0;
                // Try first word detection after ~500ms of speech
                if segment_duration >= 0.5 {
                    self.set_phase(ProcessingPhase::DetectingFirstWord);
                    debug!(
                        "Attempting first word detection at {:.2}s of speech",
                        segment_duration
                    );
                    if let Some(first_word) = self.try_detect_first_word(engine) {
                        self.first_word_sent = true;
                        info!("First word detected: '{}'", first_word);
                        let _ = event_tx.send(STTEvent::FirstWord(first_word));
                    }
                    self.set_phase(ProcessingPhase::Recording);
                }
            }

            // Check if segment is too long
            let segment_duration = self.audio_buffer.len() as f32 / 16000.0;
            if segment_duration >= self.max_segment_duration {
                info!(
                    "Max segment duration ({:.2}s) reached, triggering transcription",
                    self.max_segment_duration
                );
                self.set_phase(ProcessingPhase::Transcribing);
                let result = self.transcribe_buffer(engine);
                self.set_phase(ProcessingPhase::Idle);
                return result;
            }
        } else if self.is_in_speech {
            // In speech but current chunk is silence
            self.audio_buffer.extend_from_slice(audio);
            self.silence_duration += chunk_duration;
            self.set_phase(ProcessingPhase::SilenceDetected);

            debug!(
                "Silence during speech: {:.2}s / {:.2}s threshold",
                self.silence_duration, self.silence_threshold
            );

            // Check if we've had enough silence to end the segment
            if self.silence_duration >= self.silence_threshold {
                let segment_duration = self.audio_buffer.len() as f32 / 16000.0;

                if segment_duration >= self.min_segment_duration {
                    info!(
                        "Silence threshold ({:.2}s) reached after {:.2}s of speech, triggering transcription",
                        self.silence_threshold, segment_duration
                    );
                    self.set_phase(ProcessingPhase::Transcribing);
                    let result = self.transcribe_buffer(engine);
                    self.set_phase(ProcessingPhase::Idle);
                    return result;
                } else {
                    debug!(
                        "Segment too short ({:.2}s < {:.2}s min), discarding",
                        segment_duration, self.min_segment_duration
                    );
                    self.reset();
                    self.set_phase(ProcessingPhase::Idle);
                }
            }
        }

        None
    }

    /// Flush any buffered audio and transcribe
    fn flush(&mut self, engine: &WhisperEngine, _event_tx: &Sender<STTEvent>) -> Option<STTEvent> {
        let segment_duration = self.audio_buffer.len() as f32 / 16000.0;
        info!(
            "Flush requested: buffer={:.2}s, is_in_speech={}",
            segment_duration, self.is_in_speech
        );

        if !self.audio_buffer.is_empty() {
            if segment_duration >= self.min_segment_duration {
                self.set_phase(ProcessingPhase::Transcribing);
                let result = self.transcribe_buffer(engine);
                self.set_phase(ProcessingPhase::Idle);
                return result;
            } else {
                debug!(
                    "Flush: segment too short ({:.2}s < {:.2}s), discarding",
                    segment_duration, self.min_segment_duration
                );
            }
        }
        self.reset();
        self.set_phase(ProcessingPhase::Idle);
        None
    }

    /// Try to detect the first word from the current buffer
    fn try_detect_first_word(&self, engine: &WhisperEngine) -> Option<String> {
        if self.audio_buffer.is_empty() {
            return None;
        }

        let segment_duration = self.audio_buffer.len() as f32 / 16000.0;
        debug!(
            "Attempting first word detection on {:.2}s of audio",
            segment_duration
        );

        // Create a segment from current buffer
        let segment = AudioSegment::new(self.audio_buffer.clone(), true, self.buffer_start_time);

        // Transcribe
        match engine.transcribe(&segment) {
            Ok(result) => {
                debug!("First word detection transcription: '{}'", result.text);
                detect_first_word(&result.text)
            }
            Err(e) => {
                warn!("First word detection transcription error: {}", e);
                None
            }
        }
    }

    /// Transcribe the buffered audio
    fn transcribe_buffer(&mut self, engine: &WhisperEngine) -> Option<STTEvent> {
        if self.audio_buffer.is_empty() {
            debug!("Transcribe called with empty buffer, skipping");
            self.reset();
            return None;
        }

        let segment_duration = self.audio_buffer.len() as f32 / 16000.0;
        info!(
            "Starting transcription of {:.2}s audio segment ({} samples)",
            segment_duration,
            self.audio_buffer.len()
        );

        let segment = AudioSegment::new(
            std::mem::take(&mut self.audio_buffer),
            true,
            self.buffer_start_time,
        );

        let start_time = std::time::Instant::now();
        let result = match engine.transcribe(&segment) {
            Ok(r) => r,
            Err(e) => {
                error!(
                    "Transcription FAILED after {:.2}s: {}",
                    start_time.elapsed().as_secs_f32(),
                    e
                );
                self.reset();
                return Some(STTEvent::Error(e.to_string()));
            }
        };

        let elapsed = start_time.elapsed().as_secs_f32();
        info!(
            "Transcription COMPLETE in {:.2}s: '{}' (RTF: {:.2}x)",
            elapsed,
            result.text,
            elapsed / segment_duration
        );
        self.reset();
        Some(STTEvent::Final(result))
    }

    /// Reset state after transcription
    fn reset(&mut self) {
        debug!(
            "Resetting state (was: is_in_speech={}, buffer={:.2}s)",
            self.is_in_speech,
            self.audio_buffer.len() as f32 / 16000.0
        );
        self.audio_buffer.clear();
        self.is_in_speech = false;
        self.silence_duration = 0.0;
        self.first_word_sent = false;
    }
}

/// Extract the first word from transcribed text
///
/// This is used for early command detection to enable fast response
/// to voice commands.
fn detect_first_word(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Extract first word (split on whitespace)
    trimmed
        .split_whitespace()
        .next()
        .map(|w| w.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stt_config_default() {
        let config = STTConfig::default();
        assert_eq!(config.language, Some("en".to_string()));
        assert_eq!(config.n_threads, 4);
        assert_eq!(config.vad_threshold, 0.5);
    }

    #[test]
    fn test_detect_first_word() {
        assert_eq!(detect_first_word("Hello world"), Some("hello".to_string()));
        assert_eq!(detect_first_word("  Stop  "), Some("stop".to_string()));
        assert_eq!(detect_first_word(""), None);
        assert_eq!(detect_first_word("   "), None);
        assert_eq!(
            detect_first_word("LISTEN carefully"),
            Some("listen".to_string())
        );
    }

    #[test]
    fn test_detect_first_word_punctuation() {
        // Whisper may include punctuation
        assert_eq!(
            detect_first_word("Hello, how are you?"),
            Some("hello,".to_string())
        );
    }

    #[test]
    fn test_processing_state_new() {
        let state = ProcessingState::new(0.5, 30.0, 0.5);
        assert!(state.audio_buffer.is_empty());
        assert!(!state.is_in_speech);
        assert!(!state.first_word_sent);
    }

    #[test]
    fn test_stt_config_to_whisper() {
        let config = STTConfig {
            model_path: PathBuf::from("/test/model.bin"),
            language: Some("fr".to_string()),
            n_threads: 8,
            ..Default::default()
        };

        let whisper_config = config.to_whisper_config();
        assert_eq!(whisper_config.model_path, PathBuf::from("/test/model.bin"));
        assert_eq!(whisper_config.language, Some("fr".to_string()));
        assert_eq!(whisper_config.n_threads, 8);
    }
}
