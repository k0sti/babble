//! LLM inference engine using mistral.rs
//!
//! Provides both streaming and non-streaming inference capabilities.

use crate::llm::config::{LLMConfig, QuantizationType};
use crate::llm::context::{ConversationMessage, MessageRole};
use crate::{BabbleError, Result};
use futures::StreamExt;
use mistralrs::{
    IsqType, PagedAttentionMetaBuilder, RequestMessage, Response, TextMessageRole, TextMessages,
    TextModelBuilder,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Callback type for streaming token reception
pub type TokenCallback = Box<dyn FnMut(&str) -> bool + Send>;

/// LLM inference engine wrapping mistral.rs
pub struct LLMEngine {
    /// Model configuration
    config: LLMConfig,

    /// The mistral.rs model runner
    model: Arc<mistralrs::Model>,
}

impl LLMEngine {
    /// Create a new LLM engine with the specified configuration
    pub async fn new(config: LLMConfig) -> Result<Self> {
        info!("Initializing LLM engine with model: {}", config.model_id);

        let isq_type = match config.quantization {
            QuantizationType::None => None,
            QuantizationType::Q4K => Some(IsqType::Q4K),
            QuantizationType::Q8_0 => Some(IsqType::Q8_0),
            QuantizationType::Q4_0 => Some(IsqType::Q4_0),
        };

        let mut builder = TextModelBuilder::new(&config.model_id);

        // Apply quantization if specified
        if let Some(isq) = isq_type {
            builder = builder.with_isq(isq);
        }

        // Enable logging if configured
        if config.enable_logging {
            builder = builder.with_logging();
        }

        // Configure paged attention for efficient memory usage
        builder = builder
            .with_paged_attn(|| {
                PagedAttentionMetaBuilder::default()
                    .with_block_size(32)
                    .build()
            })
            .map_err(|e| {
                BabbleError::InferenceError(format!("Failed to configure paged attention: {}", e))
            })?;

        // Build the model
        let model = builder
            .build()
            .await
            .map_err(|e| BabbleError::ModelLoadError(format!("Failed to load LLM model: {}", e)))?;

        info!("LLM engine initialized successfully");

        Ok(Self {
            config,
            model: Arc::new(model),
        })
    }

    /// Generate a response (non-streaming)
    pub async fn generate(&self, messages: &[ConversationMessage]) -> Result<String> {
        let text_messages = self.build_messages(messages)?;

        let response = self
            .model
            .send_chat_request(text_messages)
            .await
            .map_err(|e| BabbleError::InferenceError(format!("Chat request failed: {}", e)))?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        debug!(
            "Generated response: {} tokens @ {:.1} tok/s",
            response.usage.completion_tokens, response.usage.avg_compl_tok_per_sec
        );

        Ok(content)
    }

    /// Generate a streaming response with token callback
    ///
    /// The callback receives each token as it's generated.
    /// Return `false` from the callback to stop generation early.
    pub async fn generate_stream(
        &self,
        messages: &[ConversationMessage],
        mut on_token: TokenCallback,
    ) -> Result<String> {
        let text_messages = self.build_messages(messages)?;

        // Create a channel for streaming responses
        let (tx, mut rx) = mpsc::channel::<Response>(100);

        let model = self.model.clone();

        // Spawn the streaming request
        let handle = tokio::spawn(async move {
            match model.stream_chat_request(text_messages).await {
                Ok(mut stream) => {
                    while let Some(response) = stream.next().await {
                        if tx.send(response).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Streaming request failed: {}", e);
                }
            }
        });

        // Collect tokens
        let mut full_response = String::new();
        let mut should_continue = true;

        while let Some(response) = rx.recv().await {
            if !should_continue {
                break;
            }

            // Extract delta content from streaming response
            if let Some(choice) = response.choices.first() {
                if let Some(delta) = &choice.delta {
                    if let Some(content) = &delta.content {
                        full_response.push_str(content);

                        // Call the callback with the new token
                        should_continue = on_token(content);
                    }
                }
            }
        }

        // Wait for the streaming task to complete
        let _ = handle.await;

        Ok(full_response)
    }

    /// Generate with a simple sender channel for tokens
    pub async fn generate_to_channel(
        &self,
        messages: &[ConversationMessage],
        token_tx: mpsc::Sender<String>,
    ) -> Result<String> {
        self.generate_stream(
            messages,
            Box::new(move |token| token_tx.blocking_send(token.to_string()).is_ok()),
        )
        .await
    }

    /// Build mistral.rs TextMessages from ConversationMessages
    fn build_messages(&self, messages: &[ConversationMessage]) -> Result<TextMessages> {
        let mut text_messages = TextMessages::new();

        for msg in messages {
            let role = match msg.role {
                MessageRole::System => TextMessageRole::System,
                MessageRole::User => TextMessageRole::User,
                MessageRole::Assistant => TextMessageRole::Assistant,
            };

            text_messages = text_messages.add_message(role, &msg.content);
        }

        Ok(text_messages)
    }

    /// Get the model configuration
    pub fn config(&self) -> &LLMConfig {
        &self.config
    }

    /// Get the model ID
    pub fn model_id(&self) -> &str {
        &self.config.model_id
    }
}

/// Simple synchronous wrapper for testing
pub struct SyncLLMEngine {
    runtime: tokio::runtime::Runtime,
    engine: LLMEngine,
}

impl SyncLLMEngine {
    /// Create a new synchronous LLM engine
    pub fn new(config: LLMConfig) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| BabbleError::InferenceError(format!("Failed to create runtime: {}", e)))?;

        let engine = runtime.block_on(LLMEngine::new(config))?;

        Ok(Self { runtime, engine })
    }

    /// Generate a response synchronously
    pub fn generate(&self, messages: &[ConversationMessage]) -> Result<String> {
        self.runtime.block_on(self.engine.generate(messages))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = LLMConfig::default();
        assert!(!config.model_id.is_empty());
        assert!(config.temperature > 0.0);
        assert!(config.max_tokens > 0);
    }

    #[test]
    fn test_message_role_conversion() {
        assert_eq!(MessageRole::System.as_str(), "system");
        assert_eq!(MessageRole::User.as_str(), "user");
        assert_eq!(MessageRole::Assistant.as_str(), "assistant");
    }

    // Integration tests would require model loading
    // These are marked as ignored by default
    #[tokio::test]
    #[ignore]
    async fn test_engine_creation() {
        let config = LLMConfig::default();
        let engine = LLMEngine::new(config).await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_simple_generation() {
        let config = LLMConfig::default();
        let engine = LLMEngine::new(config).await.unwrap();

        let messages = vec![
            ConversationMessage::system("You are a helpful assistant."),
            ConversationMessage::user("Say hello in one word."),
        ];

        let response = engine.generate(&messages).await;
        assert!(response.is_ok());
        assert!(!response.unwrap().is_empty());
    }
}
