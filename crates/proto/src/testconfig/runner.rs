//! Test runner for executing test configurations
//!
//! This module provides the TestRunner that schedules and executes
//! test actions at their specified times.

use super::{ActionType, Assertion, TestConfig};
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Commands that the test runner can send to the UI
#[derive(Debug, Clone)]
pub enum TestCommand {
    /// Click the record button
    ClickRecord,
    /// Stop recording
    StopRecord,
    /// Cancel recording
    CancelRecord,
    /// Exit the application
    Exit { code: i32 },
}

/// Result of an assertion check
#[derive(Debug, Clone)]
pub enum AssertionResult {
    /// Assertion passed
    Passed,
    /// Assertion failed with reason
    Failed(String),
}

/// Context for assertion checking - passed from UI to runner
pub struct AssertionContext {
    pub is_recording: bool,
    pub is_processing: bool,
    pub is_idle: bool,
    pub audio_buffer_samples: usize,
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
                // Actually, let's handle this differently - we'll process it inline
                TestCommand::Exit { code: -999 } // Sentinel value, won't be used
            }
        }
    }

    /// Check an assertion against the current context
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
        };

        match &result {
            AssertionResult::Passed => {
                info!("[TEST] PASS: Assertion {:?}", assertion);
            }
            AssertionResult::Failed(reason) => {
                error!("[TEST] FAIL: Assertion {:?} - {}", assertion, reason);
                self.test_passed = false;
            }
        }

        result
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
