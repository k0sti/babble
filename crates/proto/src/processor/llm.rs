//! LLM inference runner using mistral.rs
//!
//! Provides streaming text generation with interruption support.

use crate::{ProtoError, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use mistralrs::{
    ChatCompletionChunkResponse, ChunkChoice, Delta, IsqType, PagedAttentionMetaBuilder, Response,
    TextMessageRole, TextMessages, TextModelBuilder,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use tracing::{debug, error, info, warn};

/// Configuration for the LLM engine
#[derive(Clone, Debug)]
pub struct LLMConfig {
    /// Model identifier (HuggingFace model ID or local path)
    pub model_id: String,
    /// Temperature for sampling (0.0 = deterministic, 1.0+ = creative)
    pub temperature: f32,
    /// Maximum tokens to generate per response
    pub max_tokens: usize,
    /// Whether to use quantization (Q4K by default)
    pub use_quantization: bool,
    /// Enable logging of inference details
    pub enable_logging: bool,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            model_id: "microsoft/Phi-3.5-mini-instruct".to_string(),
            temperature: 0.7,
            max_tokens: 2048,
            use_quantization: true,
            enable_logging: false,
        }
    }
}

impl LLMConfig {
    /// Create a new LLM configuration with the specified model
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            ..Default::default()
        }
    }

    /// Set the temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Enable or disable quantization
    pub fn with_quantization(mut self, use_quantization: bool) -> Self {
        self.use_quantization = use_quantization;
        self
    }

    /// Enable or disable inference logging
    pub fn with_logging(mut self, enable: bool) -> Self {
        self.enable_logging = enable;
        self
    }
}

/// Commands sent to the LLM worker
#[derive(Clone, Debug)]
pub enum LLMCommand {
    /// Generate response for input text
    Generate(String),
    /// Stop current generation
    Stop,
    /// Shutdown the LLM worker
    Shutdown,
}

/// Events emitted by the LLM worker
#[derive(Clone, Debug)]
pub enum LLMEvent {
    /// Generation started
    Started,
    /// Token received (streaming)
    Token(String),
    /// Generation complete
    Complete {
        /// Full generated response
        response: String,
        /// Whether generation was interrupted
        interrupted: bool,
    },
    /// Error occurred
    Error(String),
    /// Worker shut down
    Shutdown,
}

/// Role of a message in the conversation
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageRole {
    /// System prompt/instructions
    System,
    /// User input
    User,
    /// Assistant response
    Assistant,
}

/// A single message in the conversation
#[derive(Clone, Debug)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Message content
    pub content: String,
}

impl Message {
    /// Create a new message
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageRole::System, content)
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(MessageRole::Assistant, content)
    }
}

/// Manages conversation context and history
#[derive(Clone, Debug)]
pub struct ConversationContext {
    /// System prompt (always included)
    system_prompt: String,
    /// Conversation history
    messages: Vec<Message>,
}

impl ConversationContext {
    /// Create a new conversation context with a system prompt
    pub fn new(system_prompt: &str) -> Self {
        Self {
            system_prompt: system_prompt.to_string(),
            messages: Vec::new(),
        }
    }

    /// Add a user message to the conversation
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message::user(content));
    }

    /// Add an assistant message to the conversation
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(Message::assistant(content));
    }

    /// Clear conversation history (keeps system prompt)
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get all messages including system prompt
    pub fn messages(&self) -> Vec<Message> {
        let mut result = vec![Message::system(&self.system_prompt)];
        result.extend(self.messages.clone());
        result
    }

    /// Get the system prompt
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Update the system prompt
    pub fn set_system_prompt(&mut self, prompt: &str) {
        self.system_prompt = prompt.to_string();
    }

    /// Get number of messages in history (excluding system prompt)
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

/// Handle for interacting with a running LLM worker
pub struct LLMHandle {
    /// Send commands to the worker
    pub command_tx: Sender<LLMCommand>,
    /// Receive events from the worker
    pub event_rx: Receiver<LLMEvent>,
    /// Thread handle for the worker
    worker_handle: Option<JoinHandle<()>>,
}

impl LLMHandle {
    /// Send a generate command
    pub fn generate(&self, input: &str) -> Result<()> {
        self.command_tx
            .send(LLMCommand::Generate(input.to_string()))
            .map_err(|e| {
                ProtoError::ChannelError(format!("Failed to send generate command: {}", e))
            })
    }

    /// Send a stop command to interrupt generation
    pub fn stop(&self) -> Result<()> {
        self.command_tx
            .send(LLMCommand::Stop)
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send stop command: {}", e)))
    }

    /// Shutdown the worker
    pub fn shutdown(self) -> Result<()> {
        let _ = self.command_tx.send(LLMCommand::Shutdown);
        if let Some(handle) = self.worker_handle {
            handle
                .join()
                .map_err(|_| ProtoError::LLMError("Worker thread panicked".to_string()))?;
        }
        Ok(())
    }

    /// Try to receive an event without blocking
    pub fn try_recv_event(&self) -> Option<LLMEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive an event, blocking until available
    pub fn recv_event(&self) -> Result<LLMEvent> {
        self.event_rx
            .recv()
            .map_err(|e| ProtoError::ChannelError(format!("Failed to receive event: {}", e)))
    }
}

/// LLM Runner that spawns a worker thread for inference
pub struct LLMRunner {
    config: LLMConfig,
}

impl LLMRunner {
    /// Create a new LLM runner with the specified configuration
    pub fn new(config: LLMConfig) -> Self {
        Self { config }
    }

    /// Get a command sender (for use before starting the worker)
    pub fn command_sender(&self) -> Sender<LLMCommand> {
        // This creates a dummy channel; real one is created in start_worker
        let (tx, _rx) = bounded(1);
        tx
    }

    /// Start the LLM worker thread
    ///
    /// Returns a handle for sending commands and receiving events.
    /// The worker runs in a separate thread with its own tokio runtime.
    pub fn start_worker(self) -> Result<LLMHandle> {
        let (command_tx, command_rx) = bounded::<LLMCommand>(100);
        let (event_tx, event_rx) = bounded::<LLMEvent>(100);

        let config = self.config.clone();

        let worker_handle = std::thread::spawn(move || {
            // Create a tokio runtime for async operations
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to create tokio runtime: {}", e);
                    let _ =
                        event_tx.send(LLMEvent::Error(format!("Failed to create runtime: {}", e)));
                    return;
                }
            };

            runtime.block_on(async move {
                worker_loop(config, command_rx, event_tx).await;
            });
        });

        Ok(LLMHandle {
            command_tx,
            event_rx,
            worker_handle: Some(worker_handle),
        })
    }
}

/// Main worker loop that handles commands and performs inference
async fn worker_loop(
    config: LLMConfig,
    command_rx: Receiver<LLMCommand>,
    event_tx: Sender<LLMEvent>,
) {
    info!("LLM worker starting with model: {}", config.model_id);

    // Initialize the model
    let model = match initialize_model(&config).await {
        Ok(m) => Arc::new(m),
        Err(e) => {
            error!("Failed to initialize model: {}", e);
            let _ = event_tx.send(LLMEvent::Error(format!(
                "Failed to initialize model: {}",
                e
            )));
            let _ = event_tx.send(LLMEvent::Shutdown);
            return;
        }
    };

    info!("LLM model loaded successfully");

    // Conversation context - starts with a default system prompt
    let mut context = ConversationContext::new(
        "You are a helpful AI assistant. Respond concisely and accurately.",
    );

    // Flag to signal generation should stop
    let should_stop = Arc::new(AtomicBool::new(false));

    loop {
        // Wait for a command
        let command = match command_rx.recv() {
            Ok(cmd) => cmd,
            Err(_) => {
                info!("Command channel closed, shutting down");
                break;
            }
        };

        match command {
            LLMCommand::Generate(input) => {
                debug!("Received generate command: {}", input);
                should_stop.store(false, Ordering::SeqCst);

                // Add user message to context
                context.add_user_message(&input);

                // Signal generation started
                if event_tx.send(LLMEvent::Started).is_err() {
                    error!("Event channel closed");
                    break;
                }

                // Build messages for the model
                let messages = context.messages();
                let text_messages = build_text_messages(&messages);

                // Perform streaming generation
                let result = generate_streaming(
                    model.clone(),
                    text_messages,
                    event_tx.clone(),
                    command_rx.clone(),
                    should_stop.clone(),
                )
                .await;

                match result {
                    Ok((response, interrupted)) => {
                        if !interrupted {
                            // Add assistant response to context
                            context.add_assistant_message(&response);
                        }

                        if event_tx
                            .send(LLMEvent::Complete {
                                response,
                                interrupted,
                            })
                            .is_err()
                        {
                            error!("Event channel closed");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Generation error: {}", e);
                        if event_tx.send(LLMEvent::Error(e.to_string())).is_err() {
                            error!("Event channel closed");
                            break;
                        }
                    }
                }
            }

            LLMCommand::Stop => {
                debug!("Received stop command");
                should_stop.store(true, Ordering::SeqCst);
            }

            LLMCommand::Shutdown => {
                info!("Received shutdown command");
                break;
            }
        }
    }

    let _ = event_tx.send(LLMEvent::Shutdown);
    info!("LLM worker shutdown complete");
}

/// Initialize the mistral.rs model
async fn initialize_model(config: &LLMConfig) -> Result<mistralrs::Model> {
    let mut builder = TextModelBuilder::new(&config.model_id);

    // Apply quantization if enabled
    if config.use_quantization {
        builder = builder.with_isq(IsqType::Q4K);
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
        .map_err(|e| ProtoError::LLMError(format!("Failed to configure paged attention: {}", e)))?;

    // Build the model
    let model = builder
        .build()
        .await
        .map_err(|e| ProtoError::LLMError(format!("Failed to load model: {}", e)))?;

    Ok(model)
}

/// Build TextMessages from conversation messages
fn build_text_messages(messages: &[Message]) -> TextMessages {
    let mut text_messages = TextMessages::new();

    for msg in messages {
        let role = match msg.role {
            MessageRole::System => TextMessageRole::System,
            MessageRole::User => TextMessageRole::User,
            MessageRole::Assistant => TextMessageRole::Assistant,
        };
        text_messages = text_messages.add_message(role, &msg.content);
    }

    text_messages
}

/// Perform streaming generation with interruption support
async fn generate_streaming(
    model: Arc<mistralrs::Model>,
    messages: TextMessages,
    event_tx: Sender<LLMEvent>,
    command_rx: Receiver<LLMCommand>,
    should_stop: Arc<AtomicBool>,
) -> Result<(String, bool)> {
    // Create a channel for streaming text chunks from the async task
    let (token_tx, mut token_rx) = tokio::sync::mpsc::channel::<String>(100);

    let model_clone = model.clone();

    // Spawn the streaming request in a separate task
    let stream_handle = tokio::spawn(async move {
        match model_clone.stream_chat_request(messages).await {
            Ok(mut stream) => {
                while let Some(response) = stream.next().await {
                    if let Response::Chunk(ChatCompletionChunkResponse { choices, .. }) = response {
                        if let Some(ChunkChoice {
                            delta:
                                Delta {
                                    content: Some(content),
                                    ..
                                },
                            ..
                        }) = choices.first()
                        {
                            if token_tx.send(content.clone()).await.is_err() {
                                // Receiver dropped, stop streaming
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Streaming request failed: {}", e);
            }
        }
    });

    // Collect tokens and check for interruption
    let mut full_response = String::new();
    let mut interrupted = false;

    loop {
        // Check for stop command (non-blocking)
        if let Ok(cmd) = command_rx.try_recv() {
            match cmd {
                LLMCommand::Stop => {
                    warn!("Generation interrupted by stop command");
                    should_stop.store(true, Ordering::SeqCst);
                    interrupted = true;
                    break;
                }
                LLMCommand::Shutdown => {
                    warn!("Generation interrupted by shutdown");
                    interrupted = true;
                    break;
                }
                _ => {
                    // Ignore other commands during generation
                }
            }
        }

        // Check if stop was signaled
        if should_stop.load(Ordering::SeqCst) {
            interrupted = true;
            break;
        }

        // Try to receive the next token with a small timeout
        match tokio::time::timeout(std::time::Duration::from_millis(10), token_rx.recv()).await {
            Ok(Some(token)) => {
                full_response.push_str(&token);

                // Send token event
                if event_tx.send(LLMEvent::Token(token)).is_err() {
                    error!("Event channel closed during streaming");
                    interrupted = true;
                    break;
                }
            }
            Ok(None) => {
                // Stream ended
                break;
            }
            Err(_) => {
                // Timeout - continue loop to check for stop
                continue;
            }
        }
    }

    // Abort the streaming task if interrupted
    if interrupted {
        stream_handle.abort();
    } else {
        // Wait for the stream task to complete
        let _ = stream_handle.await;
    }

    Ok((full_response, interrupted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config_default() {
        let config = LLMConfig::default();
        assert_eq!(config.model_id, "microsoft/Phi-3.5-mini-instruct");
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 2048);
        assert!(config.use_quantization);
    }

    #[test]
    fn test_llm_config_builder() {
        let config = LLMConfig::new("test-model")
            .with_temperature(0.5)
            .with_max_tokens(1024)
            .with_quantization(false);

        assert_eq!(config.model_id, "test-model");
        assert_eq!(config.temperature, 0.5);
        assert_eq!(config.max_tokens, 1024);
        assert!(!config.use_quantization);
    }

    #[test]
    fn test_conversation_context() {
        let mut ctx = ConversationContext::new("You are a test assistant.");

        assert_eq!(ctx.message_count(), 0);
        assert_eq!(ctx.system_prompt(), "You are a test assistant.");

        ctx.add_user_message("Hello");
        assert_eq!(ctx.message_count(), 1);

        ctx.add_assistant_message("Hi there!");
        assert_eq!(ctx.message_count(), 2);

        let messages = ctx.messages();
        assert_eq!(messages.len(), 3); // System + 2 messages
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[1].role, MessageRole::User);
        assert_eq!(messages[2].role, MessageRole::Assistant);
    }

    #[test]
    fn test_conversation_context_clear() {
        let mut ctx = ConversationContext::new("System");
        ctx.add_user_message("Hello");
        ctx.add_assistant_message("Hi");

        ctx.clear();

        assert_eq!(ctx.message_count(), 0);
        // System prompt should still be there
        let messages = ctx.messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, MessageRole::System);
    }

    #[test]
    fn test_message_creation() {
        let sys = Message::system("System message");
        assert_eq!(sys.role, MessageRole::System);
        assert_eq!(sys.content, "System message");

        let user = Message::user("User message");
        assert_eq!(user.role, MessageRole::User);

        let assistant = Message::assistant("Assistant message");
        assert_eq!(assistant.role, MessageRole::Assistant);
    }

    #[test]
    fn test_build_text_messages() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let text_messages = build_text_messages(&messages);
        // TextMessages doesn't expose internals easily, but we can verify it builds
        // The actual test would be in integration tests with the model
        let _ = text_messages;
    }
}
