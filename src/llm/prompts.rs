//! System prompts and prompt templates for the voice assistant

/// Default system prompt for the Babble voice assistant
pub const SYSTEM_PROMPT: &str = r#"You are Babble, a helpful and friendly voice assistant. Your responses will be converted to speech, so keep them clear, concise, and conversational.

## Response Format

Wrap ALL text that should be spoken aloud with [SPEAK] and [/SPEAK] markers.

Example:
[SPEAK]Hello! I'd be happy to help you with that.[/SPEAK]

## Guidelines

1. **Always use [SPEAK] markers** for spoken content
2. **Keep responses concise** - aim for 1-3 sentences when possible
3. **Be conversational** - use natural, flowing language
4. **Avoid technical jargon** unless specifically asked
5. **Don't include** code blocks, URLs, or complex formatting in [SPEAK] tags

## What NOT to speak

Leave these OUTSIDE of [SPEAK] tags (they will be displayed but not spoken):
- Code snippets
- URLs and links
- Long lists or technical details
- Mathematical formulas

## Example Responses

User: "What's the weather like?"
[SPEAK]I don't have access to real-time weather data, but I can help you find a weather service or app that does![/SPEAK]

User: "How do I make a list in Python?"
[SPEAK]To create a list in Python, you use square brackets.[/SPEAK]

Here's an example:
```python
my_list = [1, 2, 3]
```

[SPEAK]You can add items using the append method.[/SPEAK]"#;

/// Compact system prompt for smaller context windows
pub const COMPACT_SYSTEM_PROMPT: &str = r#"You are Babble, a voice assistant. Wrap spoken text with [SPEAK]...[/SPEAK] markers.

Rules:
- Use [SPEAK] for all spoken content
- Keep responses brief and conversational
- Don't put code, URLs, or technical details in [SPEAK] tags

Example: [SPEAK]Hello! How can I help you today?[/SPEAK]"#;

/// Build a customized system prompt
pub fn build_system_prompt(
    assistant_name: Option<&str>,
    personality: Option<&str>,
    additional_instructions: Option<&str>,
) -> String {
    let name = assistant_name.unwrap_or("Babble");
    let personality_desc = personality.unwrap_or("helpful and friendly");

    let mut prompt = format!(
        r#"You are {name}, a {personality_desc} voice assistant. Your responses will be converted to speech.

## Response Format

Wrap ALL spoken text with [SPEAK] and [/SPEAK] markers.
Example: [SPEAK]Hello! How can I help you?[/SPEAK]

## Guidelines

1. Always use [SPEAK] markers for spoken content
2. Keep responses concise and conversational
3. Leave code, URLs, and technical details outside [SPEAK] tags
"#
    );

    if let Some(instructions) = additional_instructions {
        prompt.push_str("\n## Additional Instructions\n\n");
        prompt.push_str(instructions);
        prompt.push('\n');
    }

    prompt
}

/// TTS marker constants
pub mod markers {
    /// Opening marker for spoken text
    pub const SPEAK_OPEN: &str = "[SPEAK]";

    /// Closing marker for spoken text
    pub const SPEAK_CLOSE: &str = "[/SPEAK]";

    /// Check if text contains TTS markers
    pub fn contains_markers(text: &str) -> bool {
        text.contains(SPEAK_OPEN) || text.contains(SPEAK_CLOSE)
    }

    /// Estimate if a partial token might be part of a marker
    pub fn might_be_partial_marker(text: &str) -> bool {
        // Check if text ends with potential partial marker
        let potential_starts = ["[", "[S", "[SP", "[SPE", "[SPEA", "[SPEAK", "[SPEAK]"];
        let potential_closes = [
            "[/", "[/S", "[/SP", "[/SPE", "[/SPEA", "[/SPEAK", "[/SPEAK]",
        ];

        for start in potential_starts {
            if text.ends_with(start) {
                return true;
            }
        }

        for close in potential_closes {
            if text.ends_with(close) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_contains_markers() {
        assert!(SYSTEM_PROMPT.contains("[SPEAK]"));
        assert!(SYSTEM_PROMPT.contains("[/SPEAK]"));
    }

    #[test]
    fn test_build_system_prompt_default() {
        let prompt = build_system_prompt(None, None, None);
        assert!(prompt.contains("Babble"));
        assert!(prompt.contains("helpful and friendly"));
        assert!(prompt.contains("[SPEAK]"));
    }

    #[test]
    fn test_build_system_prompt_custom() {
        let prompt = build_system_prompt(
            Some("Assistant"),
            Some("professional and concise"),
            Some("Always respond in formal English."),
        );

        assert!(prompt.contains("Assistant"));
        assert!(prompt.contains("professional and concise"));
        assert!(prompt.contains("formal English"));
    }

    #[test]
    fn test_markers_detection() {
        assert!(markers::contains_markers("[SPEAK]Hello[/SPEAK]"));
        assert!(markers::contains_markers("text [SPEAK]more text"));
        assert!(!markers::contains_markers("plain text without markers"));
    }

    #[test]
    fn test_partial_marker_detection() {
        assert!(markers::might_be_partial_marker("Hello ["));
        assert!(markers::might_be_partial_marker("text [SP"));
        assert!(markers::might_be_partial_marker("word [/SPE"));
        assert!(!markers::might_be_partial_marker("complete text"));
        assert!(!markers::might_be_partial_marker("Hello world"));
    }
}
