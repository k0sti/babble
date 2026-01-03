//! Application state management
//!
//! This module provides the central state for the Babble UI.

use crate::llm::{LLMCommand, LLMEvent};
use crate::messages::{AudioData, Message, MessageContent, MessageStorage, Sender};
use crate::speech::tts::{TTSCommand, TTSEvent, AudioQueue};
use crossbeam_channel::{Receiver, Sender as ChannelSender};
use parking_lot::Mutex;
use std::sync::Arc;
use std::collections::VecDeque;
use uuid::Uuid;

/// Recording state for voice input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingState {
    /// Not recording
    Idle,
    /// Currently recording audio
    Recording,
    /// Processing recorded audio (transcription)
    Processing,
}

/// Audio playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// No audio playing
    Stopped,
    /// Audio is playing
    Playing,
    /// Audio is paused
    Paused,
}

/// Debug information displayed in the debug panel
#[derive(Debug, Clone, Default)]
pub struct DebugInfo {
    /// Current transcription status
    pub transcription_status: String,
    /// LLM generation stats (tokens/sec, time to first token)
    pub llm_stats: String,
    /// TTS queue status
    pub tts_queue_status: String,
    /// Audio buffer status
    pub audio_buffer_status: String,
    /// Current frame rate
    pub fps: f32,
    /// Recent log messages
    pub log_messages: VecDeque<String>,
}

impl DebugInfo {
    pub fn new() -> Self {
        Self {
            log_messages: VecDeque::with_capacity(100),
            ..Default::default()
        }
    }

    pub fn add_log(&mut self, message: String) {
        if self.log_messages.len() >= 100 {
            self.log_messages.pop_front();
        }
        self.log_messages.push_back(message);
    }
}

/// Audio player state for the current playlist
#[derive(Debug, Clone)]
pub struct AudioPlayerState {
    /// Current audio being played or queued
    pub current_audio: Option<AudioData>,
    /// Index of current audio in playlist
    pub current_index: usize,
    /// List of audio items to play
    pub playlist: Vec<AudioData>,
    /// Current playback position in samples
    pub playback_position: usize,
    /// Playback state
    pub state: PlaybackState,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
}

impl Default for AudioPlayerState {
    fn default() -> Self {
        Self {
            current_audio: None,
            current_index: 0,
            playlist: Vec::new(),
            playback_position: 0,
            state: PlaybackState::Stopped,
            volume: 0.8,
        }
    }
}

impl AudioPlayerState {
    /// Get the current playback progress as a fraction (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if let Some(audio) = &self.current_audio {
            if audio.samples.is_empty() {
                return 0.0;
            }
            self.playback_position as f32 / audio.samples.len() as f32
        } else {
            0.0
        }
    }

    /// Get the current playback time in seconds
    pub fn current_time(&self) -> f32 {
        if let Some(audio) = &self.current_audio {
            self.playback_position as f32 / audio.sample_rate as f32
        } else {
            0.0
        }
    }

    /// Get the total duration in seconds
    pub fn total_time(&self) -> f32 {
        if let Some(audio) = &self.current_audio {
            audio.duration_seconds()
        } else {
            0.0
        }
    }

    /// Move to the next track
    pub fn next(&mut self) {
        if self.current_index + 1 < self.playlist.len() {
            self.current_index += 1;
            self.current_audio = Some(self.playlist[self.current_index].clone());
            self.playback_position = 0;
        }
    }

    /// Move to the previous track
    pub fn previous(&mut self) {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.current_audio = Some(self.playlist[self.current_index].clone());
            self.playback_position = 0;
        }
    }

    /// Check if there is a next track
    pub fn has_next(&self) -> bool {
        self.current_index + 1 < self.playlist.len()
    }

    /// Check if there is a previous track
    pub fn has_previous(&self) -> bool {
        self.current_index > 0
    }
}

/// Streaming response from the LLM
#[derive(Debug, Clone, Default)]
pub struct StreamingResponse {
    /// The accumulated response text
    pub text: String,
    /// Whether generation is in progress
    pub is_generating: bool,
    /// The request ID for this response
    pub request_id: Option<Uuid>,
    /// Time to first token in milliseconds
    pub first_token_ms: Option<u64>,
    /// Total generation time in milliseconds
    pub total_ms: Option<u64>,
}

/// Central application state
pub struct AppState {
    /// Message storage (thread-safe)
    pub messages: MessageStorage,

    /// Current text input
    pub input_text: String,

    /// Recording state
    pub recording_state: RecordingState,

    /// Audio player state
    pub audio_player: AudioPlayerState,

    /// Current streaming response from LLM
    pub streaming_response: StreamingResponse,

    /// Debug information
    pub debug_info: DebugInfo,

    /// Whether to show the debug panel
    pub show_debug_panel: bool,

    /// Waveform data for visualization (recent audio samples)
    pub waveform_data: Vec<f32>,

    /// TTS audio queue
    pub tts_queue: AudioQueue,

    /// Channel to send LLM commands
    pub llm_command_tx: Option<ChannelSender<LLMCommand>>,

    /// Channel to receive LLM events
    pub llm_event_rx: Option<Receiver<LLMEvent>>,

    /// Channel to send TTS commands
    pub tts_command_tx: Option<ChannelSender<TTSCommand>>,

    /// Channel to receive TTS events
    pub tts_event_rx: Option<Receiver<TTSEvent>>,

    /// Channel to send raw audio for recording
    pub audio_tx: Option<ChannelSender<Vec<f32>>>,

    /// Channel to receive transcription results
    pub transcription_rx: Option<Receiver<String>>,

    /// Recording audio buffer
    pub recording_buffer: Arc<Mutex<Vec<f32>>>,

    /// Last error message
    pub last_error: Option<String>,

    /// Frame time tracking for FPS
    frame_times: VecDeque<f64>,
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
            messages: MessageStorage::new(),
            input_text: String::new(),
            recording_state: RecordingState::Idle,
            audio_player: AudioPlayerState::default(),
            streaming_response: StreamingResponse::default(),
            debug_info: DebugInfo::new(),
            show_debug_panel: false,
            waveform_data: Vec::with_capacity(1024),
            tts_queue: AudioQueue::new(),
            llm_command_tx: None,
            llm_event_rx: None,
            tts_command_tx: None,
            tts_event_rx: None,
            audio_tx: None,
            transcription_rx: None,
            recording_buffer: Arc::new(Mutex::new(Vec::new())),
            last_error: None,
            frame_times: VecDeque::with_capacity(60),
        }
    }

    /// Update FPS calculation
    pub fn update_fps(&mut self, delta_time: f64) {
        self.frame_times.push_back(delta_time);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }

        if !self.frame_times.is_empty() {
            let avg_time: f64 = self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64;
            self.debug_info.fps = if avg_time > 0.0 { 1.0 / avg_time as f32 } else { 0.0 };
        }
    }

    /// Send a text message to the LLM
    pub fn send_message(&mut self) {
        let text = self.input_text.trim().to_string();
        if text.is_empty() {
            return;
        }

        // Add user message to storage
        let user_message = Message::new(Sender::User, MessageContent::Text(text.clone()));
        self.messages.add(user_message);

        // Send to LLM
        if let Some(tx) = &self.llm_command_tx {
            let request_id = Uuid::new_v4();
            let _ = tx.send(LLMCommand::Generate {
                user_message: text,
                request_id,
            });

            self.streaming_response = StreamingResponse {
                text: String::new(),
                is_generating: true,
                request_id: Some(request_id),
                first_token_ms: None,
                total_ms: None,
            };
        }

        // Clear input
        self.input_text.clear();
    }

    /// Start recording audio
    pub fn start_recording(&mut self) {
        self.recording_state = RecordingState::Recording;
        self.recording_buffer.lock().clear();
        self.waveform_data.clear();
        self.debug_info.add_log("Recording started".to_string());
    }

    /// Stop recording and process audio
    pub fn stop_recording(&mut self) {
        if self.recording_state != RecordingState::Recording {
            return;
        }

        self.recording_state = RecordingState::Processing;
        self.debug_info.add_log("Recording stopped, processing...".to_string());

        // The transcription will be handled by the audio pipeline
        // When we receive the transcription, we'll send it to the LLM
    }

    /// Cancel recording without processing
    pub fn cancel_recording(&mut self) {
        self.recording_state = RecordingState::Idle;
        self.recording_buffer.lock().clear();
        self.waveform_data.clear();
        self.debug_info.add_log("Recording cancelled".to_string());
    }

    /// Process incoming events from backend channels
    pub fn poll_events(&mut self) {
        // Poll LLM events
        if let Some(rx) = &self.llm_event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    LLMEvent::Token { token, request_id } => {
                        if self.streaming_response.request_id == Some(request_id) {
                            self.streaming_response.text.push_str(&token);
                        }
                    }
                    LLMEvent::TTSSegment { segment, request_id } => {
                        // Forward to TTS
                        if let Some(tx) = &self.tts_command_tx {
                            let _ = tx.send(TTSCommand::Synthesize { segment, request_id });
                        }
                    }
                    LLMEvent::Complete { full_response, request_id, first_token_ms, total_ms } => {
                        if self.streaming_response.request_id == Some(request_id) {
                            self.streaming_response.text = full_response.clone();
                            self.streaming_response.is_generating = false;
                            self.streaming_response.first_token_ms = Some(first_token_ms);
                            self.streaming_response.total_ms = Some(total_ms);

                            // Add assistant message to storage
                            let msg = Message::new(Sender::Assistant, MessageContent::Text(full_response));
                            self.messages.add(msg);

                            // Update debug info
                            let tokens_per_sec = if total_ms > 0 {
                                (self.streaming_response.text.split_whitespace().count() as f64 * 1000.0) / total_ms as f64
                            } else {
                                0.0
                            };
                            self.debug_info.llm_stats = format!(
                                "First token: {}ms, Total: {}ms, ~{:.1} tokens/s",
                                first_token_ms, total_ms, tokens_per_sec
                            );
                        }
                    }
                    LLMEvent::Error { error, request_id: _ } => {
                        self.last_error = Some(error.clone());
                        self.streaming_response.is_generating = false;
                        self.debug_info.add_log(format!("LLM Error: {}", error));
                    }
                    LLMEvent::Shutdown => {
                        self.debug_info.add_log("LLM pipeline shutdown".to_string());
                    }
                }
            }
        }

        // Poll TTS events
        if let Some(rx) = &self.tts_event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    TTSEvent::Audio(audio) => {
                        self.tts_queue.enqueue(audio);
                        self.debug_info.tts_queue_status = format!(
                            "Queue: {} segments, {:.1}s",
                            self.tts_queue.len(),
                            self.tts_queue.total_duration_secs()
                        );
                    }
                    TTSEvent::Error { error, segment_index, request_id: _ } => {
                        let msg = format!("TTS Error (segment {:?}): {}", segment_index, error);
                        self.debug_info.add_log(msg);
                    }
                    TTSEvent::Shutdown => {
                        self.debug_info.add_log("TTS pipeline shutdown".to_string());
                    }
                }
            }
        }

        // Poll transcription results - collect first, then process
        let transcriptions: Vec<String> = if let Some(rx) = &self.transcription_rx {
            let mut results = Vec::new();
            while let Ok(transcription) = rx.try_recv() {
                results.push(transcription);
            }
            results
        } else {
            Vec::new()
        };

        // Process collected transcriptions
        for transcription in transcriptions {
            self.recording_state = RecordingState::Idle;
            self.debug_info.transcription_status = format!("Last: \"{}\"",
                if transcription.len() > 50 {
                    format!("{}...", &transcription[..50])
                } else {
                    transcription.clone()
                }
            );

            // Send transcription to LLM
            self.input_text = transcription;
            self.send_message();
        }
    }

    /// Add audio samples to the waveform visualization
    pub fn update_waveform(&mut self, samples: &[f32]) {
        const MAX_SAMPLES: usize = 1024;

        // Downsample if needed
        if samples.len() > MAX_SAMPLES {
            let step = samples.len() / MAX_SAMPLES;
            self.waveform_data = samples.iter().step_by(step).take(MAX_SAMPLES).copied().collect();
        } else {
            self.waveform_data.extend_from_slice(samples);
            if self.waveform_data.len() > MAX_SAMPLES {
                self.waveform_data.drain(0..self.waveform_data.len() - MAX_SAMPLES);
            }
        }
    }

    /// Play/pause the current audio
    pub fn toggle_playback(&mut self) {
        match self.audio_player.state {
            PlaybackState::Stopped => {
                if self.audio_player.current_audio.is_some() {
                    self.audio_player.state = PlaybackState::Playing;
                }
            }
            PlaybackState::Playing => {
                self.audio_player.state = PlaybackState::Paused;
            }
            PlaybackState::Paused => {
                self.audio_player.state = PlaybackState::Playing;
            }
        }
    }

    /// Stop audio playback
    pub fn stop_playback(&mut self) {
        self.audio_player.state = PlaybackState::Stopped;
        self.audio_player.playback_position = 0;
    }

    /// Clear all messages
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        if let Some(tx) = &self.llm_command_tx {
            let _ = tx.send(LLMCommand::ClearContext);
        }
    }
}
