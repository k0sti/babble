pub mod audio;
pub mod llm;
pub mod messages;
pub mod speech;
pub mod ui;
pub mod utils;

use thiserror::Error;

#[derive(Error, Debug)]
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
    IOError(#[from] std::io::Error),

    #[error("Audio processing error: {0}")]
    AudioProcessingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

pub type Result<T> = std::result::Result<T, BabbleError>;
