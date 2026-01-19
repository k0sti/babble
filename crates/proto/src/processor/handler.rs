//! Message handler with command detection
//!
//! This module provides the message handler that checks transcribed text for
//! command words and coordinates between STT and LLM processing.

use crate::{ProtoError, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info, warn};

/// Command words that trigger immediate stop
const STOP_WORDS: &[&str] = &["stop", "halt", "cancel", "abort", "quit"];

/// Commands that can be detected from speech
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageCommand {
    /// Stop the current LLM generation
    Stop,
    /// Continue with normal processing (no command detected)
    Continue,
}

/// Commands that can be sent to the message handler
#[derive(Debug)]
pub enum MessageHandlerCommand {
    /// Process a first word for command detection
    CheckFirstWord(String),
    /// Process complete transcription
    ProcessTranscription(String),
    /// Shutdown the handler
    Shutdown,
}

/// Events emitted by the message handler
#[derive(Clone, Debug)]
pub enum MessageHandlerEvent {
    /// Command detected from first word
    CommandDetected(MessageCommand),
    /// Text ready to send to LLM
    TextReady(String),
    /// Handler has shut down
    Shutdown,
}

/// Message handler that coordinates between STT and LLM
///
/// The handler processes transcribed text, detects command words,
/// and routes messages appropriately:
/// - If a command word is detected in the first word, emit CommandDetected immediately
/// - Otherwise, wait for full transcription and emit TextReady
pub struct MessageHandler {
    command_tx: Sender<MessageHandlerCommand>,
    event_rx: Receiver<MessageHandlerEvent>,
}

impl MessageHandler {
    /// Create a new message handler
    ///
    /// Returns both the handler (for sending commands and receiving events)
    /// and the worker (to be started in a separate thread).
    pub fn new() -> (Self, MessageHandlerWorker) {
        let (command_tx, command_rx) = bounded(100);
        let (event_tx, event_rx) = bounded(100);

        let handler = Self {
            command_tx,
            event_rx,
        };

        let worker = MessageHandlerWorker {
            command_rx,
            event_tx,
            pending_command: None,
        };

        (handler, worker)
    }

    /// Get a sender for commands
    pub fn command_sender(&self) -> Sender<MessageHandlerCommand> {
        self.command_tx.clone()
    }

    /// Get a receiver for events
    pub fn event_receiver(&self) -> Receiver<MessageHandlerEvent> {
        self.event_rx.clone()
    }

    /// Check a first word for command detection
    pub fn check_first_word(&self, word: String) -> Result<()> {
        self.command_tx
            .send(MessageHandlerCommand::CheckFirstWord(word))
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send first word: {}", e)))
    }

    /// Process a complete transcription
    pub fn process_transcription(&self, text: String) -> Result<()> {
        self.command_tx
            .send(MessageHandlerCommand::ProcessTranscription(text))
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send transcription: {}", e)))
    }

    /// Request shutdown
    pub fn shutdown(&self) -> Result<()> {
        self.command_tx
            .send(MessageHandlerCommand::Shutdown)
            .map_err(|e| ProtoError::ChannelError(format!("Failed to send shutdown: {}", e)))
    }

    /// Try to receive an event (non-blocking)
    pub fn try_recv_event(&self) -> Option<MessageHandlerEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive an event (blocking)
    pub fn recv_event(&self) -> Result<MessageHandlerEvent> {
        self.event_rx
            .recv()
            .map_err(|e| ProtoError::ChannelError(format!("Failed to receive event: {}", e)))
    }
}

impl Default for MessageHandler {
    fn default() -> Self {
        Self::new().0
    }
}

/// Worker that processes messages in a dedicated thread
pub struct MessageHandlerWorker {
    command_rx: Receiver<MessageHandlerCommand>,
    event_tx: Sender<MessageHandlerEvent>,
    /// Tracks if a command was already detected for the current utterance
    pending_command: Option<MessageCommand>,
}

impl MessageHandlerWorker {
    /// Start the worker thread
    ///
    /// Returns a JoinHandle for the worker thread.
    pub fn start(self) -> JoinHandle<()> {
        thread::spawn(move || {
            if let Err(e) = self.run() {
                error!("Message handler worker error: {}", e);
            }
        })
    }

    /// Main worker loop
    fn run(mut self) -> Result<()> {
        info!("Message handler worker starting");

        loop {
            match self.command_rx.recv() {
                Ok(MessageHandlerCommand::CheckFirstWord(word)) => {
                    debug!("Checking first word: '{}'", word);

                    if let Some(command) = detect_command(&word) {
                        info!("Command detected from first word '{}': {:?}", word, command);
                        self.pending_command = Some(command.clone());

                        if let Err(e) = self
                            .event_tx
                            .send(MessageHandlerEvent::CommandDetected(command))
                        {
                            error!("Failed to send command event: {}", e);
                            break;
                        }
                    } else {
                        debug!("No command in first word '{}'", word);
                        // No command detected, will wait for full transcription
                    }
                }

                Ok(MessageHandlerCommand::ProcessTranscription(text)) => {
                    debug!("Processing transcription: '{}'", text);

                    // Handle empty transcriptions
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        warn!("Empty transcription received, ignoring");
                        self.pending_command = None;
                        continue;
                    }

                    // If a command was already detected, don't send to LLM
                    if let Some(MessageCommand::Stop) = self.pending_command.take() {
                        info!("Stop command was detected, not sending to LLM");
                        continue;
                    }

                    // Check if the entire text is just a command (edge case where
                    // first word wasn't received but full transcription is a command)
                    if is_only_command(trimmed) {
                        info!("Transcription is only a command word, emitting command");
                        if let Err(e) = self
                            .event_tx
                            .send(MessageHandlerEvent::CommandDetected(MessageCommand::Stop))
                        {
                            error!("Failed to send command event: {}", e);
                            break;
                        }
                        continue;
                    }

                    // Text is ready for LLM
                    info!("Text ready for LLM: '{}'", trimmed);
                    if let Err(e) = self
                        .event_tx
                        .send(MessageHandlerEvent::TextReady(trimmed.to_string()))
                    {
                        error!("Failed to send text ready event: {}", e);
                        break;
                    }
                }

                Ok(MessageHandlerCommand::Shutdown) => {
                    info!("Message handler received shutdown command");
                    let _ = self.event_tx.send(MessageHandlerEvent::Shutdown);
                    break;
                }

                Err(e) => {
                    error!("Command channel error: {}", e);
                    break;
                }
            }
        }

        info!("Message handler worker stopped");
        Ok(())
    }
}

/// Check if a word is a stop command
///
/// Returns true only for exact matches (case-insensitive).
/// Partial matches like "stopping" should NOT trigger stop.
fn is_stop_command(word: &str) -> bool {
    let normalized = word.to_lowercase();
    // Remove any trailing punctuation that Whisper might add
    let cleaned = normalized.trim_end_matches(|c: char| c.is_ascii_punctuation());

    STOP_WORDS.iter().any(|&sw| cleaned == sw)
}

/// Detect a command from a word
///
/// Returns Some(command) if the word is a recognized command,
/// None otherwise.
fn detect_command(word: &str) -> Option<MessageCommand> {
    if is_stop_command(word) {
        Some(MessageCommand::Stop)
    } else {
        None
    }
}

/// Check if the entire transcription is just a command word
fn is_only_command(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() == 1 {
        is_stop_command(words[0])
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_stop_command_exact_matches() {
        // Exact matches should trigger
        assert!(is_stop_command("stop"));
        assert!(is_stop_command("halt"));
        assert!(is_stop_command("cancel"));
        assert!(is_stop_command("abort"));
        assert!(is_stop_command("quit"));
    }

    #[test]
    fn test_is_stop_command_case_insensitive() {
        assert!(is_stop_command("Stop"));
        assert!(is_stop_command("STOP"));
        assert!(is_stop_command("StOp"));
        assert!(is_stop_command("HALT"));
        assert!(is_stop_command("Cancel"));
    }

    #[test]
    fn test_is_stop_command_with_punctuation() {
        // Whisper may add punctuation
        assert!(is_stop_command("stop."));
        assert!(is_stop_command("stop!"));
        assert!(is_stop_command("stop,"));
        assert!(is_stop_command("halt?"));
    }

    #[test]
    fn test_is_stop_command_partial_matches_rejected() {
        // Partial matches should NOT trigger stop
        assert!(!is_stop_command("stopping"));
        assert!(!is_stop_command("stopped"));
        assert!(!is_stop_command("stopper"));
        assert!(!is_stop_command("stopwatch"));
        assert!(!is_stop_command("halting"));
        assert!(!is_stop_command("cancellation"));
        assert!(!is_stop_command("aborted"));
        assert!(!is_stop_command("quitting"));
    }

    #[test]
    fn test_is_stop_command_non_commands() {
        assert!(!is_stop_command("hello"));
        assert!(!is_stop_command("start"));
        assert!(!is_stop_command("go"));
        assert!(!is_stop_command(""));
        assert!(!is_stop_command("   "));
    }

    #[test]
    fn test_detect_command() {
        assert_eq!(detect_command("stop"), Some(MessageCommand::Stop));
        assert_eq!(detect_command("HALT"), Some(MessageCommand::Stop));
        assert_eq!(detect_command("hello"), None);
        assert_eq!(detect_command("stopping"), None);
    }

    #[test]
    fn test_is_only_command() {
        assert!(is_only_command("stop"));
        assert!(is_only_command("STOP"));
        assert!(is_only_command("halt"));

        // Multiple words should not match even if first is command
        assert!(!is_only_command("stop that"));
        assert!(!is_only_command("stop please"));

        // Non-commands
        assert!(!is_only_command("hello"));
        assert!(!is_only_command("hello world"));

        // Empty
        assert!(!is_only_command(""));
    }

    #[test]
    fn test_message_handler_creation() {
        let (handler, _worker) = MessageHandler::new();

        // Verify we can get senders and receivers
        let _sender = handler.command_sender();
        let _receiver = handler.event_receiver();
    }

    #[test]
    fn test_handler_stop_command_flow() {
        let (handler, worker) = MessageHandler::new();
        let handle = worker.start();

        // Send a stop command first word
        handler.check_first_word("stop".to_string()).unwrap();

        // Should receive CommandDetected
        let event = handler.recv_event().unwrap();
        match event {
            MessageHandlerEvent::CommandDetected(cmd) => {
                assert_eq!(cmd, MessageCommand::Stop);
            }
            _ => panic!("Expected CommandDetected event"),
        }

        // Shutdown
        handler.shutdown().unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_handler_normal_text_flow() {
        let (handler, worker) = MessageHandler::new();
        let handle = worker.start();

        // Send a non-command first word
        handler.check_first_word("hello".to_string()).unwrap();

        // Send the full transcription
        handler
            .process_transcription("hello world".to_string())
            .unwrap();

        // Should receive TextReady
        let event = handler.recv_event().unwrap();
        match event {
            MessageHandlerEvent::TextReady(text) => {
                assert_eq!(text, "hello world");
            }
            _ => panic!("Expected TextReady event"),
        }

        // Shutdown
        handler.shutdown().unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_handler_empty_transcription_ignored() {
        let (handler, worker) = MessageHandler::new();
        let handle = worker.start();

        // Send empty transcription
        handler.process_transcription("".to_string()).unwrap();
        handler.process_transcription("   ".to_string()).unwrap();

        // Send a valid transcription to verify handler is still working
        handler
            .process_transcription("test message".to_string())
            .unwrap();

        // Should only receive one event for the valid transcription
        let event = handler.recv_event().unwrap();
        match event {
            MessageHandlerEvent::TextReady(text) => {
                assert_eq!(text, "test message");
            }
            _ => panic!("Expected TextReady event"),
        }

        // Shutdown
        handler.shutdown().unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_handler_command_suppresses_llm() {
        let (handler, worker) = MessageHandler::new();
        let handle = worker.start();

        // Send stop as first word
        handler.check_first_word("stop".to_string()).unwrap();

        // Receive the command event
        let event = handler.recv_event().unwrap();
        assert!(matches!(
            event,
            MessageHandlerEvent::CommandDetected(MessageCommand::Stop)
        ));

        // Send the full transcription (which would include "stop")
        handler
            .process_transcription("stop talking".to_string())
            .unwrap();

        // Send another message to verify handler continues
        handler.check_first_word("hello".to_string()).unwrap();
        handler
            .process_transcription("hello there".to_string())
            .unwrap();

        // Should receive TextReady for the second message, not for "stop talking"
        let event = handler.recv_event().unwrap();
        match event {
            MessageHandlerEvent::TextReady(text) => {
                assert_eq!(text, "hello there");
            }
            _ => panic!("Expected TextReady event"),
        }

        // Shutdown
        handler.shutdown().unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_handler_single_word_command_transcription() {
        let (handler, worker) = MessageHandler::new();
        let handle = worker.start();

        // Skip first word check, go directly to transcription with just "stop"
        handler.process_transcription("stop".to_string()).unwrap();

        // Should detect it as a command
        let event = handler.recv_event().unwrap();
        match event {
            MessageHandlerEvent::CommandDetected(cmd) => {
                assert_eq!(cmd, MessageCommand::Stop);
            }
            _ => panic!("Expected CommandDetected event"),
        }

        // Shutdown
        handler.shutdown().unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_handler_shutdown() {
        let (handler, worker) = MessageHandler::new();
        let handle = worker.start();

        handler.shutdown().unwrap();

        // Should receive shutdown event
        let event = handler.recv_event().unwrap();
        assert!(matches!(event, MessageHandlerEvent::Shutdown));

        handle.join().unwrap();
    }
}
