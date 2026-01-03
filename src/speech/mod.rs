//! Speech processing modules for STT and TTS
//!
//! This module provides:
//! - Speech-to-text (STT) using Whisper
//! - Text-to-speech (TTS) using Piper

pub mod stt;
pub mod tts;

// Re-export commonly used types
pub use tts::{
    normalize_text_for_tts, AudioQueue, TTSAudio, TTSCommand, TTSConfig, TTSEngine, TTSEvent,
    TTSPipeline, VITS_SAMPLE_RATE,
};
