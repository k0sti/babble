//! LLM integration with mistral.rs
//!
//! This module provides the conversational AI capabilities for Babble,
//! including streaming inference, TTS marker parsing, and conversation
//! context management.
//!
//! # Architecture
//!
//! The LLM module is organized into several components:
//!
//! - **config**: Configuration for model loading and inference parameters
//! - **context**: Conversation history and context window management
//! - **inference**: The LLM engine wrapper around mistral.rs
//! - **pipeline**: Channel-based async pipeline for inference requests
//! - **prompts**: System prompts and TTS marker definitions
//! - **tts_parser**: Streaming parser for extracting TTS segments
//!
//! # Usage
//!
//! ```rust,ignore
//! use babble::llm::{LLMConfig, LLMPipeline, LLMCommand, LLMEvent};
//! use uuid::Uuid;
//!
//! // Create and start the pipeline
//! let config = LLMConfig::default();
//! let pipeline = LLMPipeline::new(config);
//! let cmd_tx = pipeline.command_sender();
//! let event_rx = pipeline.event_receiver();
//! pipeline.start_worker()?;
//!
//! // Send a generation request
//! cmd_tx.send(LLMCommand::Generate {
//!     user_message: "Hello!".to_string(),
//!     request_id: Uuid::new_v4(),
//! })?;
//!
//! // Receive events
//! while let Ok(event) = event_rx.recv() {
//!     match event {
//!         LLMEvent::Token { token, .. } => print!("{}", token),
//!         LLMEvent::TTSSegment { segment, .. } => {
//!             if segment.should_speak {
//!                 // Send to TTS engine
//!             }
//!         }
//!         LLMEvent::Complete { .. } => break,
//!         _ => {}
//!     }
//! }
//! ```

pub mod config;
pub mod context;
pub mod inference;
pub mod pipeline;
pub mod prompts;
pub mod tts_parser;

// Re-export commonly used types
pub use config::{LLMConfig, QuantizationType};
pub use context::{ConversationContext, ConversationMessage, MessageRole};
pub use inference::{LLMEngine, SyncLLMEngine, TokenCallback};
pub use pipeline::{generate_once, LLMCommand, LLMEvent, LLMPipeline, LLMPipelineBuilder};
pub use prompts::{build_system_prompt, markers, COMPACT_SYSTEM_PROMPT, SYSTEM_PROMPT};
pub use tts_parser::{parse_response, TTSParser, TTSSegment};
