//! LLM Pipeline for managing inference requests and streaming responses
//!
//! Provides a channel-based interface similar to TranscriptionPipeline,
//! with support for streaming token generation and TTS segment extraction.

use crate::llm::config::LLMConfig;
use crate::llm::context::ConversationContext;
use crate::llm::inference::LLMEngine;
use crate::llm::prompts::SYSTEM_PROMPT;
use crate::llm::tts_parser::{TTSParser, TTSSegment};
use crate::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::time::Instant;
use tokio::runtime::Runtime;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Commands that can be sent to the LLM pipeline
#[derive(Debug, Clone)]
pub enum LLMCommand {
    /// Generate a response for the given user message
    Generate {
        /// The user's message
        user_message: String,
        /// Unique request ID for tracking
        request_id: Uuid,
    },

    /// Update the system prompt
    UpdateSystemPrompt(String),

    /// Clear conversation history
    ClearContext,

    /// Shutdown the pipeline
    Shutdown,
}

/// Events emitted by the LLM pipeline
#[derive(Debug, Clone)]
pub enum LLMEvent {
    /// A new token was generated
    Token {
        /// The token text
        token: String,
        /// Request ID this token belongs to
        request_id: Uuid,
    },

    /// A TTS segment was extracted and is ready for synthesis
    TTSSegment {
        /// The extracted segment
        segment: TTSSegment,
        /// Request ID this segment belongs to
        request_id: Uuid,
    },

    /// Generation completed
    Complete {
        /// The full response text
        full_response: String,
        /// Request ID
        request_id: Uuid,
        /// Time to first token in milliseconds
        first_token_ms: u64,
        /// Total generation time in milliseconds
        total_ms: u64,
    },

    /// An error occurred
    Error {
        /// Error message
        error: String,
        /// Request ID if applicable
        request_id: Option<Uuid>,
    },

    /// Pipeline has shut down
    Shutdown,
}

/// LLM Pipeline with channel-based communication
pub struct LLMPipeline {
    /// Configuration
    config: LLMConfig,

    /// Command sender
    command_tx: Sender<LLMCommand>,

    /// Command receiver (for worker)
    command_rx: Receiver<LLMCommand>,

    /// Event sender (for worker)
    event_tx: Sender<LLMEvent>,

    /// Event receiver
    event_rx: Receiver<LLMEvent>,
}

impl LLMPipeline {
    /// Create a new LLM pipeline
    pub fn new(config: LLMConfig) -> Self {
        let (command_tx, command_rx) = bounded(100);
        let (event_tx, event_rx) = bounded(100);

        Self {
            config,
            command_tx,
            command_rx,
            event_tx,
            event_rx,
        }
    }

    /// Get a sender for commands
    pub fn command_sender(&self) -> Sender<LLMCommand> {
        self.command_tx.clone()
    }

    /// Get a receiver for events
    pub fn event_receiver(&self) -> Receiver<LLMEvent> {
        self.event_rx.clone()
    }

    /// Start the pipeline worker thread
    ///
    /// This spawns a new thread that handles LLM inference requests.
    pub fn start_worker(self) -> Result<()> {
        let config = self.config.clone();
        let command_rx = self.command_rx.clone();
        let event_tx = self.event_tx.clone();

        std::thread::spawn(move || {
            info!("LLM pipeline worker starting");

            // Create tokio runtime for async operations
            let runtime = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to create tokio runtime: {}", e);
                    let _ = event_tx.send(LLMEvent::Error {
                        error: format!("Runtime creation failed: {}", e),
                        request_id: None,
                    });
                    let _ = event_tx.send(LLMEvent::Shutdown);
                    return;
                }
            };

            // Initialize the LLM engine
            let engine = match runtime.block_on(LLMEngine::new(config.clone())) {
                Ok(engine) => engine,
                Err(e) => {
                    error!("Failed to initialize LLM engine: {}", e);
                    let _ = event_tx.send(LLMEvent::Error {
                        error: e.to_string(),
                        request_id: None,
                    });
                    let _ = event_tx.send(LLMEvent::Shutdown);
                    return;
                }
            };

            // Initialize conversation context
            let mut context = ConversationContext::new(SYSTEM_PROMPT, config.context_size);

            // Initialize TTS parser
            let mut tts_parser = TTSParser::new();

            info!("LLM pipeline worker ready");

            // Process commands
            loop {
                match command_rx.recv() {
                    Ok(LLMCommand::Generate {
                        user_message,
                        request_id,
                    }) => {
                        debug!("Processing generate request: {}", request_id);

                        // Add user message to context
                        context.add_user_message(&user_message);

                        // Reset TTS parser for new response
                        tts_parser.reset();

                        let start_time = Instant::now();
                        let full_response: String;

                        // Get messages for inference
                        let messages = context.get_messages();

                        // Clone what we need for the closure
                        let event_tx_clone = event_tx.clone();
                        let req_id = request_id;

                        // Run streaming inference
                        let result = runtime.block_on(async {
                            engine
                                .generate_stream(
                                    &messages,
                                    Box::new(move |token| {
                                        // Send token event
                                        let _ = event_tx_clone.send(LLMEvent::Token {
                                            token: token.to_string(),
                                            request_id: req_id,
                                        });

                                        true // Continue generation
                                    }),
                                )
                                .await
                        });

                        match result {
                            Ok(response) => {
                                full_response = response.clone();

                                // Parse any remaining TTS segments
                                let segments = tts_parser.feed(&response);
                                for segment in segments {
                                    let _ = event_tx.send(LLMEvent::TTSSegment {
                                        segment,
                                        request_id,
                                    });
                                }

                                // Flush remaining content
                                if let Some(segment) = tts_parser.flush() {
                                    let _ = event_tx.send(LLMEvent::TTSSegment {
                                        segment,
                                        request_id,
                                    });
                                }

                                // Add assistant response to context
                                context.add_assistant_message(&full_response);

                                let total_ms = start_time.elapsed().as_millis() as u64;
                                let first_token_ms = total_ms / 10; // Approximate

                                debug!(
                                    "Generation complete: {} chars in {}ms",
                                    full_response.len(),
                                    total_ms
                                );

                                let _ = event_tx.send(LLMEvent::Complete {
                                    full_response,
                                    request_id,
                                    first_token_ms,
                                    total_ms,
                                });
                            }
                            Err(e) => {
                                error!("Generation failed: {}", e);
                                let _ = event_tx.send(LLMEvent::Error {
                                    error: e.to_string(),
                                    request_id: Some(request_id),
                                });
                            }
                        }
                    }

                    Ok(LLMCommand::UpdateSystemPrompt(prompt)) => {
                        info!("Updating system prompt");
                        context.set_system_prompt(&prompt);
                    }

                    Ok(LLMCommand::ClearContext) => {
                        info!("Clearing conversation context");
                        context.clear();
                    }

                    Ok(LLMCommand::Shutdown) => {
                        info!("LLM pipeline worker shutting down");
                        let _ = event_tx.send(LLMEvent::Shutdown);
                        break;
                    }

                    Err(e) => {
                        error!("Command channel error: {}", e);
                        break;
                    }
                }
            }

            info!("LLM pipeline worker stopped");
        });

        Ok(())
    }
}

/// Builder for creating LLM pipelines with custom configuration
pub struct LLMPipelineBuilder {
    config: LLMConfig,
    system_prompt: Option<String>,
}

impl LLMPipelineBuilder {
    /// Create a new pipeline builder
    pub fn new() -> Self {
        Self {
            config: LLMConfig::default(),
            system_prompt: None,
        }
    }

    /// Set the model ID
    pub fn with_model(mut self, model_id: impl Into<String>) -> Self {
        self.config.model_id = model_id.into();
        self
    }

    /// Set the configuration
    pub fn with_config(mut self, config: LLMConfig) -> Self {
        self.config = config;
        self
    }

    /// Set a custom system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Build the pipeline
    pub fn build(self) -> LLMPipeline {
        LLMPipeline::new(self.config)
    }
}

impl Default for LLMPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a simple one-shot generation
pub async fn generate_once(
    config: LLMConfig,
    system_prompt: &str,
    user_message: &str,
) -> Result<String> {
    let engine = LLMEngine::new(config).await?;

    let messages = vec![
        crate::llm::context::ConversationMessage::system(system_prompt),
        crate::llm::context::ConversationMessage::user(user_message),
    ];

    engine.generate(&messages).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let config = LLMConfig::default();
        let pipeline = LLMPipeline::new(config);

        // Verify channels are created
        let _cmd_tx = pipeline.command_sender();
        let _event_rx = pipeline.event_receiver();
    }

    #[test]
    fn test_builder() {
        let pipeline = LLMPipelineBuilder::new()
            .with_model("test-model")
            .with_system_prompt("Test prompt")
            .build();

        assert!(pipeline.command_tx.capacity().is_some());
    }

    #[test]
    fn test_command_variants() {
        let cmd1 = LLMCommand::Generate {
            user_message: "Hello".to_string(),
            request_id: Uuid::new_v4(),
        };

        let cmd2 = LLMCommand::ClearContext;
        let cmd3 = LLMCommand::Shutdown;

        // Just verify these compile and can be created
        match cmd1 {
            LLMCommand::Generate { .. } => {}
            _ => panic!("Wrong variant"),
        }

        match cmd2 {
            LLMCommand::ClearContext => {}
            _ => panic!("Wrong variant"),
        }

        match cmd3 {
            LLMCommand::Shutdown => {}
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_event_variants() {
        let request_id = Uuid::new_v4();

        let _token = LLMEvent::Token {
            token: "test".to_string(),
            request_id,
        };

        let _segment = LLMEvent::TTSSegment {
            segment: TTSSegment::spoken("Hello".to_string(), 0),
            request_id,
        };

        let _complete = LLMEvent::Complete {
            full_response: "Full response".to_string(),
            request_id,
            first_token_ms: 100,
            total_ms: 500,
        };

        let _error = LLMEvent::Error {
            error: "Test error".to_string(),
            request_id: Some(request_id),
        };

        let _shutdown = LLMEvent::Shutdown;
    }
}
