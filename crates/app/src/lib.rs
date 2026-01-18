pub mod audio;
pub mod integration;
pub mod llm;
pub mod messages;
pub mod speech;
pub mod ui;
pub mod utils;

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum BabbleError {
    #[error("Audio device error: {0}")]
    AudioDeviceError(String),

    #[error("Model load error: {0}")]
    ModelLoadError(String),

    #[error("Transcription error: {0}")]
    TranscriptionError(String),

    #[error("Inference error: {0}")]
    InferenceError(String),

    #[error("TTS error: {0}")]
    TTSError(String),

    #[error("IO error: {0}")]
    IOError(String),

    #[error("Audio processing error: {0}")]
    AudioProcessingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Pipeline error: {0}")]
    PipelineError(String),

    #[error("Orchestrator error: {0}")]
    OrchestratorError(String),
}

impl From<std::io::Error> for BabbleError {
    fn from(e: std::io::Error) -> Self {
        BabbleError::IOError(e.to_string())
    }
}

impl BabbleError {
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Hardware/device errors may require user intervention
            BabbleError::AudioDeviceError(_) => false,
            // Model errors require restarting
            BabbleError::ModelLoadError(_) => false,
            // These are typically transient errors
            BabbleError::TranscriptionError(_) => true,
            BabbleError::InferenceError(_) => true,
            BabbleError::TTSError(_) => true,
            BabbleError::IOError(_) => false,
            BabbleError::AudioProcessingError(_) => true,
            BabbleError::ConfigError(_) => false,
            BabbleError::ChannelError(_) => false,
            BabbleError::PipelineError(_) => true,
            BabbleError::OrchestratorError(_) => true,
        }
    }

    /// Get a user-friendly description
    pub fn user_message(&self) -> String {
        match self {
            BabbleError::AudioDeviceError(_) => {
                "Audio device error. Please check your microphone/speakers.".to_string()
            }
            BabbleError::ModelLoadError(_) => {
                "Failed to load AI model. Please verify model files are present.".to_string()
            }
            BabbleError::TranscriptionError(_) => {
                "Speech recognition failed. Please try again.".to_string()
            }
            BabbleError::InferenceError(_) => {
                "AI response generation failed. Please try again.".to_string()
            }
            BabbleError::TTSError(_) => {
                "Text-to-speech failed. Response will be shown as text.".to_string()
            }
            BabbleError::IOError(_) => {
                "File system error occurred.".to_string()
            }
            BabbleError::AudioProcessingError(_) => {
                "Audio processing failed. Please try again.".to_string()
            }
            BabbleError::ConfigError(_) => {
                "Configuration error. Please check settings.".to_string()
            }
            BabbleError::ChannelError(_) => {
                "Internal communication error. Please restart the application.".to_string()
            }
            BabbleError::PipelineError(_) => {
                "Processing pipeline error. Please try again.".to_string()
            }
            BabbleError::OrchestratorError(_) => {
                "System error occurred. Please try again.".to_string()
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, BabbleError>;
