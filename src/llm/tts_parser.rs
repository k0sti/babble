//! TTS marker parser for extracting spoken segments from LLM output
//!
//! This module provides streaming-capable parsing of [SPEAK]...[/SPEAK] markers
//! in LLM output, allowing text to be sent to TTS as soon as complete segments
//! are detected.

use crate::llm::prompts::markers::{SPEAK_CLOSE, SPEAK_OPEN};

/// A segment of text extracted from LLM output
#[derive(Clone, Debug, PartialEq)]
pub struct TTSSegment {
    /// The text content of this segment
    pub text: String,

    /// Whether this segment should be spoken (was inside [SPEAK] tags)
    pub should_speak: bool,

    /// Sequential index of this segment in the response
    pub index: usize,
}

impl TTSSegment {
    /// Create a new TTS segment
    pub fn new(text: String, should_speak: bool, index: usize) -> Self {
        Self {
            text,
            should_speak,
            index,
        }
    }

    /// Create a spoken segment
    pub fn spoken(text: String, index: usize) -> Self {
        Self::new(text, true, index)
    }

    /// Create a display-only segment
    pub fn display_only(text: String, index: usize) -> Self {
        Self::new(text, false, index)
    }
}

/// Parser state for tracking marker boundaries
#[derive(Clone, Debug, PartialEq)]
enum ParserState {
    /// Outside of any [SPEAK] tags
    Outside,
    /// Inside [SPEAK] tags (content should be spoken)
    InsideSpeak,
    /// Potentially starting an opening tag
    MaybeOpenTag(String),
    /// Potentially starting a closing tag
    MaybeCloseTag(String),
}

/// Streaming parser for TTS markers in LLM output
///
/// Handles token-by-token input and extracts complete TTS segments
/// as they become available, handling partial markers across token boundaries.
#[derive(Clone, Debug)]
pub struct TTSParser {
    /// Current parser state
    state: ParserState,

    /// Buffer for accumulating text in current segment
    buffer: String,

    /// Current segment index
    current_index: usize,

    /// Accumulated text that might be part of a marker
    pending_marker: String,
}

impl Default for TTSParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TTSParser {
    /// Create a new TTS parser
    pub fn new() -> Self {
        Self {
            state: ParserState::Outside,
            buffer: String::new(),
            current_index: 0,
            pending_marker: String::new(),
        }
    }

    /// Reset the parser to initial state
    pub fn reset(&mut self) {
        self.state = ParserState::Outside;
        self.buffer.clear();
        self.current_index = 0;
        self.pending_marker.clear();
    }

    /// Feed a token into the parser and extract any complete segments
    ///
    /// Returns a vector of complete TTS segments that can be processed.
    /// May return multiple segments if the token completes multiple markers.
    pub fn feed(&mut self, token: &str) -> Vec<TTSSegment> {
        let mut segments = Vec::new();

        // Combine pending marker text with new token
        let combined = format!("{}{}", self.pending_marker, token);
        self.pending_marker.clear();

        let mut chars = combined.chars().peekable();
        let mut current_text = String::new();

        while let Some(c) = chars.next() {
            current_text.push(c);

            // Check for potential marker start
            if current_text.ends_with('[') {
                // Could be start of a marker - need to look ahead
                let remaining: String = chars.clone().collect();

                if remaining.starts_with("SPEAK]") {
                    // Complete opening tag found
                    // Emit any buffered content as non-spoken
                    let prefix = &current_text[..current_text.len() - 1];
                    if !prefix.is_empty() {
                        self.buffer.push_str(prefix);
                    }

                    if !self.buffer.is_empty() && self.state == ParserState::Outside {
                        segments.push(TTSSegment::display_only(
                            self.buffer.clone(),
                            self.current_index,
                        ));
                        self.current_index += 1;
                        self.buffer.clear();
                    }

                    // Skip past "SPEAK]"
                    for _ in 0..6 {
                        chars.next();
                    }
                    current_text.clear();
                    self.state = ParserState::InsideSpeak;
                } else if remaining.starts_with("/SPEAK]") {
                    // Complete closing tag found
                    let prefix = &current_text[..current_text.len() - 1];
                    if !prefix.is_empty() {
                        self.buffer.push_str(prefix);
                    }

                    if !self.buffer.is_empty() && self.state == ParserState::InsideSpeak {
                        segments.push(TTSSegment::spoken(self.buffer.clone(), self.current_index));
                        self.current_index += 1;
                        self.buffer.clear();
                    }

                    // Skip past "/SPEAK]"
                    for _ in 0..7 {
                        chars.next();
                    }
                    current_text.clear();
                    self.state = ParserState::Outside;
                } else if is_partial_marker(&format!("[{}", remaining)) {
                    // Could be partial marker - save for next token
                    let prefix = &current_text[..current_text.len() - 1];
                    if !prefix.is_empty() {
                        self.buffer.push_str(prefix);
                    }
                    self.pending_marker = format!("[{}", remaining);
                    return segments;
                }
            }
        }

        // Add remaining text to buffer
        if !current_text.is_empty() {
            // Check if we might have a partial marker at the end
            if might_end_with_partial_marker(&current_text) {
                let (safe, pending) = split_at_potential_marker(&current_text);
                self.buffer.push_str(&safe);
                self.pending_marker = pending;
            } else {
                self.buffer.push_str(&current_text);
            }
        }

        segments
    }

    /// Flush any remaining content as a final segment
    ///
    /// Call this when the LLM response is complete to get any remaining text.
    pub fn flush(&mut self) -> Option<TTSSegment> {
        // Include any pending marker text as regular content
        if !self.pending_marker.is_empty() {
            self.buffer.push_str(&self.pending_marker);
            self.pending_marker.clear();
        }

        if self.buffer.is_empty() {
            return None;
        }

        let segment = match self.state {
            ParserState::InsideSpeak => TTSSegment::spoken(self.buffer.clone(), self.current_index),
            _ => TTSSegment::display_only(self.buffer.clone(), self.current_index),
        };

        self.buffer.clear();
        self.current_index += 1;

        Some(segment)
    }

    /// Get the current segment index
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Check if currently inside a [SPEAK] block
    pub fn is_inside_speak(&self) -> bool {
        matches!(self.state, ParserState::InsideSpeak)
    }
}

/// Check if a string could be a partial marker
fn is_partial_marker(s: &str) -> bool {
    SPEAK_OPEN.starts_with(s) || SPEAK_CLOSE.starts_with(s)
}

/// Check if string might end with partial marker
fn might_end_with_partial_marker(s: &str) -> bool {
    let suffixes = ["[", "[S", "[SP", "[SPE", "[SPEA", "[SPEAK"];
    let close_suffixes = ["[/", "[/S", "[/SP", "[/SPE", "[/SPEA", "[/SPEAK"];

    for suffix in suffixes.iter().chain(close_suffixes.iter()) {
        if s.ends_with(suffix) {
            return true;
        }
    }
    false
}

/// Split string at potential partial marker
fn split_at_potential_marker(s: &str) -> (String, String) {
    let suffixes = ["[", "[S", "[SP", "[SPE", "[SPEA", "[SPEAK"];
    let close_suffixes = ["[/", "[/S", "[/SP", "[/SPE", "[/SPEA", "[/SPEAK"];

    for suffix in suffixes.iter().chain(close_suffixes.iter()).rev() {
        if s.ends_with(suffix) {
            let split_pos = s.len() - suffix.len();
            return (s[..split_pos].to_string(), s[split_pos..].to_string());
        }
    }
    (s.to_string(), String::new())
}

/// Parse a complete response (non-streaming)
///
/// Useful for testing or when the complete response is already available.
pub fn parse_response(response: &str) -> Vec<TTSSegment> {
    let mut parser = TTSParser::new();
    let mut segments = parser.feed(response);

    if let Some(final_segment) = parser.flush() {
        segments.push(final_segment);
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_spoken_segment() {
        let segments = parse_response("[SPEAK]Hello world![/SPEAK]");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello world!");
        assert!(segments[0].should_speak);
    }

    #[test]
    fn test_display_only_segment() {
        let segments = parse_response("This is not spoken");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "This is not spoken");
        assert!(!segments[0].should_speak);
    }

    #[test]
    fn test_mixed_content() {
        let response =
            "Some code:\n```rust\nlet x = 1;\n```\n[SPEAK]Here's an explanation.[/SPEAK]";
        let segments = parse_response(response);

        assert_eq!(segments.len(), 2);
        assert!(!segments[0].should_speak);
        assert!(segments[0].text.contains("code"));
        assert!(segments[1].should_speak);
        assert_eq!(segments[1].text, "Here's an explanation.");
    }

    #[test]
    fn test_multiple_spoken_segments() {
        let response = "[SPEAK]First part.[/SPEAK] Code here. [SPEAK]Second part.[/SPEAK]";
        let segments = parse_response(response);

        assert_eq!(segments.len(), 3);
        assert!(segments[0].should_speak);
        assert_eq!(segments[0].text, "First part.");
        assert!(!segments[1].should_speak);
        assert!(segments[2].should_speak);
        assert_eq!(segments[2].text, "Second part.");
    }

    #[test]
    fn test_streaming_tokens() {
        let mut parser = TTSParser::new();

        // Simulate token-by-token streaming
        let tokens = ["[SP", "EAK]", "Hello ", "world!", "[/SPE", "AK]"];

        let mut all_segments = Vec::new();
        for token in tokens {
            all_segments.extend(parser.feed(token));
        }

        if let Some(final_seg) = parser.flush() {
            all_segments.push(final_seg);
        }

        assert_eq!(all_segments.len(), 1);
        assert!(all_segments[0].should_speak);
        assert_eq!(all_segments[0].text, "Hello world!");
    }

    #[test]
    fn test_segment_indices() {
        let response = "[SPEAK]First[/SPEAK] middle [SPEAK]Second[/SPEAK] end [SPEAK]Third[/SPEAK]";
        let segments = parse_response(response);

        for (i, segment) in segments.iter().enumerate() {
            assert_eq!(segment.index, i);
        }
    }

    #[test]
    fn test_partial_marker_detection() {
        assert!(might_end_with_partial_marker("Hello ["));
        assert!(might_end_with_partial_marker("Text [SP"));
        assert!(might_end_with_partial_marker("More [/SPE"));
        assert!(!might_end_with_partial_marker("Complete text"));
    }

    #[test]
    fn test_empty_input() {
        let segments = parse_response("");
        assert!(segments.is_empty());
    }

    #[test]
    fn test_only_markers() {
        let segments = parse_response("[SPEAK][/SPEAK]");
        assert!(segments.is_empty());
    }

    #[test]
    fn test_nested_brackets() {
        let response = "[SPEAK]Array is [1, 2, 3][/SPEAK]";
        let segments = parse_response(response);

        assert_eq!(segments.len(), 1);
        assert!(segments[0].should_speak);
        assert_eq!(segments[0].text, "Array is [1, 2, 3]");
    }

    #[test]
    fn test_parser_reset() {
        let mut parser = TTSParser::new();
        parser.feed("[SPEAK]Hello");

        parser.reset();

        assert!(!parser.is_inside_speak());
        assert_eq!(parser.current_index(), 0);
    }
}
