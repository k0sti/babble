//! LLM Text Display Component
//!
//! Displays streaming LLM text in real-time with visual indicators for
//! generation state, interruption, and smooth animations.

use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use egui::{self, RichText};

/// A component for displaying streaming LLM text in real-time.
///
/// This component provides:
/// - Blinking cursor during text generation
/// - Auto-scroll to bottom as text grows
/// - Visual indication of interrupted state
/// - Smooth animation for typing effect
pub struct TextDisplay<'a> {
    state: &'a AppState,
    theme: &'a Theme,
}

impl<'a> TextDisplay<'a> {
    /// Create a new TextDisplay component.
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    /// Render the text display component.
    pub fn show(self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Build the display text with optional cursor
                    let streaming = &self.state.streaming_response;
                    let mut display_text = streaming.text.clone();

                    if streaming.is_generating {
                        // Add blinking cursor during generation
                        let time = ui.ctx().input(|i| i.time);
                        if (time * 2.0).fract() < 0.5 {
                            display_text.push('▌');
                        }
                        // Request continuous repaint for animation
                        ui.ctx().request_repaint();
                    }

                    // Determine text color based on state
                    let text_color = if streaming.was_interrupted {
                        self.theme.warning
                    } else {
                        self.theme.text_primary
                    };

                    // Show empty state or the streaming text
                    if display_text.is_empty() && !streaming.is_generating {
                        // Show placeholder when idle with no text
                        ui.label(
                            RichText::new("Waiting for response...")
                                .size(16.0)
                                .color(self.theme.text_muted)
                                .italics(),
                        );
                    } else if display_text.is_empty() && streaming.is_generating {
                        // Show typing indicator when generating but no text yet
                        self.show_typing_indicator(ui);
                    } else {
                        // Show the actual text
                        let label =
                            ui.label(RichText::new(&display_text).size(16.0).color(text_color));

                        // Add accessibility info
                        let accessibility_text = if streaming.is_generating {
                            format!("Generating response: {}", &streaming.text)
                        } else if streaming.was_interrupted {
                            format!("Interrupted response: {}", &streaming.text)
                        } else {
                            format!("Response: {}", &streaming.text)
                        };
                        label.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Label,
                                true,
                                &accessibility_text,
                            )
                        });
                    }

                    // Show interrupted indicator if applicable
                    if streaming.was_interrupted && !streaming.is_generating {
                        ui.add_space(self.theme.spacing_sm);
                        ui.label(
                            RichText::new("(Generation was interrupted)")
                                .size(12.0)
                                .color(self.theme.warning)
                                .italics(),
                        );
                    }
                });
            });
    }

    /// Show a typing indicator (animated dots) when waiting for first token.
    fn show_typing_indicator(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for i in 0..3 {
                let time = ui.ctx().input(|i| i.time);
                // Stagger the animation for each dot
                let phase = time * 3.0 + i as f64 * 0.5;
                let alpha = (phase.sin() * 0.5 + 0.5) as f32;

                ui.label(
                    RichText::new("●")
                        .size(12.0)
                        .color(self.theme.primary.gamma_multiply(alpha)),
                );
            }
        });
        ui.ctx().request_repaint();
    }
}

/// Helper struct for managing streaming text state.
///
/// This provides a cleaner API for appending tokens, tracking
/// generation state, and handling interruptions.
#[derive(Debug, Clone, Default)]
pub struct StreamingText {
    /// The accumulated text content.
    pub content: String,
    /// Whether text is currently being generated.
    pub is_generating: bool,
    /// Whether generation was interrupted before completion.
    pub was_interrupted: bool,
}

impl StreamingText {
    /// Create a new empty StreamingText.
    pub fn new() -> Self {
        Self {
            content: String::new(),
            is_generating: false,
            was_interrupted: false,
        }
    }

    /// Append a token to the streaming text.
    ///
    /// This should be called for each token received from the LLM.
    pub fn append(&mut self, token: &str) {
        self.content.push_str(token);
    }

    /// Mark the generation as complete.
    ///
    /// # Arguments
    /// * `interrupted` - Whether the generation was interrupted before natural completion
    pub fn complete(&mut self, interrupted: bool) {
        self.is_generating = false;
        self.was_interrupted = interrupted;
    }

    /// Clear the text and reset state for a new generation.
    pub fn clear(&mut self) {
        self.content.clear();
        self.is_generating = false;
        self.was_interrupted = false;
    }

    /// Start a new generation, clearing previous content.
    pub fn start(&mut self) {
        self.clear();
        self.is_generating = true;
    }
}
