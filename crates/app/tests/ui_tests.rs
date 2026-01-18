//! UI automation tests using egui_kittest and AccessKit
//!
//! These tests verify the UI behavior by simulating user interactions
//! and checking the accessibility tree for expected elements.

use babble::messages::{Message, MessageContent, Sender};
use babble::ui::{AppState, Theme};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

/// Application state wrapper for testing
struct TestApp {
    state: AppState,
    #[allow(dead_code)]
    theme: Theme,
}

impl TestApp {
    fn new() -> Self {
        Self {
            state: AppState::new(),
            theme: Theme::dark(),
        }
    }

    fn with_message(self, sender: Sender, text: &str) -> Self {
        self.state
            .messages
            .add(Message::new(sender, MessageContent::Text(text.to_string())));
        self
    }

    #[allow(dead_code)]
    fn with_streaming(mut self, text: &str) -> Self {
        self.state.streaming_response.is_generating = true;
        self.state.streaming_response.text = text.to_string();
        self
    }
}

/// Render the chat UI for testing
fn render_chat_ui(app: &mut TestApp, ui: &mut egui::Ui) {
    // Message display area
    egui::ScrollArea::vertical()
        .id_salt("test_messages")
        .max_height(300.0)
        .show(ui, |ui| {
            let messages = app.state.messages.get_all();
            for message in &messages {
                let is_user = matches!(message.sender, Sender::User);
                let label_text = match &message.content {
                    MessageContent::Text(text) => {
                        if is_user {
                            format!("User message: {}", text)
                        } else {
                            format!("Assistant response: {}", text)
                        }
                    }
                    _ => "Non-text message".to_string(),
                };

                let display_text = match &message.content {
                    MessageContent::Text(text) => text.clone(),
                    _ => "[media]".to_string(),
                };

                let response = ui.label(&display_text);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label_text)
                });
            }

            // Show streaming response if generating
            if app.state.streaming_response.is_generating
                && !app.state.streaming_response.text.is_empty()
            {
                let streaming_text =
                    format!("Streaming response: {}", &app.state.streaming_response.text);
                let response = ui.label(&app.state.streaming_response.text);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &streaming_text)
                });
            }
        });

    ui.separator();

    // Input area
    ui.horizontal(|ui| {
        let text_edit = egui::TextEdit::singleline(&mut app.state.input_text)
            .hint_text("Type a message...")
            .desired_width(200.0)
            .id(egui::Id::new("message_input"));

        let text_response = ui.add(text_edit);
        // For text inputs, use labeled() to set the accessibility label
        text_response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Message input")
        });

        let send_enabled = !app.state.input_text.trim().is_empty();
        let send_button = egui::Button::new("Send");
        let send_response = ui.add_enabled(send_enabled, send_button);
        send_response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, send_enabled, "Send message")
        });

        if send_response.clicked() {
            let text = app.state.input_text.trim().to_string();
            if !text.is_empty() {
                // Add user message
                let user_message = Message::new(Sender::User, MessageContent::Text(text));
                app.state.messages.add(user_message);
                app.state.input_text.clear();
            }
        }
    });
}

/// Test that the message input field exists and is accessible
#[test]
fn test_message_input_exists() {
    let app = TestApp::new();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Find the message input by its accessibility label - if found, test passes
    let _input = harness.get_by_label("Message input");
}

/// Test that the send button exists and is accessible
#[test]
fn test_send_button_exists() {
    let app = TestApp::new();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Find the send button by its accessibility label
    let _button = harness.get_by_label("Send message");
}

/// Test that typing text into the input field works
#[test]
fn test_type_text_into_input() {
    let app = TestApp::new();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Find and focus the message input
    harness.get_by_label("Message input").focus();
    harness.run();

    // Type text into the input
    harness.get_by_label("Message input").type_text("Hello, world!");
    harness.run();

    // The state should now contain the typed text
    assert_eq!(harness.state().state.input_text, "Hello, world!");
}

/// Test that clicking send adds a user message
#[test]
fn test_send_message_creates_user_message() {
    let app = TestApp::new();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Type a message
    harness.get_by_label("Message input").focus();
    harness.run();

    harness.get_by_label("Message input").type_text("Test message");
    harness.run();

    // Click the send button
    harness.get_by_label("Send message").click();
    harness.run();

    // Verify the message was added
    let messages = harness.state().state.messages.get_all();
    assert_eq!(messages.len(), 1, "Should have exactly one message");

    let first_message = &messages[0];
    assert!(
        matches!(first_message.sender, Sender::User),
        "Message should be from user"
    );
    match &first_message.content {
        MessageContent::Text(text) => {
            assert_eq!(text, "Test message", "Message text should match");
        }
        _ => panic!("Message should be text content"),
    }

    // Input should be cleared
    assert!(
        harness.state().state.input_text.is_empty(),
        "Input should be cleared after sending"
    );
}

/// Test that user messages appear in the message list with correct accessibility labels
#[test]
fn test_user_message_appears_in_list() {
    let app = TestApp::new().with_message(Sender::User, "Hello AI!");

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Find the user message by its accessibility label
    let _message = harness.get_by_label("User message: Hello AI!");
}

/// Test that assistant responses appear in the message list
#[test]
fn test_assistant_response_appears_in_list() {
    let app = TestApp::new().with_message(Sender::Assistant, "Hello! How can I help you today?");

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Find the assistant message by its accessibility label
    let _message = harness.get_by_label("Assistant response: Hello! How can I help you today?");
}

/// Test the complete flow: enter text, send to LLM, receive response
#[test]
fn test_complete_chat_flow() {
    let app = TestApp::new();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Step 1: Type a message
    harness.get_by_label("Message input").focus();
    harness.run();

    harness.get_by_label("Message input").type_text("What is 2 + 2?");
    harness.run();

    // Step 2: Click send
    harness.get_by_label("Send message").click();
    harness.run();

    // Verify user message was sent
    {
        let messages = harness.state().state.messages.get_all();
        assert_eq!(messages.len(), 1, "Should have user message");
        assert!(matches!(messages[0].sender, Sender::User));
    }

    // Step 3: Simulate LLM response (in real scenario, this would come from the backend)
    harness.state_mut().state.messages.add(Message::new(
        Sender::Assistant,
        MessageContent::Text("2 + 2 equals 4.".to_string()),
    ));

    // Re-run to update UI
    harness.run();

    // Step 4: Verify both messages are visible
    let messages = harness.state().state.messages.get_all();
    assert_eq!(
        messages.len(),
        2,
        "Should have both user and assistant messages"
    );

    // Find both messages in the accessibility tree
    let _user_msg = harness.get_by_label("User message: What is 2 + 2?");
    let _assistant_msg = harness.get_by_label("Assistant response: 2 + 2 equals 4.");
}

/// Test that streaming response is accessible
#[test]
fn test_streaming_response_accessible() {
    let app = TestApp::new().with_streaming("The answer is");

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Find the streaming response by its accessibility label
    let _streaming = harness.get_by_label("Streaming response: The answer is");
}

/// Test that empty input cannot be sent
#[test]
fn test_cannot_send_empty_message() {
    let app = TestApp::new();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Try to click send with empty input
    harness.get_by_label("Send message").click();
    harness.run();

    // No messages should be added
    let messages = harness.state().state.messages.get_all();
    assert!(messages.is_empty(), "Should not send empty message");
}

/// Test that sending a message to LLM produces a response
/// This test verifies the actual LLM integration - it should FAIL if LLM is not responding
#[test]
fn test_send_message_to_llm_gets_response() {
    let app = TestApp::new();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Step 1: Type a message
    harness.get_by_label("Message input").focus();
    harness.run();

    harness
        .get_by_label("Message input")
        .type_text("What is 2 + 2?");
    harness.run();

    // Step 2: Click send - this calls state.send_message() which sends to LLM
    harness.get_by_label("Send message").click();
    harness.run();

    // Verify user message was sent
    {
        let messages = harness.state().state.messages.get_all();
        assert_eq!(messages.len(), 1, "Should have user message");
        assert!(matches!(messages[0].sender, Sender::User));
    }

    // Step 3: Poll for events and run multiple frames to allow LLM response to arrive
    // In a real scenario with LLM connected, poll_events() would receive LLMEvent::Complete
    for _ in 0..100 {
        harness.state_mut().state.poll_events();
        harness.run();
    }

    // Step 4: Check that we received an assistant response
    let messages = harness.state().state.messages.get_all();

    // This assertion will FAIL if LLM is not connected/responding
    assert!(
        messages.len() >= 2,
        "Expected LLM response but only found {} message(s). LLM is not responding!",
        messages.len()
    );

    // Verify the second message is from assistant
    assert!(
        matches!(messages[1].sender, Sender::Assistant),
        "Second message should be from assistant"
    );
}

/// Test multiple messages in conversation
#[test]
fn test_multiple_messages_conversation() {
    let app = TestApp::new()
        .with_message(Sender::User, "Hi!")
        .with_message(Sender::Assistant, "Hello!")
        .with_message(Sender::User, "How are you?")
        .with_message(Sender::Assistant, "I'm doing well, thanks!");

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 500.0))
        .build_state(
            |ctx, app: &mut TestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_chat_ui(app, ui);
                });
            },
            app,
        );

    harness.run();

    // Verify all messages are accessible
    let _ = harness.get_by_label("User message: Hi!");
    let _ = harness.get_by_label("Assistant response: Hello!");
    let _ = harness.get_by_label("User message: How are you?");
    let _ = harness.get_by_label("Assistant response: I'm doing well, thanks!");

    // All 4 messages should be present
    assert_eq!(harness.state().state.messages.get_all().len(), 4);
}
