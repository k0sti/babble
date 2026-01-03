//! Text-to-speech implementation with sherpa-rs (VITS models)
//!
//! This module provides TTS synthesis using VITS neural TTS models via sherpa-rs,
//! with streaming support for LLM response segments.

use crate::audio::resampler::resample_audio;
use crate::llm::tts_parser::TTSSegment;
use crate::{BabbleError, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use sherpa_rs::tts::{VitsTts, VitsTtsConfig};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Default sample rate for VITS TTS output (22050 Hz typical for Piper models)
pub const VITS_SAMPLE_RATE: u32 = 22050;

/// Configuration for the TTS engine
#[derive(Clone, Debug)]
pub struct TTSConfig {
    /// Path to the ONNX model file
    pub model_path: String,

    /// Path to the tokens file
    pub tokens_path: String,

    /// Path to the lexicon file (optional for some models)
    pub lexicon_path: Option<String>,

    /// Path to the data directory (optional)
    pub data_dir: Option<String>,

    /// Path to dict directory (optional)
    pub dict_dir: Option<String>,

    /// Length scale for speech rate (1.0 = normal, <1.0 = faster, >1.0 = slower)
    pub length_scale: f32,

    /// Noise scale for variation
    pub noise_scale: f32,

    /// Noise scale width
    pub noise_scale_w: f32,

    /// Optional speaker ID for multi-speaker models
    pub speaker_id: i32,

    /// Output sample rate (will resample if different from model's native rate)
    pub output_sample_rate: u32,

    /// Maximum queue size for pending TTS requests
    pub queue_size: usize,
}

impl Default for TTSConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            tokens_path: String::new(),
            lexicon_path: None,
            data_dir: None,
            dict_dir: None,
            length_scale: 1.0,
            noise_scale: 0.667,
            noise_scale_w: 0.8,
            speaker_id: 0,
            output_sample_rate: 22050,
            queue_size: 100,
        }
    }
}

impl TTSConfig {
    /// Create a new TTS config with required paths
    pub fn new(model_path: impl Into<String>, tokens_path: impl Into<String>) -> Self {
        Self {
            model_path: model_path.into(),
            tokens_path: tokens_path.into(),
            ..Default::default()
        }
    }

    /// Set the lexicon path
    pub fn with_lexicon(mut self, lexicon_path: impl Into<String>) -> Self {
        self.lexicon_path = Some(lexicon_path.into());
        self
    }

    /// Set the data directory
    pub fn with_data_dir(mut self, data_dir: impl Into<String>) -> Self {
        self.data_dir = Some(data_dir.into());
        self
    }

    /// Set the dict directory
    pub fn with_dict_dir(mut self, dict_dir: impl Into<String>) -> Self {
        self.dict_dir = Some(dict_dir.into());
        self
    }

    /// Set the speaker ID for multi-speaker models
    pub fn with_speaker(mut self, speaker_id: i32) -> Self {
        self.speaker_id = speaker_id;
        self
    }

    /// Set the speech rate (length scale)
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.length_scale = 1.0 / speed.max(0.1); // Invert: higher speed = lower length_scale
        self
    }

    /// Set the output sample rate
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.output_sample_rate = sample_rate;
        self
    }
}

/// Generated audio data from TTS
#[derive(Clone, Debug)]
pub struct TTSAudio {
    /// Audio samples (f32, mono)
    pub samples: Vec<f32>,

    /// Sample rate of the audio
    pub sample_rate: u32,

    /// Index of the segment this audio belongs to
    pub segment_index: usize,

    /// Request ID this audio belongs to
    pub request_id: Uuid,
}

impl TTSAudio {
    /// Get the duration of this audio in seconds
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }

    /// Get the duration of this audio in milliseconds
    pub fn duration_ms(&self) -> u64 {
        (self.samples.len() as u64 * 1000) / self.sample_rate as u64
    }
}

/// Command sent to the TTS pipeline
#[derive(Clone, Debug)]
pub enum TTSCommand {
    /// Synthesize a TTS segment
    Synthesize {
        /// The segment to synthesize
        segment: TTSSegment,
        /// Request ID for tracking
        request_id: Uuid,
    },

    /// Update the speaker ID
    SetSpeaker(i32),

    /// Shutdown the pipeline
    Shutdown,
}

/// Event emitted by the TTS pipeline
#[derive(Clone, Debug)]
pub enum TTSEvent {
    /// Audio was successfully generated
    Audio(TTSAudio),

    /// An error occurred during synthesis
    Error {
        /// Error message
        error: String,
        /// Segment index if applicable
        segment_index: Option<usize>,
        /// Request ID if applicable
        request_id: Option<Uuid>,
    },

    /// Pipeline has shut down
    Shutdown,
}

/// TTS Engine wrapping sherpa-rs VitsTts
pub struct TTSEngine {
    tts: VitsTts,
    config: TTSConfig,
    model_sample_rate: u32,
}

impl TTSEngine {
    /// Create a new TTS engine
    pub fn new(config: TTSConfig) -> Result<Self> {
        if config.model_path.is_empty() {
            return Err(BabbleError::ConfigError("Model path is required".into()));
        }

        if config.tokens_path.is_empty() {
            return Err(BabbleError::ConfigError("Tokens path is required".into()));
        }

        let model_path = Path::new(&config.model_path);
        if !model_path.exists() {
            return Err(BabbleError::ModelLoadError(format!(
                "Model not found: {}",
                config.model_path
            )));
        }

        let tokens_path = Path::new(&config.tokens_path);
        if !tokens_path.exists() {
            return Err(BabbleError::ModelLoadError(format!(
                "Tokens file not found: {}",
                config.tokens_path
            )));
        }

        info!("Loading VITS TTS model from: {}", config.model_path);

        let vits_config = VitsTtsConfig {
            model: config.model_path.clone(),
            tokens: config.tokens_path.clone(),
            lexicon: config.lexicon_path.clone().unwrap_or_default(),
            data_dir: config.data_dir.clone().unwrap_or_default(),
            dict_dir: config.dict_dir.clone().unwrap_or_default(),
            length_scale: config.length_scale,
            noise_scale: config.noise_scale,
            noise_scale_w: config.noise_scale_w,
            ..Default::default()
        };

        let tts = VitsTts::new(vits_config);

        info!("TTS engine initialized successfully");

        Ok(Self {
            tts,
            config,
            model_sample_rate: VITS_SAMPLE_RATE, // Will be updated from actual audio
        })
    }

    /// Synthesize text to audio samples
    pub fn synthesize(&mut self, text: &str) -> Result<(Vec<f32>, u32)> {
        if text.trim().is_empty() {
            return Ok((Vec::new(), self.config.output_sample_rate));
        }

        // Normalize text for TTS
        let normalized = normalize_text_for_tts(text);
        if normalized.is_empty() {
            return Ok((Vec::new(), self.config.output_sample_rate));
        }

        debug!("Synthesizing: {}", normalized);

        // Generate audio
        let audio = self
            .tts
            .create(&normalized, self.config.speaker_id, 1.0)
            .map_err(|e| BabbleError::TTSError(format!("Synthesis failed: {}", e)))?;

        let mut samples = audio.samples;
        let model_sample_rate = audio.sample_rate as u32;
        self.model_sample_rate = model_sample_rate;

        // Resample if needed
        if self.config.output_sample_rate != model_sample_rate {
            samples = resample_audio(
                &samples,
                model_sample_rate,
                self.config.output_sample_rate,
                1, // mono
            )?;
        }

        debug!(
            "Synthesized {} samples ({:.2}s)",
            samples.len(),
            samples.len() as f32 / self.config.output_sample_rate as f32
        );

        Ok((samples, self.config.output_sample_rate))
    }

    /// Synthesize a TTS segment
    pub fn synthesize_segment(&mut self, segment: &TTSSegment, request_id: Uuid) -> Result<TTSAudio> {
        let (samples, sample_rate) = self.synthesize(&segment.text)?;

        Ok(TTSAudio {
            samples,
            sample_rate,
            segment_index: segment.index,
            request_id,
        })
    }

    /// Get the output sample rate
    pub fn sample_rate(&self) -> u32 {
        self.config.output_sample_rate
    }
}

/// TTS Pipeline with channel-based communication
///
/// Provides async TTS synthesis with queuing and threading support.
pub struct TTSPipeline {
    /// Configuration
    config: TTSConfig,

    /// Command sender
    command_tx: Sender<TTSCommand>,

    /// Command receiver (for worker)
    command_rx: Receiver<TTSCommand>,

    /// Event sender (for worker)
    event_tx: Sender<TTSEvent>,

    /// Event receiver
    event_rx: Receiver<TTSEvent>,
}

impl TTSPipeline {
    /// Create a new TTS pipeline
    pub fn new(config: TTSConfig) -> Self {
        let (command_tx, command_rx) = bounded(config.queue_size);
        let (event_tx, event_rx) = bounded(config.queue_size);

        Self {
            config,
            command_tx,
            command_rx,
            event_tx,
            event_rx,
        }
    }

    /// Get a sender for commands
    pub fn command_sender(&self) -> Sender<TTSCommand> {
        self.command_tx.clone()
    }

    /// Get a receiver for events
    pub fn event_receiver(&self) -> Receiver<TTSEvent> {
        self.event_rx.clone()
    }

    /// Start the pipeline worker thread
    /// Returns the JoinHandle for the worker thread.
    pub fn start_worker(self) -> Result<thread::JoinHandle<()>> {
        let config = self.config.clone();
        let command_rx = self.command_rx.clone();
        let event_tx = self.event_tx.clone();

        let handle = thread::spawn(move || {
            info!("TTS pipeline worker starting");

            // Initialize the TTS engine
            let mut engine = match TTSEngine::new(config) {
                Ok(engine) => engine,
                Err(e) => {
                    error!("Failed to initialize TTS engine: {}", e);
                    let _ = event_tx.send(TTSEvent::Error {
                        error: e.to_string(),
                        segment_index: None,
                        request_id: None,
                    });
                    let _ = event_tx.send(TTSEvent::Shutdown);
                    return;
                }
            };

            info!("TTS pipeline worker ready");

            // Process commands
            loop {
                match command_rx.recv() {
                    Ok(TTSCommand::Synthesize { segment, request_id }) => {
                        if !segment.should_speak {
                            // Skip non-spoken segments
                            continue;
                        }

                        debug!(
                            "Processing TTS segment {}: {}",
                            segment.index,
                            &segment.text[..segment.text.len().min(50)]
                        );

                        match engine.synthesize_segment(&segment, request_id) {
                            Ok(audio) => {
                                if !audio.samples.is_empty() {
                                    let _ = event_tx.send(TTSEvent::Audio(audio));
                                }
                            }
                            Err(e) => {
                                warn!("TTS synthesis failed for segment {}: {}", segment.index, e);
                                let _ = event_tx.send(TTSEvent::Error {
                                    error: e.to_string(),
                                    segment_index: Some(segment.index),
                                    request_id: Some(request_id),
                                });
                            }
                        }
                    }

                    Ok(TTSCommand::SetSpeaker(speaker_id)) => {
                        info!("Speaker ID change requested: {}", speaker_id);
                        // Note: Would need to recreate engine with new speaker
                        // For now, just log the request
                    }

                    Ok(TTSCommand::Shutdown) => {
                        info!("TTS pipeline worker shutting down");
                        let _ = event_tx.send(TTSEvent::Shutdown);
                        break;
                    }

                    Err(e) => {
                        error!("Command channel error: {}", e);
                        break;
                    }
                }
            }

            info!("TTS pipeline worker stopped");
        });

        Ok(handle)
    }
}

/// Audio queue for buffering TTS output
///
/// Provides thread-safe queuing of audio chunks for playback.
pub struct AudioQueue {
    /// Queued audio segments, ordered by segment index
    segments: Arc<Mutex<Vec<TTSAudio>>>,

    /// Next segment index expected for playback
    next_playback_index: Arc<Mutex<usize>>,

    /// Current request ID being processed
    current_request: Arc<Mutex<Option<Uuid>>>,
}

impl AudioQueue {
    /// Create a new audio queue
    pub fn new() -> Self {
        Self {
            segments: Arc::new(Mutex::new(Vec::new())),
            next_playback_index: Arc::new(Mutex::new(0)),
            current_request: Arc::new(Mutex::new(None)),
        }
    }

    /// Add an audio segment to the queue
    pub fn enqueue(&self, audio: TTSAudio) {
        let mut segments = self.segments.lock();
        let mut current = self.current_request.lock();

        // If this is a new request, clear the queue
        if current.map(|r| r != audio.request_id).unwrap_or(true) {
            segments.clear();
            *self.next_playback_index.lock() = 0;
            *current = Some(audio.request_id);
        }

        // Insert in order by segment index
        let pos = segments
            .iter()
            .position(|s| s.segment_index > audio.segment_index)
            .unwrap_or(segments.len());
        segments.insert(pos, audio);
    }

    /// Get the next audio segment ready for playback
    ///
    /// Returns the next segment in order, or None if not yet available.
    pub fn dequeue(&self) -> Option<TTSAudio> {
        let mut segments = self.segments.lock();
        let mut next_idx = self.next_playback_index.lock();

        // Find the segment with the expected index
        if let Some(pos) = segments.iter().position(|s| s.segment_index == *next_idx) {
            *next_idx += 1;
            Some(segments.remove(pos))
        } else {
            None
        }
    }

    /// Get all available samples for playback
    ///
    /// Returns samples in segment order, collecting contiguous segments.
    pub fn drain_available(&self) -> Vec<f32> {
        let mut samples = Vec::new();

        while let Some(audio) = self.dequeue() {
            samples.extend(audio.samples);
        }

        samples
    }

    /// Clear all pending audio
    pub fn clear(&self) {
        self.segments.lock().clear();
        *self.next_playback_index.lock() = 0;
        *self.current_request.lock() = None;
    }

    /// Get the number of segments in the queue
    pub fn len(&self) -> usize {
        self.segments.lock().len()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.segments.lock().is_empty()
    }

    /// Get total duration of queued audio in seconds
    pub fn total_duration_secs(&self) -> f32 {
        self.segments
            .lock()
            .iter()
            .map(|s| s.duration_secs())
            .sum()
    }
}

impl Default for AudioQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize text for TTS synthesis
///
/// Handles common abbreviations, numbers, and punctuation for better speech output.
pub fn normalize_text_for_tts(text: &str) -> String {
    let mut result = text.to_string();

    // Expand common abbreviations
    let abbreviations = [
        ("Mr.", "Mister"),
        ("Mrs.", "Misses"),
        ("Ms.", "Miss"),
        ("Dr.", "Doctor"),
        ("Prof.", "Professor"),
        ("Jr.", "Junior"),
        ("Sr.", "Senior"),
        ("vs.", "versus"),
        ("etc.", "etcetera"),
        ("e.g.", "for example"),
        ("i.e.", "that is"),
        ("approx.", "approximately"),
        ("govt.", "government"),
        ("dept.", "department"),
        ("st.", "street"),
        ("ave.", "avenue"),
        ("blvd.", "boulevard"),
        ("no.", "number"),
        ("vol.", "volume"),
        ("pg.", "page"),
        ("pp.", "pages"),
        ("hrs.", "hours"),
        ("mins.", "minutes"),
        ("secs.", "seconds"),
        ("lb.", "pounds"),
        ("lbs.", "pounds"),
        ("oz.", "ounces"),
        ("ft.", "feet"),
        ("in.", "inches"),
        ("yd.", "yards"),
        ("mi.", "miles"),
        ("km.", "kilometers"),
        ("cm.", "centimeters"),
        ("mm.", "millimeters"),
    ];

    for (abbrev, expansion) in abbreviations {
        result = result.replace(abbrev, expansion);
    }

    // Handle common symbols
    result = result.replace("&", " and ");
    result = result.replace("%", " percent");
    result = result.replace("@", " at ");
    result = result.replace("#", " number ");
    result = result.replace("$", " dollars ");
    result = result.replace("€", " euros ");
    result = result.replace("£", " pounds ");
    result = result.replace("+", " plus ");
    result = result.replace("=", " equals ");

    // Expand numbers with ordinal suffixes
    result = expand_ordinals(&result);

    // Handle time formats (e.g., "3:30" -> "three thirty")
    result = expand_time_format(&result);

    // Clean up whitespace
    result = result.split_whitespace().collect::<Vec<_>>().join(" ");

    // Remove or replace problematic characters
    result = result
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".,!?;:'-\"".contains(*c))
        .collect();

    result.trim().to_string()
}

/// Expand ordinal numbers (1st, 2nd, 3rd, etc.)
fn expand_ordinals(text: &str) -> String {
    let mut result = text.to_string();

    // Simple ordinal patterns
    let ordinals = [
        ("1st", "first"),
        ("2nd", "second"),
        ("3rd", "third"),
        ("4th", "fourth"),
        ("5th", "fifth"),
        ("6th", "sixth"),
        ("7th", "seventh"),
        ("8th", "eighth"),
        ("9th", "ninth"),
        ("10th", "tenth"),
        ("11th", "eleventh"),
        ("12th", "twelfth"),
        ("13th", "thirteenth"),
        ("20th", "twentieth"),
        ("21st", "twenty-first"),
        ("22nd", "twenty-second"),
        ("23rd", "twenty-third"),
        ("30th", "thirtieth"),
        ("100th", "hundredth"),
    ];

    for (ordinal, word) in ordinals {
        result = result.replace(ordinal, word);
    }

    result
}

/// Expand time format (e.g., "3:30" -> "three thirty")
fn expand_time_format(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut num = String::from(c);
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() {
                    num.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            // Check if followed by colon and another number (time format)
            if let Some(&':') = chars.peek() {
                chars.next(); // consume ':'
                let mut minutes = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() && minutes.len() < 2 {
                        minutes.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if !minutes.is_empty() {
                    // It's a time format
                    let hour = number_to_words(&num);
                    let min = number_to_words(&minutes);
                    if minutes == "00" {
                        result.push_str(&format!("{} o'clock", hour));
                    } else {
                        result.push_str(&format!("{} {}", hour, min));
                    }
                } else {
                    result.push_str(&num);
                    result.push(':');
                }
            } else {
                result.push_str(&num);
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Convert a number string to words (simple implementation)
fn number_to_words(num_str: &str) -> String {
    let num: u32 = match num_str.parse() {
        Ok(n) => n,
        Err(_) => return num_str.to_string(),
    };

    let ones = [
        "", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen",
        "eighteen", "nineteen",
    ];

    let tens = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];

    match num {
        0 => "zero".to_string(),
        1..=19 => ones[num as usize].to_string(),
        20..=99 => {
            let t = (num / 10) as usize;
            let o = (num % 10) as usize;
            if o == 0 {
                tens[t].to_string()
            } else {
                format!("{}-{}", tens[t], ones[o])
            }
        }
        100..=999 => {
            let h = (num / 100) as usize;
            let rem = num % 100;
            if rem == 0 {
                format!("{} hundred", ones[h])
            } else {
                format!("{} hundred {}", ones[h], number_to_words(&rem.to_string()))
            }
        }
        _ => num_str.to_string(), // Fall back for large numbers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_config_default() {
        let config = TTSConfig::default();
        assert_eq!(config.output_sample_rate, 22050);
        assert_eq!(config.speaker_id, 0);
        assert!(config.lexicon_path.is_none());
    }

    #[test]
    fn test_tts_config_builder() {
        let config = TTSConfig::new("model.onnx", "tokens.txt")
            .with_lexicon("lexicon.txt")
            .with_speaker(5)
            .with_sample_rate(48000)
            .with_speed(1.5);

        assert_eq!(config.model_path, "model.onnx");
        assert_eq!(config.tokens_path, "tokens.txt");
        assert_eq!(config.lexicon_path, Some("lexicon.txt".to_string()));
        assert_eq!(config.speaker_id, 5);
        assert_eq!(config.output_sample_rate, 48000);
        assert!((config.length_scale - 0.667).abs() < 0.01); // 1.0 / 1.5
    }

    #[test]
    fn test_tts_audio_duration() {
        let audio = TTSAudio {
            samples: vec![0.0; 22050],
            sample_rate: 22050,
            segment_index: 0,
            request_id: Uuid::new_v4(),
        };

        assert!((audio.duration_secs() - 1.0).abs() < 0.01);
        assert_eq!(audio.duration_ms(), 1000);
    }

    #[test]
    fn test_normalize_abbreviations() {
        let text = "Dr. Smith met Mr. Johnson at 3:30";
        let normalized = normalize_text_for_tts(text);
        assert!(normalized.contains("Doctor"));
        assert!(normalized.contains("Mister"));
    }

    #[test]
    fn test_normalize_symbols() {
        let text = "50% discount & free shipping";
        let normalized = normalize_text_for_tts(text);
        assert!(normalized.contains("percent"));
        assert!(normalized.contains("and"));
    }

    #[test]
    fn test_normalize_ordinals() {
        let text = "The 1st and 2nd place winners";
        let normalized = normalize_text_for_tts(text);
        assert!(normalized.contains("first"));
        assert!(normalized.contains("second"));
    }

    #[test]
    fn test_number_to_words() {
        assert_eq!(number_to_words("0"), "zero");
        assert_eq!(number_to_words("1"), "one");
        assert_eq!(number_to_words("15"), "fifteen");
        assert_eq!(number_to_words("20"), "twenty");
        assert_eq!(number_to_words("42"), "forty-two");
        assert_eq!(number_to_words("100"), "one hundred");
        assert_eq!(number_to_words("123"), "one hundred twenty-three");
    }

    #[test]
    fn test_audio_queue_ordering() {
        let queue = AudioQueue::new();
        let request_id = Uuid::new_v4();

        // Add segments out of order
        queue.enqueue(TTSAudio {
            samples: vec![2.0],
            sample_rate: 22050,
            segment_index: 2,
            request_id,
        });
        queue.enqueue(TTSAudio {
            samples: vec![0.0],
            sample_rate: 22050,
            segment_index: 0,
            request_id,
        });
        queue.enqueue(TTSAudio {
            samples: vec![1.0],
            sample_rate: 22050,
            segment_index: 1,
            request_id,
        });

        // Should dequeue in order
        let first = queue.dequeue().unwrap();
        assert_eq!(first.segment_index, 0);
        assert_eq!(first.samples[0], 0.0);

        let second = queue.dequeue().unwrap();
        assert_eq!(second.segment_index, 1);
        assert_eq!(second.samples[0], 1.0);

        let third = queue.dequeue().unwrap();
        assert_eq!(third.segment_index, 2);
        assert_eq!(third.samples[0], 2.0);
    }

    #[test]
    fn test_audio_queue_new_request_clears() {
        let queue = AudioQueue::new();
        let request1 = Uuid::new_v4();
        let request2 = Uuid::new_v4();

        queue.enqueue(TTSAudio {
            samples: vec![1.0],
            sample_rate: 22050,
            segment_index: 0,
            request_id: request1,
        });

        assert_eq!(queue.len(), 1);

        // New request should clear the queue
        queue.enqueue(TTSAudio {
            samples: vec![2.0],
            sample_rate: 22050,
            segment_index: 0,
            request_id: request2,
        });

        assert_eq!(queue.len(), 1);
        let audio = queue.dequeue().unwrap();
        assert_eq!(audio.request_id, request2);
    }

    #[test]
    fn test_audio_queue_drain() {
        let queue = AudioQueue::new();
        let request_id = Uuid::new_v4();

        for i in 0..3 {
            queue.enqueue(TTSAudio {
                samples: vec![i as f32; 100],
                sample_rate: 22050,
                segment_index: i,
                request_id,
            });
        }

        let samples = queue.drain_available();
        assert_eq!(samples.len(), 300);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_tts_pipeline_creation() {
        let config = TTSConfig::new("test.onnx", "tokens.txt");
        let pipeline = TTSPipeline::new(config);

        let _cmd_tx = pipeline.command_sender();
        let _event_rx = pipeline.event_receiver();
    }

    #[test]
    fn test_command_variants() {
        let segment = TTSSegment::spoken("Hello".to_string(), 0);
        let request_id = Uuid::new_v4();

        let cmd1 = TTSCommand::Synthesize { segment, request_id };
        let cmd2 = TTSCommand::SetSpeaker(5);
        let cmd3 = TTSCommand::Shutdown;

        match cmd1 {
            TTSCommand::Synthesize { .. } => {}
            _ => panic!("Wrong variant"),
        }

        match cmd2 {
            TTSCommand::SetSpeaker(id) => assert_eq!(id, 5),
            _ => panic!("Wrong variant"),
        }

        match cmd3 {
            TTSCommand::Shutdown => {}
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_event_variants() {
        let audio = TTSAudio {
            samples: vec![0.0],
            sample_rate: 22050,
            segment_index: 0,
            request_id: Uuid::new_v4(),
        };

        let _event1 = TTSEvent::Audio(audio);
        let _event2 = TTSEvent::Error {
            error: "test".to_string(),
            segment_index: Some(0),
            request_id: None,
        };
        let _event3 = TTSEvent::Shutdown;
    }
}
