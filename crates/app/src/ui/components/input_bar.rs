//! Input bar component
//!
//! Provides text input, record button, and send controls.

use crate::ui::state::{AppState, RecordingState};
use crate::ui::theme::Theme;
use egui::{self, Key, RichText, Vec2};

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
                    // Clear button
                    self.show_clear_button(ui);

                    ui.add_space(self.theme.spacing_sm);

                    // Record button
                    self.show_record_button(ui);

                    ui.add_space(self.theme.spacing_sm);

                    // Text input
                    self.show_text_input(ui);

                    ui.add_space(self.theme.spacing_sm);

                    // Send text button
                    self.show_send_text_button(ui);

                    ui.add_space(self.theme.spacing_sm);

                    // Send voice button
                    self.show_send_voice_button(ui);
                });
            });
    }

    fn show_clear_button(&mut self, ui: &mut egui::Ui) {
        let has_content =
            !self.state.input_text.trim().is_empty() || !self.state.waveform_data.is_empty();
        let is_recording = self.state.recording_state != RecordingState::Idle;

        let button = egui::Button::new(RichText::new("🗑").size(18.0).color(if has_content {
            self.theme.text_secondary
        } else {
            self.theme.text_muted
        }))
        .min_size(Vec2::splat(44.0))
        .rounding(self.theme.button_rounding);

        let response = ui.add_enabled(has_content && !is_recording, button);

        if response.clicked() {
            self.state.input_text.clear();
            self.state.waveform_data.clear();
        }

        response.on_hover_text("Clear input");
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
        let was_right_clicked = response.secondary_clicked();

        // Check if primary mouse button is currently held down (globally)
        let primary_down = ui.input(|i| i.pointer.primary_down());

        // Detect if pointer just pressed on this button
        let pointer_pressed_on_button = ui.input(|i| {
            i.pointer.primary_pressed()
                && response
                    .rect
                    .contains(i.pointer.interact_pos().unwrap_or_default())
        });

        // Show tooltip (this consumes response if we use on_hover_text)
        if is_hovered && !is_processing {
            response.on_hover_text(tooltip);
        }

        // Handle press and release for push-to-talk
        // Start recording when pointer is first pressed on the button
        if pointer_pressed_on_button && !is_recording && !is_processing {
            self.state.start_recording();
        }

        // Stop recording when mouse button is released (check global pointer state)
        if is_recording && !primary_down {
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
                egui::Stroke::new(
                    2.0 * pulse,
                    self.theme.recording.gamma_multiply(1.0 - pulse * 0.5),
                ),
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
            .margin(egui::Margin::symmetric(12.0, 8.0))
            .id(egui::Id::new("message_input"));

        let response = ui.add_enabled(!is_generating && !is_recording, text_edit);

        // Add accessibility name for the text input
        response.widget_info(|| {
            egui::WidgetInfo::text_edit(
                ui.is_enabled() && !is_generating && !is_recording,
                &self.state.input_text,
                "Message input",
            )
        });

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

    fn show_send_text_button(&mut self, ui: &mut egui::Ui) {
        let can_send = !self.state.input_text.trim().is_empty()
            && !self.state.streaming_response.is_generating
            && self.state.recording_state == RecordingState::Idle;

        let is_generating = self.state.streaming_response.is_generating;

        let (icon, tooltip) = if is_generating {
            ("⏹", "Stop generation")
        } else {
            ("↙", "Send text (Enter)")
        };

        let button_color = if can_send || is_generating {
            self.theme.primary
        } else {
            self.theme.text_muted
        };

        let button = egui::Button::new(RichText::new(icon).size(18.0).color(egui::Color32::WHITE))
            .min_size(Vec2::splat(44.0))
            .rounding(self.theme.button_rounding)
            .fill(button_color);

        let response = ui.add_enabled(can_send || is_generating, button);

        // Add accessibility label
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                can_send || is_generating,
                "Send message",
            )
        });

        if response.clicked() {
            if is_generating {
                // Stop generation
                self.state.streaming_response.is_generating = false;
            } else {
                self.state.send_message();
            }
        }

        response.on_hover_text(tooltip);
    }

    fn show_send_voice_button(&mut self, ui: &mut egui::Ui) {
        let has_waveform = !self.state.waveform_data.is_empty();
        let is_recording = self.state.recording_state != RecordingState::Idle;
        let is_generating = self.state.streaming_response.is_generating;

        // Can send voice when we have recorded waveform data and not currently recording or generating
        let can_send = has_waveform && !is_recording && !is_generating;

        let button_color = if can_send {
            self.theme.success
        } else {
            self.theme.text_muted
        };

        let button = egui::Button::new(RichText::new("↗").size(18.0).color(egui::Color32::WHITE))
            .min_size(Vec2::splat(44.0))
            .rounding(self.theme.button_rounding)
            .fill(button_color);

        let response = ui.add_enabled(can_send, button);

        if response.clicked() {
            // Voice is automatically processed and sent after recording stops
            // This button could be used for re-sending voice or confirming
            // For now, voice is sent automatically after transcription
        }

        response.on_hover_text("Send voice");
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
