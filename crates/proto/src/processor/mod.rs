//! Processing modules for proto application
//!
//! This module contains the processing pipeline components:
//! - LLM inference with streaming support
//! - Speech-to-text transcription with first-word detection
//! - Message handler with command detection

mod handler;
pub mod llm;
mod stt;

// Re-export commonly used types
pub use handler::{
    MessageCommand, MessageHandler, MessageHandlerCommand, MessageHandlerEvent,
    MessageHandlerWorker,
};
pub use llm::{
    ConversationContext, LLMCommand, LLMConfig, LLMEvent, LLMHandle, LLMRunner, Message,
    MessageRole,
};
pub use stt::{ProcessingPhase, STTCommand, STTConfig, STTEvent, STTProcessor, STTWorker};
