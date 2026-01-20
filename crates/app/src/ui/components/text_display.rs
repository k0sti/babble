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
