//! Processing modules for proto application
//!
//! This module contains the processing pipeline components:
//! - LLM inference with streaming support
//! - Speech-to-text transcription (TODO)
//! - Audio preprocessing (TODO)

pub mod llm;

// Re-export commonly used types
pub use llm::{
    ConversationContext, LLMCommand, LLMConfig, LLMEvent, LLMHandle, LLMRunner, Message,
    MessageRole,
};
