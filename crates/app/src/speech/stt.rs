use crate::audio::vad::VoiceActivityDetector;
use crate::{BabbleError, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Configuration for the Whisper speech-to-text engine
#[derive(Clone, Debug)]
pub struct WhisperConfig {
    /// Path to the Whisper model file
    pub model_path: PathBuf,

    /// Language to transcribe (None for auto-detection)
    pub language: Option<String>,

    /// Number of threads to use for transcription
    pub n_threads: i32,

    /// Enable translation to English
    pub translate: bool,

    /// Print timestamps for each segment
    pub print_timestamps: bool,

    /// Minimum speech segment duration in seconds
    pub min_segment_duration: f32,

    /// Maximum speech segment duration in seconds
    pub max_segment_duration: f32,

    /// Silence duration threshold to trigger transcription (seconds)
    pub silence_threshold: f32,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/ggml-base.en.bin"),
            language: Some("en".to_string()),
            n_threads: 4,
            translate: false,
            print_timestamps: false,
            min_segment_duration: 0.5,
            max_segment_duration: 30.0,
            silence_threshold: 0.5,
        }
    }
}

/// Audio segment with metadata for transcription
#[derive(Clone, Debug)]
pub struct AudioSegment {
    /// Audio samples (mono, f32, 16kHz)
    pub samples: Vec<f32>,

    /// Whether this segment contains speech
    pub is_speech: bool,

    /// Timestamp when the segment started
    pub start_time: f64,

    /// Duration of the segment in seconds
    pub duration: f64,
}

impl AudioSegment {
    pub fn new(samples: Vec<f32>, is_speech: bool, start_time: f64) -> Self {
        let duration = samples.len() as f64 / 16000.0;
        Self {
            samples,
            is_speech,
            start_time,
            duration,
        }
    }
}

/// Result of transcription
#[derive(Clone, Debug)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,

    /// Start timestamp
    pub start_time: f64,

    /// End timestamp
    pub end_time: f64,

    /// Confidence score (if available)
    pub confidence: Option<f32>,
}

/// Commands that can be sent to the transcription worker
#[derive(Debug)]
pub enum TranscriptionCommand {
    /// Transcribe an audio segment
    Transcribe(AudioSegment),

    /// Shutdown the transcription worker
    Shutdown,
}

/// Events sent from the transcription worker
#[derive(Clone, Debug)]
pub enum TranscriptionEvent {
    /// Transcription result
    Result(TranscriptionResult),

    /// Error occurred during transcription
    Error(String),

    /// Worker has shut down
    Shutdown,
}

/// Whisper speech-to-text engine
pub struct WhisperEngine {
    config: WhisperConfig,
    context: WhisperContext,
}

impl WhisperEngine {
    /// Create a new Whisper engine
    pub fn new(config: WhisperConfig) -> Result<Self> {
        info!("Loading Whisper model from: {:?}", config.model_path);

        if !config.model_path.exists() {
            return Err(BabbleError::ModelLoadError(format!(
                "Model file not found: {:?}",
                config.model_path
            )));
        }

        let ctx = WhisperContext::new_with_params(
            config
                .model_path
                .to_str()
                .ok_or_else(|| BabbleError::ModelLoadError("Invalid model path".to_string()))?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| {
            BabbleError::ModelLoadError(format!("Failed to load Whisper model: {:?}", e))
        })?;

        info!("Whisper model loaded successfully");

        Ok(Self { config, context: ctx })
    }

    /// Transcribe an audio segment
    pub fn transcribe(&self, segment: &AudioSegment) -> Result<TranscriptionResult> {
        if segment.samples.is_empty() {
            return Err(BabbleError::TranscriptionError(
                "Empty audio segment".to_string(),
            ));
        }

        debug!(
            "Transcribing audio segment: {} samples, {:.2}s duration",
            segment.samples.len(),
            segment.duration
        );

        // Create transcription parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Configure parameters
        params.set_n_threads(self.config.n_threads);
        params.set_translate(self.config.translate);
        params.set_print_timestamps(self.config.print_timestamps);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);

        if let Some(ref lang) = self.config.language {
            params.set_language(Some(lang));
        }

        // Create a state for this transcription
        let mut state = self.context.create_state().map_err(|e| {
            BabbleError::TranscriptionError(format!("Failed to create state: {:?}", e))
        })?;

        // Run the transcription
        state.full(params, &segment.samples).map_err(|e| {
            BabbleError::TranscriptionError(format!("Transcription failed: {:?}", e))
        })?;

        // Collect the results
        let num_segments = state.full_n_segments().map_err(|e| {
            BabbleError::TranscriptionError(format!("Failed to get segments: {:?}", e))
        })?;

        let mut text = String::new();
        let mut start_time = f64::MAX;
        let mut end_time = f64::MIN;

        for i in 0..num_segments {
            let segment_text = state.full_get_segment_text(i).map_err(|e| {
                BabbleError::TranscriptionError(format!("Failed to get segment text: {:?}", e))
            })?;

            let t0 = state.full_get_segment_t0(i).map_err(|e| {
                BabbleError::TranscriptionError(format!("Failed to get start time: {:?}", e))
            })?;

            let t1 = state.full_get_segment_t1(i).map_err(|e| {
                BabbleError::TranscriptionError(format!("Failed to get end time: {:?}", e))
            })?;

            // Convert timestamps from centiseconds to seconds
            let t0_sec = t0 as f64 / 100.0;
            let t1_sec = t1 as f64 / 100.0;

            start_time = start_time.min(t0_sec);
            end_time = end_time.max(t1_sec);

            text.push_str(&segment_text);
        }

        // Adjust timestamps relative to the original segment
        let adjusted_start = segment.start_time + start_time;
        let adjusted_end = segment.start_time + end_time;

        debug!("Transcription result: '{}'", text.trim());

        Ok(TranscriptionResult {
            text: text.trim().to_string(),
            start_time: adjusted_start,
            end_time: adjusted_end,
            confidence: None,
        })
    }
}

/// Real-time transcription pipeline with VAD-based segmentation
pub struct TranscriptionPipeline {
    config: WhisperConfig,
    vad: VoiceActivityDetector,

    // Audio accumulation buffer
    audio_buffer: Vec<f32>,

    // Timestamp tracking
    buffer_start_time: f64,
    current_time: f64,

    // Speech detection state
    is_in_speech: bool,
    silence_duration: f32,
    speech_start_time: f64,

    // Channels
    command_tx: Sender<TranscriptionCommand>,
    command_rx: Receiver<TranscriptionCommand>,
    event_tx: Sender<TranscriptionEvent>,
    event_rx: Receiver<TranscriptionEvent>,
}

impl TranscriptionPipeline {
    /// Create a new transcription pipeline
    pub fn new(config: WhisperConfig, vad: VoiceActivityDetector) -> Result<Self> {
        let (command_tx, command_rx) = bounded(100);
        let (event_tx, event_rx) = bounded(100);

        Ok(Self {
            config,
            vad,
            audio_buffer: Vec::new(),
            buffer_start_time: 0.0,
            current_time: 0.0,
            is_in_speech: false,
            silence_duration: 0.0,
            speech_start_time: 0.0,
            command_tx,
            command_rx,
            event_tx,
            event_rx,
        })
    }

    /// Get a sender for commands
    pub fn command_sender(&self) -> Sender<TranscriptionCommand> {
        self.command_tx.clone()
    }

    /// Get a receiver for events
    pub fn event_receiver(&self) -> Receiver<TranscriptionEvent> {
        self.event_rx.clone()
    }

    /// Process audio chunks with VAD-based segmentation
    ///
    /// Returns true if a segment was triggered for transcription
    pub fn process_audio(&mut self, audio: &[f32], timestamp: f64) -> Result<bool> {
        let chunk_duration = audio.len() as f32 / 16000.0;

        // Run VAD on the audio chunk
        let is_speech = self.vad.is_speech(audio)?;

        let mut segment_triggered = false;

        if is_speech {
            if !self.is_in_speech {
                // Transition from silence to speech
                self.is_in_speech = true;
                self.speech_start_time = timestamp;
                self.buffer_start_time = timestamp;
                self.audio_buffer.clear();
                debug!("Speech started at {:.2}s", timestamp);
            }

            // Accumulate audio
            self.audio_buffer.extend_from_slice(audio);
            self.silence_duration = 0.0;

            // Check if segment is too long
            let segment_duration = self.audio_buffer.len() as f32 / 16000.0;
            if segment_duration >= self.config.max_segment_duration {
                debug!("Maximum segment duration reached, triggering transcription");
                self.trigger_transcription()?;
                segment_triggered = true;
            }
        } else {
            if self.is_in_speech {
                // In speech but current chunk is silence
                self.audio_buffer.extend_from_slice(audio);
                self.silence_duration += chunk_duration;

                // Check if we've had enough silence to end the segment
                if self.silence_duration >= self.config.silence_threshold {
                    let segment_duration = self.audio_buffer.len() as f32 / 16000.0;

                    if segment_duration >= self.config.min_segment_duration {
                        debug!("Silence threshold reached, triggering transcription");
                        self.trigger_transcription()?;
                        segment_triggered = true;
                    } else {
                        debug!("Segment too short ({:.2}s), discarding", segment_duration);
                        self.reset_state();
                    }
                }
            }
        }

        self.current_time = timestamp + chunk_duration as f64;

        Ok(segment_triggered)
    }

    /// Trigger transcription of the accumulated audio buffer
    fn trigger_transcription(&mut self) -> Result<()> {
        if self.audio_buffer.is_empty() {
            return Ok(());
        }

        let segment = AudioSegment::new(self.audio_buffer.clone(), true, self.buffer_start_time);

        self.command_tx
            .send(TranscriptionCommand::Transcribe(segment))
            .map_err(|e| {
                BabbleError::TranscriptionError(format!("Failed to send command: {}", e))
            })?;

        self.reset_state();

        Ok(())
    }

    /// Reset the internal state
    fn reset_state(&mut self) {
        self.audio_buffer.clear();
        self.is_in_speech = false;
        self.silence_duration = 0.0;
    }

    /// Start the transcription worker thread
    pub fn start_worker(self) -> Result<()> {
        let config = self.config.clone();
        let command_rx = self.command_rx.clone();
        let event_tx = self.event_tx.clone();

        std::thread::spawn(move || {
            info!("Transcription worker started");

            // Initialize the Whisper engine
            let engine = match WhisperEngine::new(config) {
                Ok(engine) => engine,
                Err(e) => {
                    error!("Failed to initialize Whisper engine: {}", e);
                    let _ = event_tx.send(TranscriptionEvent::Error(e.to_string()));
                    let _ = event_tx.send(TranscriptionEvent::Shutdown);
                    return;
                }
            };

            // Process commands
            loop {
                match command_rx.recv() {
                    Ok(TranscriptionCommand::Transcribe(segment)) => {
                        debug!("Processing transcription request");

                        match engine.transcribe(&segment) {
                            Ok(result) => {
                                if let Err(e) = event_tx.send(TranscriptionEvent::Result(result)) {
                                    error!("Failed to send transcription result: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Transcription error: {}", e);
                                let _ = event_tx.send(TranscriptionEvent::Error(e.to_string()));
                            }
                        }
                    }
                    Ok(TranscriptionCommand::Shutdown) => {
                        info!("Transcription worker shutting down");
                        let _ = event_tx.send(TranscriptionEvent::Shutdown);
                        break;
                    }
                    Err(e) => {
                        error!("Command channel error: {}", e);
                        break;
                    }
                }
            }

            info!("Transcription worker stopped");
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_config_default() {
        let config = WhisperConfig::default();
        assert_eq!(config.language, Some("en".to_string()));
        assert_eq!(config.n_threads, 4);
        assert!(!config.translate);
    }

    #[test]
    fn test_audio_segment_creation() {
        let samples = vec![0.0f32; 16000]; // 1 second at 16kHz
        let segment = AudioSegment::new(samples.clone(), true, 0.0);

        assert_eq!(segment.samples.len(), 16000);
        assert!(segment.is_speech);
        assert_eq!(segment.start_time, 0.0);
        assert!((segment.duration - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_transcription_result() {
        let result = TranscriptionResult {
            text: "Hello world".to_string(),
            start_time: 0.0,
            end_time: 1.5,
            confidence: Some(0.95),
        };

        assert_eq!(result.text, "Hello world");
        assert_eq!(result.start_time, 0.0);
        assert_eq!(result.end_time, 1.5);
        assert_eq!(result.confidence, Some(0.95));
    }
}
