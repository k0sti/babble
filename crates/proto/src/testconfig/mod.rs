//! Test configuration module for automated UI testing
//!
//! This module provides functionality to run predefined test scenarios
//! by loading TOML configuration files that specify timed actions.

mod runner;

pub use runner::{AssertionContext, AssertionResult, TestCommand, TestRunner};

use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// A test configuration loaded from a TOML file
#[derive(Debug, Clone, Deserialize)]
pub struct TestConfig {
    /// Test metadata
    pub test: TestMetadata,
    /// List of timed actions to execute
    pub actions: Vec<TestAction>,
}

/// Metadata about the test
#[derive(Debug, Clone, Deserialize)]
pub struct TestMetadata {
    /// Name of the test
    pub name: String,
    /// Description of what the test validates
    #[serde(default)]
    pub description: String,
}

/// A single test action with timing
#[derive(Debug, Clone, Deserialize)]
pub struct TestAction {
    /// Time in milliseconds after test start to execute this action
    pub time_ms: u64,
    /// The action to perform
    pub action: ActionType,
    /// Optional assertion to validate after the action
    #[serde(default)]
    pub assert: Option<Assertion>,
}

/// Types of actions that can be performed during a test
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionType {
    /// Click the record button (toggle recording)
    ClickRecord,
    /// Stop recording
    StopRecord,
    /// Cancel recording
    CancelRecord,
    /// Exit the application
    Exit {
        /// Exit code (0 for success, non-zero for failure)
        #[serde(default)]
        code: i32,
    },
    /// Log a message for debugging
    Log {
        /// Message to log
        message: String,
    },
}

/// Assertions to validate test conditions
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Assertion {
    /// Assert that we are currently recording
    IsRecording,
    /// Assert that we are not recording (idle)
    IsIdle,
    /// Assert that we are processing
    IsProcessing,
    /// Assert that the audio buffer contains at least N samples
    AudioBufferMinSamples {
        /// Minimum number of samples expected
        min_samples: usize,
    },
    /// Assert that the audio buffer is not empty
    AudioBufferNotEmpty,
    /// Assert that the STT processor is in a specific phase
    SttPhase {
        /// Expected phase: "idle", "recording", "silence_detected", "transcribing", "detecting_first_word"
        phase: String,
    },
    /// Assert that at least N speech chunks have been detected
    SttSpeechChunksMin {
        /// Minimum number of speech chunks
        min_chunks: u64,
    },
    /// Assert that a transcription result was received
    SttHasTranscription,
    /// Assert that a first word was detected
    SttHasFirstWord,
    /// Assert that transcription text contains a substring
    SttTranscriptionContains {
        /// Substring to search for (case-insensitive)
        text: String,
    },
}

impl TestConfig {
    /// Load a test configuration from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, TestConfigError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| TestConfigError::IoError {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

        let config: TestConfig =
            toml::from_str(&content).map_err(|e| TestConfigError::ParseError {
                path: path.display().to_string(),
                error: e.to_string(),
            })?;

        config.validate()?;
        Ok(config)
    }

    /// Validate the test configuration
    fn validate(&self) -> Result<(), TestConfigError> {
        if self.actions.is_empty() {
            return Err(TestConfigError::ValidationError(
                "Test configuration must have at least one action".to_string(),
            ));
        }

        // Check that actions are sorted by time
        let mut last_time = 0;
        for action in &self.actions {
            if action.time_ms < last_time {
                return Err(TestConfigError::ValidationError(format!(
                    "Actions must be ordered by time. Found action at {}ms after action at {}ms",
                    action.time_ms, last_time
                )));
            }
            last_time = action.time_ms;
        }

        // Check that there's an exit action
        let has_exit = self
            .actions
            .iter()
            .any(|a| matches!(a.action, ActionType::Exit { .. }));
        if !has_exit {
            return Err(TestConfigError::ValidationError(
                "Test configuration must have an Exit action".to_string(),
            ));
        }

        Ok(())
    }
}

impl TestAction {
    /// Get the duration from test start for this action
    pub fn delay(&self) -> Duration {
        Duration::from_millis(self.time_ms)
    }
}

/// Errors that can occur when loading or validating test configurations
#[derive(Debug, Clone)]
pub enum TestConfigError {
    /// IO error reading the file
    IoError { path: String, error: String },
    /// Error parsing the TOML
    ParseError { path: String, error: String },
    /// Validation error in the configuration
    ValidationError(String),
}

impl std::fmt::Display for TestConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestConfigError::IoError { path, error } => {
                write!(f, "Failed to read test config '{}': {}", path, error)
            }
            TestConfigError::ParseError { path, error } => {
                write!(f, "Failed to parse test config '{}': {}", path, error)
            }
            TestConfigError::ValidationError(msg) => {
                write!(f, "Invalid test config: {}", msg)
            }
        }
    }
}

impl std::error::Error for TestConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action_type() {
        let toml_str = r#"
            [test]
            name = "Basic recording test"

            [[actions]]
            time_ms = 500
            action = { type = "click_record" }

            [[actions]]
            time_ms = 2500
            action = { type = "stop_record" }

            [[actions]]
            time_ms = 3000
            action = { type = "exit", code = 0 }
        "#;

        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.test.name, "Basic recording test");
        assert_eq!(config.actions.len(), 3);
        assert!(matches!(config.actions[0].action, ActionType::ClickRecord));
        assert!(matches!(config.actions[1].action, ActionType::StopRecord));
        assert!(matches!(
            config.actions[2].action,
            ActionType::Exit { code: 0 }
        ));
    }

    #[test]
    fn test_parse_with_assertions() {
        let toml_str = r#"
            [test]
            name = "Test with assertions"

            [[actions]]
            time_ms = 500
            action = { type = "click_record" }
            assert = { type = "is_recording" }

            [[actions]]
            time_ms = 1000
            action = { type = "exit", code = 0 }
        "#;

        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert!(matches!(
            config.actions[0].assert,
            Some(Assertion::IsRecording)
        ));
    }

    #[test]
    fn test_parse_audio_buffer_assertion() {
        let toml_str = r#"
            [test]
            name = "Audio buffer test"

            [[actions]]
            time_ms = 2500
            action = { type = "log", message = "Checking buffer" }
            assert = { type = "audio_buffer_min_samples", min_samples = 48000 }

            [[actions]]
            time_ms = 3000
            action = { type = "exit", code = 0 }
        "#;

        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert!(matches!(
            config.actions[0].assert,
            Some(Assertion::AudioBufferMinSamples { min_samples: 48000 })
        ));
    }
}
