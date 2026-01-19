//! Processing modules for proto application
//!
//! This module contains the processing pipeline components:
//! - LLM inference with streaming support
//! - Speech-to-text transcription with first-word detection
//! - Audio preprocessing (TODO)

pub mod llm;
mod stt;

// Re-export commonly used types
pub use llm::{
    ConversationContext, LLMCommand, LLMConfig, LLMEvent, LLMHandle, LLMRunner, Message,
    MessageRole,
};
pub use stt::{STTCommand, STTConfig, STTEvent, STTProcessor, STTWorker};
