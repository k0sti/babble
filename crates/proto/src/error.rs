//! Error types for the Proto application
//!
//! This module defines custom error types following the pattern from the main babble crate.

use thiserror::Error;

/// Proto application errors
#[derive(Error, Debug, Clone)]
pub enum ProtoError {
    /// Audio device initialization or operation error
    #[error("Audio device error: {0}")]
    AudioDeviceError(String),

    /// Speech-to-text transcription error
    #[error("Speech-to-text error: {0}")]
    STTError(String),

    /// LLM inference error
    #[error("LLM error: {0}")]
    LLMError(String),

    /// Channel communication error
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// File system I/O error
    #[error("IO error: {0}")]
    IOError(String),

    /// Audio processing error
    #[error("Audio processing error: {0}")]
    AudioProcessingError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl From<std::io::Error> for ProtoError {
    fn from(e: std::io::Error) -> Self {
        ProtoError::IOError(e.to_string())
    }
}

impl ProtoError {
    /// Check if this error is recoverable
    ///
    /// Recoverable errors allow the application to continue running,
    /// while non-recoverable errors may require user intervention or restart.
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Hardware/device errors may require user intervention
            ProtoError::AudioDeviceError(_) => false,
            // STT errors are typically transient
            ProtoError::STTError(_) => true,
            // LLM errors are typically transient
            ProtoError::LLMError(_) => true,
            // Channel errors indicate internal issues
            ProtoError::ChannelError(_) => false,
            // IO errors may require user intervention
            ProtoError::IOError(_) => false,
            // Audio processing errors are typically transient
            ProtoError::AudioProcessingError(_) => true,
            // Config errors require user intervention
            ProtoError::ConfigError(_) => false,
        }
    }

    /// Get a user-friendly description of the error
    ///
    /// Returns a message suitable for display in the UI.
    pub fn user_message(&self) -> String {
        match self {
            ProtoError::AudioDeviceError(_) => {
                "Audio device error. Please check your microphone/speakers.".to_string()
            }
            ProtoError::STTError(_) => {
                "Speech recognition failed. Please try again.".to_string()
            }
            ProtoError::LLMError(_) => {
                "AI response generation failed. Please try again.".to_string()
            }
            ProtoError::ChannelError(_) => {
                "Internal communication error. Please restart the application.".to_string()
            }
            ProtoError::IOError(_) => {
                "File system error occurred.".to_string()
            }
            ProtoError::AudioProcessingError(_) => {
                "Audio processing failed. Please try again.".to_string()
            }
            ProtoError::ConfigError(_) => {
                "Configuration error. Please check settings.".to_string()
            }
        }
    }
}

/// Result type alias for Proto operations
pub type Result<T> = std::result::Result<T, ProtoError>;
