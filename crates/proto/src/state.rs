//! Unified application state for the Proto voice assistant
//!
//! This module provides a thread-safe shared state that can be accessed by:
//! - **Orchestrator**: Writes state changes based on processor events
//! - **UI**: Reads state for rendering, sends commands
//! - **TestRunner**: Reads state for assertions, sends commands
//!
//! The design separates:
//! - **State**: Shared data that can be queried synchronously
//! - **Commands**: Requests to change state (sent to orchestrator)
//! - **Events**: Notifications for UI updates (streaming tokens, errors)

use parking_lot::RwLock;
use std::sync::Arc;

/// Recording pipeline state
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecordingState {
    /// No recording in progress
    #[default]
    Idle,
    /// Actively recording audio from microphone
    Recording,
    /// Recording stopped, audio being processed by STT
    Processing,
}

impl RecordingState {
    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        matches!(self, RecordingState::Recording)
    }

    /// Check if processing (STT running)
    pub fn is_processing(&self) -> bool {
        matches!(self, RecordingState::Processing)
    }

    /// Check if idle
    pub fn is_idle(&self) -> bool {
        matches!(self, RecordingState::Idle)
    }

    /// Check if in an active state (not idle)
    pub fn is_active(&self) -> bool {
        !self.is_idle()
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

/// LLM generation state
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LLMState {
    /// LLM is idle, ready for input
    #[default]
    Idle,
    /// LLM is generating a response
    Generating,
}

impl LLMState {
    /// Check if currently generating
    pub fn is_generating(&self) -> bool {
        matches!(self, LLMState::Generating)
    }

    /// Check if idle
    pub fn is_idle(&self) -> bool {
        matches!(self, LLMState::Idle)
    }
}

impl std::fmt::Display for LLMState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LLMState::Idle => write!(f, "Idle"),
            LLMState::Generating => write!(f, "Generating"),
        }
    }
}

/// Transcription state from STT
#[derive(Clone, Debug, Default)]
pub struct TranscriptionState {
    /// Last completed transcription text
    pub last_text: Option<String>,
    /// Whether first word has been detected (for command processing)
    pub has_first_word: bool,
    /// The detected first word (if any)
    pub first_word: Option<String>,
}

impl TranscriptionState {
    /// Create a new empty transcription state
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear transcription state for new recording
    pub fn clear(&mut self) {
        self.last_text = None;
        self.has_first_word = false;
        self.first_word = None;
    }

    /// Set the first word
    pub fn set_first_word(&mut self, word: String) {
        self.first_word = Some(word);
        self.has_first_word = true;
    }

    /// Set the final transcription
    pub fn set_transcription(&mut self, text: String) {
        self.last_text = Some(text);
    }
}

/// LLM response state
#[derive(Clone, Debug, Default)]
pub struct ResponseState {
    /// Current response being generated (accumulated tokens)
    pub current_text: String,
    /// Whether the last response was interrupted
    pub was_interrupted: bool,
    /// Last complete response
    pub last_complete: Option<String>,
}

impl ResponseState {
    /// Create a new empty response state
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear for new generation
    pub fn start_generation(&mut self) {
        self.current_text.clear();
        self.was_interrupted = false;
    }

    /// Append a token to current response
    pub fn append_token(&mut self, token: &str) {
        self.current_text.push_str(token);
    }

    /// Mark generation as complete
    pub fn complete(&mut self, interrupted: bool) {
        self.was_interrupted = interrupted;
        if !self.current_text.is_empty() {
            self.last_complete = Some(self.current_text.clone());
        }
    }

    /// Clear all response state
    pub fn clear(&mut self) {
        self.current_text.clear();
        self.was_interrupted = false;
        self.last_complete = None;
    }
}

/// Unified application state
///
/// This is the single source of truth for application state.
/// It can be shared across threads using `SharedAppState`.
#[derive(Clone, Debug, Default)]
pub struct AppState {
    /// Recording/STT pipeline state
    pub recording: RecordingState,
    /// LLM generation state
    pub llm: LLMState,
    /// Transcription results
    pub transcription: TranscriptionState,
    /// LLM response
    pub response: ResponseState,
    /// Current error (if any)
    pub error: Option<String>,
    /// Audio buffer sample count (for assertions/UI)
    pub audio_buffer_samples: usize,
    /// Frame counter for debugging
    pub frame_count: u64,
    /// Debug mode enabled
    pub debug_mode: bool,
    /// Max frames before exit (0 = unlimited)
    pub max_frames: u64,
}

impl AppState {
    /// Create a new default state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an immutable snapshot of current state
    pub fn snapshot(&self) -> AppStateSnapshot {
        AppStateSnapshot {
            recording: self.recording,
            llm: self.llm,
            transcription: self.transcription.clone(),
            response: self.response.clone(),
            error: self.error.clone(),
            audio_buffer_samples: self.audio_buffer_samples,
            frame_count: self.frame_count,
            debug_mode: self.debug_mode,
            max_frames: self.max_frames,
        }
    }

    /// Check if system is completely idle
    pub fn is_idle(&self) -> bool {
        self.recording.is_idle() && self.llm.is_idle()
    }

    /// Check if system is busy (recording, processing, or generating)
    pub fn is_busy(&self) -> bool {
        !self.is_idle()
    }

    /// Set an error
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Clear the current error
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    // === State transitions ===

    /// Start recording
    pub fn start_recording(&mut self) {
        self.recording = RecordingState::Recording;
        self.transcription.clear();
        self.clear_error();
    }

    /// Stop recording and begin processing
    pub fn stop_recording(&mut self) {
        self.recording = RecordingState::Processing;
    }

    /// Cancel recording without processing
    pub fn cancel_recording(&mut self) {
        self.recording = RecordingState::Idle;
        self.transcription.clear();
    }

    /// Finish STT processing
    pub fn finish_processing(&mut self) {
        self.recording = RecordingState::Idle;
    }

    /// Start LLM generation
    pub fn start_generation(&mut self) {
        self.llm = LLMState::Generating;
        self.response.start_generation();
    }

    /// Finish LLM generation
    pub fn finish_generation(&mut self, interrupted: bool) {
        self.llm = LLMState::Idle;
        self.response.complete(interrupted);
    }
}

/// Immutable snapshot of application state
///
/// Used for event emission and thread-safe reads without holding locks.
#[derive(Clone, Debug)]
pub struct AppStateSnapshot {
    pub recording: RecordingState,
    pub llm: LLMState,
    pub transcription: TranscriptionState,
    pub response: ResponseState,
    pub error: Option<String>,
    pub audio_buffer_samples: usize,
    pub frame_count: u64,
    pub debug_mode: bool,
    pub max_frames: u64,
}

/// Thread-safe shared application state
///
/// This wraps `AppState` in `Arc<RwLock<>>` for safe concurrent access.
#[derive(Clone)]
pub struct SharedAppState {
    inner: Arc<RwLock<AppState>>,
}

impl Default for SharedAppState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedAppState {
    /// Create a new shared state
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppState::new())),
        }
    }

    /// Get a read lock on the state
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, AppState> {
        self.inner.read()
    }

    /// Get a write lock on the state
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, AppState> {
        self.inner.write()
    }

    /// Get a snapshot of current state (no lock held after return)
    pub fn snapshot(&self) -> AppStateSnapshot {
        self.inner.read().snapshot()
    }

    // === Convenience read methods ===

    /// Check if recording
    pub fn is_recording(&self) -> bool {
        self.inner.read().recording.is_recording()
    }

    /// Check if processing
    pub fn is_processing(&self) -> bool {
        self.inner.read().recording.is_processing()
    }

    /// Check if LLM is generating
    pub fn is_generating(&self) -> bool {
        self.inner.read().llm.is_generating()
    }

    /// Check if system is idle
    pub fn is_idle(&self) -> bool {
        self.inner.read().is_idle()
    }

    /// Get current recording state
    pub fn recording_state(&self) -> RecordingState {
        self.inner.read().recording
    }

    /// Get current LLM state
    pub fn llm_state(&self) -> LLMState {
        self.inner.read().llm
    }

    /// Get last transcription text
    pub fn last_transcription(&self) -> Option<String> {
        self.inner.read().transcription.last_text.clone()
    }

    /// Get current response text
    pub fn current_response(&self) -> String {
        self.inner.read().response.current_text.clone()
    }

    /// Get audio buffer sample count
    pub fn audio_buffer_samples(&self) -> usize {
        self.inner.read().audio_buffer_samples
    }

    /// Get current frame count
    pub fn frame_count(&self) -> u64 {
        self.inner.read().frame_count
    }

    /// Check if debug mode is enabled
    pub fn is_debug_mode(&self) -> bool {
        self.inner.read().debug_mode
    }

    /// Get max frames limit (0 = unlimited)
    pub fn max_frames(&self) -> u64 {
        self.inner.read().max_frames
    }
}

/// Commands that can be sent to control the application
///
/// These are processed by the orchestrator and result in state changes.
#[derive(Clone, Debug)]
pub enum AppCommand {
    /// Start recording audio
    StartRecording,
    /// Stop recording and process with STT
    StopRecording,
    /// Cancel recording without processing
    CancelRecording,
    /// Send text directly to LLM (bypasses STT)
    SendText(String),
    /// Stop current LLM generation
    StopGeneration,
    /// Clear conversation history
    ClearHistory,
    /// Shutdown all processors
    Shutdown,
}

/// Events emitted by the application
///
/// These are used for UI updates and logging. State should be queried
/// directly from `SharedAppState` rather than reconstructed from events.
#[derive(Clone, Debug)]
pub enum AppEvent {
    /// State has changed (trigger UI repaint)
    StateChanged,
    /// LLM token received (for streaming display)
    LLMToken(String),
    /// Error occurred
    Error(String),
    /// Shutdown complete
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_state_transitions() {
        let mut state = AppState::new();
        assert!(state.recording.is_idle());

        state.start_recording();
        assert!(state.recording.is_recording());

        state.stop_recording();
        assert!(state.recording.is_processing());

        state.finish_processing();
        assert!(state.recording.is_idle());
    }

    #[test]
    fn test_cancel_recording() {
        let mut state = AppState::new();
        state.start_recording();
        state.cancel_recording();
        assert!(state.recording.is_idle());
    }

    #[test]
    fn test_llm_state_transitions() {
        let mut state = AppState::new();
        assert!(state.llm.is_idle());

        state.start_generation();
        assert!(state.llm.is_generating());

        state.finish_generation(false);
        assert!(state.llm.is_idle());
        assert!(!state.response.was_interrupted);
    }

    #[test]
    fn test_llm_interruption() {
        let mut state = AppState::new();
        state.start_generation();
        state.response.append_token("Hello ");
        state.response.append_token("world");
        state.finish_generation(true);

        assert!(state.llm.is_idle());
        assert!(state.response.was_interrupted);
        assert_eq!(
            state.response.last_complete,
            Some("Hello world".to_string())
        );
    }

    #[test]
    fn test_transcription_state() {
        let mut state = AppState::new();
        assert!(!state.transcription.has_first_word);
        assert!(state.transcription.last_text.is_none());

        state.transcription.set_first_word("stop".to_string());
        assert!(state.transcription.has_first_word);
        assert_eq!(state.transcription.first_word, Some("stop".to_string()));

        state
            .transcription
            .set_transcription("stop the music".to_string());
        assert_eq!(
            state.transcription.last_text,
            Some("stop the music".to_string())
        );

        state.transcription.clear();
        assert!(!state.transcription.has_first_word);
        assert!(state.transcription.last_text.is_none());
    }

    #[test]
    fn test_shared_state() {
        let shared = SharedAppState::new();

        assert!(shared.is_idle());
        assert!(!shared.is_recording());

        {
            let mut state = shared.write();
            state.start_recording();
        }

        assert!(shared.is_recording());
        assert!(!shared.is_idle());

        let snapshot = shared.snapshot();
        assert!(snapshot.recording.is_recording());
    }

    #[test]
    fn test_snapshot_is_independent() {
        let shared = SharedAppState::new();

        let snapshot1 = shared.snapshot();
        assert!(snapshot1.recording.is_idle());

        {
            shared.write().start_recording();
        }

        // snapshot1 should still show idle
        assert!(snapshot1.recording.is_idle());

        // new snapshot shows recording
        let snapshot2 = shared.snapshot();
        assert!(snapshot2.recording.is_recording());
    }

    #[test]
    fn test_is_busy() {
        let mut state = AppState::new();
        assert!(!state.is_busy());

        state.start_recording();
        assert!(state.is_busy());

        state.cancel_recording();
        assert!(!state.is_busy());

        state.start_generation();
        assert!(state.is_busy());
    }

    #[test]
    fn test_app_command_variants() {
        let _start = AppCommand::StartRecording;
        let _stop = AppCommand::StopRecording;
        let _cancel = AppCommand::CancelRecording;
        let _text = AppCommand::SendText("test".to_string());
        let _stop_gen = AppCommand::StopGeneration;
        let _clear = AppCommand::ClearHistory;
        let _shutdown = AppCommand::Shutdown;
    }

    #[test]
    fn test_app_event_variants() {
        let _changed = AppEvent::StateChanged;
        let _token = AppEvent::LLMToken("hello".to_string());
        let _error = AppEvent::Error("test error".to_string());
        let _shutdown = AppEvent::Shutdown;
    }

    #[test]
    fn test_debug_fields_default() {
        let state = AppState::new();
        assert_eq!(state.frame_count, 0);
        assert!(!state.debug_mode);
        assert_eq!(state.max_frames, 0);
    }

    #[test]
    fn test_frame_counter() {
        let shared = SharedAppState::new();

        assert_eq!(shared.frame_count(), 0);

        {
            let mut state = shared.write();
            state.frame_count += 1;
        }
        assert_eq!(shared.frame_count(), 1);

        {
            let mut state = shared.write();
            state.frame_count += 99;
        }
        assert_eq!(shared.frame_count(), 100);
    }

    #[test]
    fn test_debug_mode() {
        let shared = SharedAppState::new();

        assert!(!shared.is_debug_mode());

        {
            let mut state = shared.write();
            state.debug_mode = true;
        }
        assert!(shared.is_debug_mode());
    }

    #[test]
    fn test_max_frames() {
        let shared = SharedAppState::new();

        assert_eq!(shared.max_frames(), 0);

        {
            let mut state = shared.write();
            state.max_frames = 100;
        }
        assert_eq!(shared.max_frames(), 100);
    }

    #[test]
    fn test_snapshot_includes_debug_fields() {
        let shared = SharedAppState::new();

        {
            let mut state = shared.write();
            state.debug_mode = true;
            state.max_frames = 50;
            state.frame_count = 25;
        }

        let snapshot = shared.snapshot();
        assert!(snapshot.debug_mode);
        assert_eq!(snapshot.max_frames, 50);
        assert_eq!(snapshot.frame_count, 25);
    }
}
