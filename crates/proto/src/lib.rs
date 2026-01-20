//! Proto - Voice-controlled LLM assistant with real-time speech processing
//!
//! This crate provides a voice-first interface for interacting with LLMs,
//! featuring real-time speech-to-text, intelligent response generation,
//! and text-to-speech output.

pub mod audio;
pub mod error;
pub mod message;
pub mod processor;
pub mod state;
pub mod testconfig;
pub mod ui;

// Re-export error types
pub use error::{ProtoError, Result};

// Re-export audio types
pub use audio::{AudioDeviceInfo, AudioRecorder, AudioRingBuffer};

// Re-export state types
pub use state::{
    AppCommand, AppEvent, AppState, AppStateSnapshot, LLMState, RecordingState, ResponseState,
    SharedAppState, TranscriptionState,
};
