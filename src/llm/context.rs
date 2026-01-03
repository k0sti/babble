//! Conversation context management for LLM interactions
//!
//! Manages conversation history, context window limits, and message formatting
//! for the LLM.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Role of a message in the conversation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    /// System prompt/instructions
    System,
    /// User input
    User,
    /// Assistant response
    Assistant,
}

impl MessageRole {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }
    }
}

/// A single message in the conversation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// Role of the message sender
    pub role: MessageRole,

    /// Message content
    pub content: String,

    /// When the message was created
    pub timestamp: DateTime<Utc>,

    /// Estimated token count for this message
    pub token_estimate: usize,
}

impl ConversationMessage {
    /// Create a new conversation message
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        let content = content.into();
        let token_estimate = estimate_tokens(&content);

        Self {
            role,
            content,
            timestamp: Utc::now(),
            token_estimate,
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

/// Manages the conversation context and history
#[derive(Clone, Debug)]
pub struct ConversationContext {
    /// System prompt (always included)
    system_prompt: String,

    /// System prompt token estimate
    system_tokens: usize,

    /// Conversation history
    messages: Vec<ConversationMessage>,

    /// Maximum tokens allowed in context
    max_tokens: usize,

    /// Current token count (excluding system prompt)
    current_tokens: usize,

    /// Maximum number of messages to keep
    max_messages: usize,
}

impl ConversationContext {
    /// Create a new conversation context
    pub fn new(system_prompt: impl Into<String>, max_tokens: usize) -> Self {
        let system_prompt = system_prompt.into();
        let system_tokens = estimate_tokens(&system_prompt);

        Self {
            system_prompt,
            system_tokens,
            messages: Vec::new(),
            max_tokens,
            current_tokens: 0,
            max_messages: 100,
        }
    }

    /// Set the maximum number of messages to keep
    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = max;
        self
    }

    /// Get the system prompt
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Update the system prompt
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = prompt.into();
        self.system_tokens = estimate_tokens(&self.system_prompt);
    }

    /// Add a user message to the conversation
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        let message = ConversationMessage::user(content);
        self.add_message(message);
    }

    /// Add an assistant message to the conversation
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        let message = ConversationMessage::assistant(content);
        self.add_message(message);
    }

    /// Add a message to the conversation
    fn add_message(&mut self, message: ConversationMessage) {
        self.current_tokens += message.token_estimate;
        self.messages.push(message);
        self.trim_to_fit();
    }

    /// Get all messages including system prompt
    pub fn get_messages(&self) -> Vec<ConversationMessage> {
        let mut result = vec![ConversationMessage::system(self.system_prompt.clone())];
        result.extend(self.messages.clone());
        result
    }

    /// Get only the conversation messages (without system prompt)
    pub fn get_history(&self) -> &[ConversationMessage] {
        &self.messages
    }

    /// Get the last N messages
    pub fn get_recent_messages(&self, n: usize) -> Vec<ConversationMessage> {
        let start = self.messages.len().saturating_sub(n);
        self.messages[start..].to_vec()
    }

    /// Get the last user message
    pub fn last_user_message(&self) -> Option<&ConversationMessage> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
    }

    /// Get the last assistant message
    pub fn last_assistant_message(&self) -> Option<&ConversationMessage> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
    }

    /// Clear conversation history
    pub fn clear(&mut self) {
        self.messages.clear();
        self.current_tokens = 0;
    }

    /// Get estimated total token count
    pub fn total_tokens(&self) -> usize {
        self.system_tokens + self.current_tokens
    }

    /// Get available tokens for response
    pub fn available_tokens(&self) -> usize {
        self.max_tokens.saturating_sub(self.total_tokens())
    }

    /// Get the number of messages in history
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Trim old messages to fit within context window
    fn trim_to_fit(&mut self) {
        // Remove old messages if we exceed token limit
        while self.total_tokens() > self.max_tokens && !self.messages.is_empty() {
            if let Some(removed) = self.messages.first() {
                self.current_tokens = self.current_tokens.saturating_sub(removed.token_estimate);
            }
            self.messages.remove(0);
        }

        // Also enforce max message count
        while self.messages.len() > self.max_messages {
            if let Some(removed) = self.messages.first() {
                self.current_tokens = self.current_tokens.saturating_sub(removed.token_estimate);
            }
            self.messages.remove(0);
        }
    }

    /// Export conversation to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.messages)
    }

    /// Import messages from JSON
    pub fn from_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let messages: Vec<ConversationMessage> = serde_json::from_str(json)?;
        self.messages = messages;
        self.current_tokens = self.messages.iter().map(|m| m.token_estimate).sum();
        self.trim_to_fit();
        Ok(())
    }
}

/// Estimate token count for a string
///
/// Uses a simple heuristic: ~4 characters per token for English text.
/// This is a rough approximation; actual tokenization varies by model.
fn estimate_tokens(text: &str) -> usize {
    // Simple heuristic: ~4 chars per token, minimum 1 token
    let char_estimate = (text.len() + 3) / 4;

    // Also consider word count as another heuristic
    let word_estimate = text.split_whitespace().count();

    // Use the larger of the two estimates
    char_estimate.max(word_estimate).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = ConversationMessage::user("Hello, world!");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello, world!");
        assert!(msg.token_estimate > 0);
    }

    #[test]
    fn test_context_creation() {
        let ctx = ConversationContext::new("You are a helpful assistant.", 4096);
        assert_eq!(ctx.message_count(), 0);
        assert!(ctx.total_tokens() > 0); // System prompt tokens
    }

    #[test]
    fn test_add_messages() {
        let mut ctx = ConversationContext::new("System prompt", 4096);

        ctx.add_user_message("Hello");
        assert_eq!(ctx.message_count(), 1);

        ctx.add_assistant_message("Hi there!");
        assert_eq!(ctx.message_count(), 2);

        let messages = ctx.get_messages();
        assert_eq!(messages.len(), 3); // System + 2 messages
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[1].role, MessageRole::User);
        assert_eq!(messages[2].role, MessageRole::Assistant);
    }

    #[test]
    fn test_token_limiting() {
        // Very small context window
        let mut ctx = ConversationContext::new("Sys", 50);

        // Add many messages
        for i in 0..20 {
            ctx.add_user_message(format!("Message {}", i));
        }

        // Should have trimmed old messages
        assert!(ctx.message_count() < 20);
        assert!(ctx.total_tokens() <= 50);
    }

    #[test]
    fn test_clear() {
        let mut ctx = ConversationContext::new("System", 4096);
        ctx.add_user_message("Hello");
        ctx.add_assistant_message("Hi");

        ctx.clear();

        assert_eq!(ctx.message_count(), 0);
        assert_eq!(ctx.current_tokens, 0);
    }

    #[test]
    fn test_last_messages() {
        let mut ctx = ConversationContext::new("System", 4096);
        ctx.add_user_message("User 1");
        ctx.add_assistant_message("Assistant 1");
        ctx.add_user_message("User 2");

        let last_user = ctx.last_user_message().unwrap();
        assert_eq!(last_user.content, "User 2");

        let last_assistant = ctx.last_assistant_message().unwrap();
        assert_eq!(last_assistant.content, "Assistant 1");
    }

    #[test]
    fn test_token_estimation() {
        assert!(estimate_tokens("") >= 1);
        assert!(estimate_tokens("Hello") >= 1);
        assert!(estimate_tokens("This is a longer sentence with more words.") > 5);
    }

    #[test]
    fn test_json_export_import() {
        let mut ctx = ConversationContext::new("System", 4096);
        ctx.add_user_message("Hello");
        ctx.add_assistant_message("Hi there!");

        let json = ctx.to_json().unwrap();

        let mut ctx2 = ConversationContext::new("System", 4096);
        ctx2.from_json(&json).unwrap();

        assert_eq!(ctx2.message_count(), 2);
    }

    #[test]
    fn test_recent_messages() {
        let mut ctx = ConversationContext::new("System", 4096);
        for i in 0..10 {
            ctx.add_user_message(format!("Message {}", i));
        }

        let recent = ctx.get_recent_messages(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content, "Message 7");
        assert_eq!(recent[2].content, "Message 9");
    }
}
