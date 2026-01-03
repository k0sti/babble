//! Input bar component
//!
//! Provides text input, record button, and send controls.

use crate::ui::state::{AppState, RecordingState};
use crate::ui::theme::Theme;
use egui::{self, RichText, Vec2, Key};

/// Input bar component for text and voice input
pub struct InputBar<'a> {
    state: &'a mut AppState,
    theme: &'a Theme,
}

impl<'a> InputBar<'a> {
    pub fn new(state: &'a mut AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    pub fn show(mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(self.theme.bg_secondary)
            .rounding(self.theme.card_rounding)
            .inner_margin(self.theme.spacing)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Record button
                    self.show_record_button(ui);

                    ui.add_space(self.theme.spacing_sm);

                    // Text input
                    self.show_text_input(ui);

                    ui.add_space(self.theme.spacing_sm);

                    // Send button
                    self.show_send_button(ui);
                });
            });
    }

    fn show_record_button(&mut self, ui: &mut egui::Ui) {
        let is_recording = self.state.recording_state == RecordingState::Recording;
        let is_processing = self.state.recording_state == RecordingState::Processing;

        let (icon, tooltip, color) = match self.state.recording_state {
            RecordingState::Idle => ("🎤", "Hold to record", self.theme.text_secondary),
            RecordingState::Recording => ("⏹", "Release to stop", self.theme.recording),
            RecordingState::Processing => ("⏳", "Processing...", self.theme.warning),
        };

        let button = egui::Button::new(RichText::new(icon).size(20.0).color(color))
            .min_size(Vec2::splat(44.0))
            .rounding(self.theme.button_rounding);

        let button = if is_recording {
            button.fill(self.theme.recording.gamma_multiply(0.2))
        } else {
            button
        };

        let response = ui.add_enabled(!is_processing, button);

        // Store rect before consuming response with on_hover_text
        let button_rect = response.rect;

        // Handle interactions
        let is_hovered = response.hovered();
        let is_pointer_down = response.is_pointer_button_down_on();
        let was_right_clicked = response.secondary_clicked();

        // Show tooltip (this consumes response if we use on_hover_text)
        if is_hovered && !is_processing {
            response.on_hover_text(tooltip);
        }

        // Handle press and release for push-to-talk
        if is_pointer_down && !is_recording && !is_processing {
            self.state.start_recording();
        } else if !is_pointer_down && is_recording {
            self.state.stop_recording();
        }

        // Handle right-click to cancel
        if was_right_clicked && is_recording {
            self.state.cancel_recording();
        }

        // Show pulsing indicator when recording
        if is_recording {
            let t = ui.ctx().input(|i| i.time);
            let pulse = ((t * 3.0).sin() * 0.5 + 0.5) as f32;

            // Draw pulsing ring around the button
            let painter = ui.painter();
            let center = button_rect.center();
            let radius = button_rect.width() / 2.0 + 2.0 + pulse * 3.0;

            painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(2.0 * pulse, self.theme.recording.gamma_multiply(1.0 - pulse * 0.5)),
            );

            ui.ctx().request_repaint();
        }
    }

    fn show_text_input(&mut self, ui: &mut egui::Ui) {
        let is_generating = self.state.streaming_response.is_generating;
        let is_recording = self.state.recording_state != RecordingState::Idle;

        // Use remaining width for the text input
        let available_width = ui.available_width() - 60.0; // Reserve space for send button

        let text_edit = egui::TextEdit::singleline(&mut self.state.input_text)
            .hint_text("Type a message...")
            .desired_width(available_width)
            .font(egui::TextStyle::Body)
            .margin(egui::Margin::symmetric(12.0, 8.0));

        let response = ui.add_enabled(!is_generating && !is_recording, text_edit);

        // Handle Enter to send (Shift+Enter for newline in multiline mode)
        if response.has_focus() && !self.state.input_text.trim().is_empty() {
            let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
            let shift_held = ui.input(|i| i.modifiers.shift);

            if enter_pressed && !shift_held {
                self.state.send_message();
            }
        }

        // Focus the text input when clicking anywhere in the input bar (if not recording)
        if !is_recording && !is_generating {
            response.request_focus();
        }
    }

    fn show_send_button(&mut self, ui: &mut egui::Ui) {
        let can_send = !self.state.input_text.trim().is_empty()
            && !self.state.streaming_response.is_generating
            && self.state.recording_state == RecordingState::Idle;

        let icon = if self.state.streaming_response.is_generating {
            "⏹" // Stop icon when generating
        } else {
            "➤" // Send icon
        };

        let tooltip = if self.state.streaming_response.is_generating {
            "Stop generation"
        } else {
            "Send message (Enter)"
        };

        let button_color = if can_send || self.state.streaming_response.is_generating {
            self.theme.primary
        } else {
            self.theme.text_muted
        };

        let button = egui::Button::new(RichText::new(icon).size(18.0).color(egui::Color32::WHITE))
            .min_size(Vec2::splat(44.0))
            .rounding(self.theme.button_rounding)
            .fill(button_color);

        let response = ui.add_enabled(
            can_send || self.state.streaming_response.is_generating,
            button,
        );

        if response.clicked() {
            if self.state.streaming_response.is_generating {
                // TODO: Cancel generation
                self.state.streaming_response.is_generating = false;
            } else {
                self.state.send_message();
            }
        }

        response.on_hover_text(tooltip);
    }
}

/// Compact input for use in dialogs or smaller spaces
#[allow(dead_code)]
pub struct CompactInput<'a> {
    value: &'a mut String,
    placeholder: &'a str,
    theme: &'a Theme,
}

#[allow(dead_code)]
impl<'a> CompactInput<'a> {
    pub fn new(value: &'a mut String, placeholder: &'a str, theme: &'a Theme) -> Self {
        Self {
            value,
            placeholder,
            theme,
        }
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let text_edit = egui::TextEdit::singleline(self.value)
            .hint_text(self.placeholder)
            .font(egui::TextStyle::Body)
            .margin(egui::Margin::symmetric(8.0, 6.0));

        ui.add(text_edit)
    }
}
