//! Test runner for executing test configurations
//!
//! This module provides the TestRunner that schedules and executes
//! test actions at their specified times.
//!
//! The test runner uses the unified `SharedAppState` for assertions,
//! allowing it to query the same state that the UI and orchestrator use.

use super::{ActionType, Assertion, TestConfig};
use crate::state::{AppState, SharedAppState};
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Commands that the test runner can send to the UI/orchestrator
#[derive(Debug, Clone)]
pub enum TestCommand {
    /// Click the record button (toggle recording)
    ClickRecord,
    /// Stop recording
    StopRecord,
    /// Cancel recording
    CancelRecord,
    /// Exit the application
    Exit { code: i32 },
    /// Send text directly to LLM (bypasses STT)
    SendText { text: String },
    /// Stop LLM generation
    StopGeneration,
    /// Capture screenshot to output/<name>.png
    Snapshot { name: String },
    /// Mark test as successful
    ReportSuccess,
    /// Mark test as failed with reason
    ReportFailure { reason: String },
}

/// Result of an assertion check
#[derive(Debug, Clone)]
pub enum AssertionResult {
    /// Assertion passed
    Passed,
    /// Assertion failed with reason
    Failed(String),
}

/// Legacy context type - deprecated, use SharedAppState instead
///
/// This is kept for backwards compatibility with existing code.
/// New code should use `SharedAppState` directly.
#[deprecated(since = "0.2.0", note = "Use SharedAppState instead")]
pub struct AssertionContext {
    pub is_recording: bool,
    pub is_processing: bool,
    pub is_idle: bool,
    pub audio_buffer_samples: usize,
    pub stt_phase: Option<String>,
    pub stt_speech_chunks: u64,
    pub stt_has_transcription: bool,
    pub stt_has_first_word: bool,
    pub stt_last_transcription: Option<String>,
}

#[allow(deprecated)]
impl Default for AssertionContext {
    fn default() -> Self {
        Self {
            is_recording: false,
            is_processing: false,
            is_idle: true,
            audio_buffer_samples: 0,
            stt_phase: None,
            stt_speech_chunks: 0,
            stt_has_transcription: false,
            stt_has_first_word: false,
            stt_last_transcription: None,
        }
    }
}

#[allow(deprecated)]
impl AssertionContext {
    /// Create an AssertionContext from SharedAppState
    pub fn from_shared_state(state: &SharedAppState) -> Self {
        let s = state.read();
        Self {
            is_recording: s.recording.is_recording(),
            is_processing: s.recording.is_processing(),
            is_idle: s.recording.is_idle() && s.llm.is_idle(),
            audio_buffer_samples: s.audio_buffer_samples,
            stt_phase: None,      // Not tracked in unified state
            stt_speech_chunks: 0, // Not tracked in unified state
            stt_has_transcription: s.transcription.last_text.is_some(),
            stt_has_first_word: s.transcription.has_first_word,
            stt_last_transcription: s.transcription.last_text.clone(),
        }
    }

    /// Create an AssertionContext from AppState reference
    pub fn from_app_state(s: &AppState) -> Self {
        Self {
            is_recording: s.recording.is_recording(),
            is_processing: s.recording.is_processing(),
            is_idle: s.recording.is_idle() && s.llm.is_idle(),
            audio_buffer_samples: s.audio_buffer_samples,
            stt_phase: None,
            stt_speech_chunks: 0,
            stt_has_transcription: s.transcription.last_text.is_some(),
            stt_has_first_word: s.transcription.has_first_word,
            stt_last_transcription: s.transcription.last_text.clone(),
        }
    }
}

/// Test runner that schedules and executes test actions
pub struct TestRunner {
    config: TestConfig,
    start_time: Option<Instant>,
    current_action_index: usize,
    completed: bool,
    test_passed: bool,
}

impl TestRunner {
    /// Create a new test runner from a configuration
    pub fn new(config: TestConfig) -> Self {
        info!("[TEST] Loaded test configuration: {}", config.test.name);
        if !config.test.description.is_empty() {
            info!("[TEST] Description: {}", config.test.description);
        }
        info!("[TEST] Total actions: {}", config.actions.len());

        Self {
            config,
            start_time: None,
            current_action_index: 0,
            completed: false,
            test_passed: true,
        }
    }

    /// Start the test (call this on the first frame)
    pub fn start(&mut self) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
            info!("[TEST] Test started: {}", self.config.test.name);
        }
    }

    /// Check if the test is complete
    pub fn is_completed(&self) -> bool {
        self.completed
    }

    /// Check if the test passed (only valid after completion)
    pub fn test_passed(&self) -> bool {
        self.test_passed
    }

    /// Get the current elapsed time since test start
    pub fn elapsed(&self) -> Duration {
        self.start_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Poll for the next command to execute
    ///
    /// Returns Some(command) if there's an action to execute now,
    /// or None if we should wait.
    pub fn poll(&mut self) -> Option<(TestCommand, Option<Assertion>)> {
        if self.completed {
            return None;
        }

        let start_time = self.start_time?;
        let elapsed = start_time.elapsed();

        // Check if it's time for the next action
        if self.current_action_index >= self.config.actions.len() {
            self.completed = true;
            return None;
        }

        let action = &self.config.actions[self.current_action_index];

        if elapsed >= action.delay() {
            let command = self.action_to_command(&action.action);
            let assertion = action.assert.clone();

            debug!(
                "[TEST] Executing action at {}ms: {:?}",
                action.time_ms, action.action
            );

            self.current_action_index += 1;

            // Check if this was the last action
            if self.current_action_index >= self.config.actions.len() {
                self.completed = true;
            }

            Some((command, assertion))
        } else {
            None
        }
    }

    /// Convert an ActionType to a TestCommand
    fn action_to_command(&self, action: &ActionType) -> TestCommand {
        match action {
            ActionType::ClickRecord => TestCommand::ClickRecord,
            ActionType::StopRecord => TestCommand::StopRecord,
            ActionType::CancelRecord => TestCommand::CancelRecord,
            ActionType::Exit { code } => TestCommand::Exit { code: *code },
            ActionType::Log { message } => {
                info!("[TEST] Log: {}", message);
                // Log actions don't produce a command that affects UI,
                // but we return Exit with special code to indicate no-op
                TestCommand::Exit { code: -999 } // Sentinel value, won't be used
            }
            ActionType::SendText { text } => TestCommand::SendText { text: text.clone() },
            ActionType::StopGeneration => TestCommand::StopGeneration,
            ActionType::Snapshot { name } => TestCommand::Snapshot { name: name.clone() },
            ActionType::ReportSuccess => TestCommand::ReportSuccess,
            ActionType::ReportFailure { reason } => TestCommand::ReportFailure {
                reason: reason.clone(),
            },
        }
    }

    /// Check an assertion against the shared application state
    ///
    /// This is the preferred method for checking assertions.
    pub fn check_assertion_with_state(
        &mut self,
        assertion: &Assertion,
        state: &SharedAppState,
    ) -> AssertionResult {
        let s = state.read();
        self.check_assertion_impl(assertion, &s)
    }

    /// Check an assertion against an AppState reference
    pub fn check_assertion_with_app_state(
        &mut self,
        assertion: &Assertion,
        state: &AppState,
    ) -> AssertionResult {
        self.check_assertion_impl(assertion, state)
    }

    /// Check an assertion against the legacy AssertionContext
    ///
    /// This method is deprecated. Use `check_assertion_with_state` instead.
    #[allow(deprecated)]
    pub fn check_assertion(
        &mut self,
        assertion: &Assertion,
        context: &AssertionContext,
    ) -> AssertionResult {
        let result = match assertion {
            Assertion::IsRecording => {
                if context.is_recording {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed("Expected state to be Recording".to_string())
                }
            }
            Assertion::IsIdle => {
                if context.is_idle {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed("Expected state to be Idle".to_string())
                }
            }
            Assertion::IsProcessing => {
                if context.is_processing {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed("Expected state to be Processing".to_string())
                }
            }
            Assertion::AudioBufferMinSamples { min_samples } => {
                if context.audio_buffer_samples >= *min_samples {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(format!(
                        "Expected at least {} audio samples, got {}",
                        min_samples, context.audio_buffer_samples
                    ))
                }
            }
            Assertion::AudioBufferNotEmpty => {
                if context.audio_buffer_samples > 0 {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed("Expected audio buffer to not be empty".to_string())
                }
            }
            Assertion::SttPhase { phase } => {
                if let Some(ref current_phase) = context.stt_phase {
                    if current_phase.to_lowercase() == phase.to_lowercase() {
                        AssertionResult::Passed
                    } else {
                        AssertionResult::Failed(format!(
                            "Expected STT phase '{}', got '{}'",
                            phase, current_phase
                        ))
                    }
                } else {
                    AssertionResult::Failed(format!(
                        "Expected STT phase '{}', but STT is not initialized",
                        phase
                    ))
                }
            }
            Assertion::SttSpeechChunksMin { min_chunks } => {
                if context.stt_speech_chunks >= *min_chunks {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(format!(
                        "Expected at least {} speech chunks, got {}",
                        min_chunks, context.stt_speech_chunks
                    ))
                }
            }
            Assertion::SttHasTranscription => {
                if context.stt_has_transcription {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(
                        "Expected transcription result, none received".to_string(),
                    )
                }
            }
            Assertion::SttHasFirstWord => {
                if context.stt_has_first_word {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(
                        "Expected first word detection, none received".to_string(),
                    )
                }
            }
            Assertion::SttTranscriptionContains { text } => {
                if let Some(ref transcription) = context.stt_last_transcription {
                    if transcription.to_lowercase().contains(&text.to_lowercase()) {
                        AssertionResult::Passed
                    } else {
                        AssertionResult::Failed(format!(
                            "Expected transcription to contain '{}', got '{}'",
                            text, transcription
                        ))
                    }
                } else {
                    AssertionResult::Failed(format!(
                        "Expected transcription containing '{}', but no transcription received",
                        text
                    ))
                }
            }
        };

        self.log_assertion_result(assertion, &result);
        result
    }

    /// Internal implementation for checking assertions against AppState
    fn check_assertion_impl(&mut self, assertion: &Assertion, state: &AppState) -> AssertionResult {
        let result = match assertion {
            Assertion::IsRecording => {
                if state.recording.is_recording() {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(format!(
                        "Expected state to be Recording, got {:?}",
                        state.recording
                    ))
                }
            }
            Assertion::IsIdle => {
                if state.is_idle() {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(format!(
                        "Expected state to be Idle, got recording={:?}, llm={:?}",
                        state.recording, state.llm
                    ))
                }
            }
            Assertion::IsProcessing => {
                if state.recording.is_processing() {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(format!(
                        "Expected state to be Processing, got {:?}",
                        state.recording
                    ))
                }
            }
            Assertion::AudioBufferMinSamples { min_samples } => {
                if state.audio_buffer_samples >= *min_samples {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(format!(
                        "Expected at least {} audio samples, got {}",
                        min_samples, state.audio_buffer_samples
                    ))
                }
            }
            Assertion::AudioBufferNotEmpty => {
                if state.audio_buffer_samples > 0 {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed("Expected audio buffer to not be empty".to_string())
                }
            }
            Assertion::SttPhase { phase } => {
                // STT phase is not tracked in unified state yet
                // This assertion will need STT-specific integration
                AssertionResult::Failed(format!(
                    "STT phase assertion '{}' not yet supported with unified state",
                    phase
                ))
            }
            Assertion::SttSpeechChunksMin { min_chunks } => {
                // Speech chunks not tracked in unified state yet
                AssertionResult::Failed(format!(
                    "STT speech chunks assertion (min={}) not yet supported with unified state",
                    min_chunks
                ))
            }
            Assertion::SttHasTranscription => {
                if state.transcription.last_text.is_some() {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(
                        "Expected transcription result, none received".to_string(),
                    )
                }
            }
            Assertion::SttHasFirstWord => {
                if state.transcription.has_first_word {
                    AssertionResult::Passed
                } else {
                    AssertionResult::Failed(
                        "Expected first word detection, none received".to_string(),
                    )
                }
            }
            Assertion::SttTranscriptionContains { text } => {
                if let Some(ref transcription) = state.transcription.last_text {
                    if transcription.to_lowercase().contains(&text.to_lowercase()) {
                        AssertionResult::Passed
                    } else {
                        AssertionResult::Failed(format!(
                            "Expected transcription to contain '{}', got '{}'",
                            text, transcription
                        ))
                    }
                } else {
                    AssertionResult::Failed(format!(
                        "Expected transcription containing '{}', but no transcription received",
                        text
                    ))
                }
            }
        };

        self.log_assertion_result(assertion, &result);
        result
    }

    /// Log the result of an assertion check
    fn log_assertion_result(&mut self, assertion: &Assertion, result: &AssertionResult) {
        match result {
            AssertionResult::Passed => {
                info!("[TEST] PASS: Assertion {:?}", assertion);
            }
            AssertionResult::Failed(reason) => {
                error!("[TEST] FAIL: Assertion {:?} - {}", assertion, reason);
                self.test_passed = false;
            }
        }
    }

    /// Get a summary of the test result
    pub fn summary(&self) -> String {
        let status = if self.test_passed { "PASSED" } else { "FAILED" };
        format!(
            "[TEST] Test '{}' {}: Executed {} actions in {:?}",
            self.config.test.name,
            status,
            self.current_action_index,
            self.elapsed()
        )
    }
}
