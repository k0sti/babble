//! Processing modules for proto application
//!
//! This module contains the processing pipeline components:
//! - LLM inference with streaming support
//! - Speech-to-text transcription with first-word detection
//! - Message handler with command detection
//! - Orchestrator for coordinating all processors

mod handler;
pub mod llm;
mod orchestrator;
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
pub use orchestrator::{Orchestrator, OrchestratorConfig, OrchestratorHandle};
pub use stt::{ProcessingPhase, STTCommand, STTConfig, STTEvent, STTProcessor, STTWorker};

// Re-export unified state types from the state module for convenience
pub use crate::state::{AppCommand, AppEvent, SharedAppState};
