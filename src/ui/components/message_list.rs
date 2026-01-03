//! Message list component
//!
//! Displays the conversation history with support for text, audio, images, and files.

use crate::messages::{Message, MessageContent, Sender, AudioData};
use crate::ui::state::{AppState, StreamingResponse};
use crate::ui::theme::Theme;
use egui::{self, Color32, RichText, Sense, Vec2, Rect, Pos2, Align};

/// Message list component
pub struct MessageList<'a> {
    state: &'a AppState,
    theme: &'a Theme,
}

impl<'a> MessageList<'a> {
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    pub fn show(self, ui: &mut egui::Ui) {
        let messages = self.state.messages.get_all();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(self.theme.spacing);

                    if messages.is_empty() && !self.state.streaming_response.is_generating {
                        self.show_empty_state(ui);
                    } else {
                        for message in &messages {
                            self.show_message(ui, message);
                            ui.add_space(self.theme.spacing_sm);
                        }

                        // Show streaming response if generating
                        if self.state.streaming_response.is_generating {
                            self.show_streaming_response(ui, &self.state.streaming_response);
                        }
                    }

                    ui.add_space(self.theme.spacing);
                });
            });
    }

    fn show_empty_state(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);

            ui.label(
                RichText::new("Welcome to Babble")
                    .size(24.0)
                    .color(self.theme.text_primary),
            );

            ui.add_space(self.theme.spacing);

            ui.label(
                RichText::new("Start a conversation by typing a message or recording your voice.")
                    .size(14.0)
                    .color(self.theme.text_muted),
            );

            ui.add_space(self.theme.spacing_lg);

            // Quick action hints
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(self.theme.spacing);

                self.show_hint_card(ui, "Type", "Enter your message below");
                self.show_hint_card(ui, "Record", "Hold the mic button to record");
                self.show_hint_card(ui, "Listen", "Audio responses play automatically");
            });
        });
    }

    fn show_hint_card(&self, ui: &mut egui::Ui, title: &str, description: &str) {
        egui::Frame::none()
            .fill(self.theme.bg_secondary)
            .rounding(self.theme.card_rounding)
            .inner_margin(self.theme.spacing)
            .show(ui, |ui| {
                ui.set_width(150.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(title)
                            .size(14.0)
                            .strong()
                            .color(self.theme.primary),
                    );
                    ui.label(
                        RichText::new(description)
                            .size(12.0)
                            .color(self.theme.text_muted),
                    );
                });
            });
    }

    fn show_message(&self, ui: &mut egui::Ui, message: &Message) {
        let is_user = matches!(message.sender, Sender::User);
        let bubble_color = if is_user {
            self.theme.user_bubble
        } else {
            self.theme.assistant_bubble
        };

        let text_color = if is_user {
            Color32::WHITE
        } else {
            self.theme.text_primary
        };

        // Align messages based on sender
        let align = if is_user { Align::RIGHT } else { Align::LEFT };

        ui.with_layout(egui::Layout::top_down(align), |ui| {
            // Sender label
            ui.label(
                RichText::new(if is_user { "You" } else { "Babble" })
                    .size(12.0)
                    .color(self.theme.text_muted),
            );

            ui.add_space(2.0);

            // Message bubble
            let max_width = ui.available_width() * 0.75;

            egui::Frame::none()
                .fill(bubble_color)
                .rounding(self.theme.bubble_rounding)
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_max_width(max_width);

                    match &message.content {
                        MessageContent::Text(text) => {
                            ui.label(RichText::new(text).color(text_color));
                        }
                        MessageContent::Audio(audio) => {
                            self.show_audio_message(ui, audio, text_color);
                        }
                        MessageContent::Image(image) => {
                            self.show_image_message(ui, &image.data, text_color);
                        }
                        MessageContent::File(file) => {
                            self.show_file_message(ui, &file.name, &file.mime_type, text_color);
                        }
                    }
                });

            // Timestamp
            let time_str = message.timestamp.format("%H:%M").to_string();
            ui.label(
                RichText::new(time_str)
                    .size(10.0)
                    .color(self.theme.text_muted),
            );
        });
    }

    fn show_audio_message(&self, ui: &mut egui::Ui, audio: &AudioData, text_color: Color32) {
        ui.horizontal(|ui| {
            // Play button
            let play_btn = ui.add(egui::Button::new(
                RichText::new("▶").size(16.0).color(text_color),
            ).min_size(Vec2::splat(32.0)));

            if play_btn.clicked() {
                // TODO: Trigger audio playback
            }

            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Voice message")
                        .color(text_color)
                        .strong(),
                );

                let duration = audio.duration_seconds();
                ui.label(
                    RichText::new(format!("{:.1}s", duration))
                        .size(12.0)
                        .color(text_color.gamma_multiply(0.8)),
                );
            });

            // Mini waveform visualization
            let (rect, _) = ui.allocate_exact_size(Vec2::new(80.0, 24.0), Sense::hover());
            self.draw_mini_waveform(ui, rect, &audio.samples);
        });
    }

    fn draw_mini_waveform(&self, ui: &mut egui::Ui, rect: Rect, samples: &[f32]) {
        let painter = ui.painter();

        // Draw background
        painter.rect_filled(rect, 4.0, self.theme.bg_tertiary);

        if samples.is_empty() {
            return;
        }

        // Downsample for display
        let bar_count = 20;
        let samples_per_bar = samples.len() / bar_count;
        if samples_per_bar == 0 {
            return;
        }

        let bar_width = rect.width() / bar_count as f32;
        let center_y = rect.center().y;
        let max_height = rect.height() * 0.8;

        for i in 0..bar_count {
            let start = i * samples_per_bar;
            let end = (start + samples_per_bar).min(samples.len());

            // Calculate RMS for this segment
            let rms: f32 = samples[start..end]
                .iter()
                .map(|s| s * s)
                .sum::<f32>()
                / (end - start) as f32;
            let rms = rms.sqrt();

            let height = (rms * max_height * 4.0).min(max_height);
            let x = rect.left() + i as f32 * bar_width + bar_width * 0.5;

            painter.line_segment(
                [
                    Pos2::new(x, center_y - height / 2.0),
                    Pos2::new(x, center_y + height / 2.0),
                ],
                egui::Stroke::new(2.0, self.theme.waveform_active),
            );
        }
    }

    fn show_image_message(&self, ui: &mut egui::Ui, _data: &[u8], text_color: Color32) {
        // For now, show a placeholder
        ui.horizontal(|ui| {
            ui.label(RichText::new("🖼").size(24.0));
            ui.label(RichText::new("Image").color(text_color));
        });
    }

    fn show_file_message(&self, ui: &mut egui::Ui, name: &str, mime_type: &str, text_color: Color32) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("📎").size(20.0));
            ui.vertical(|ui| {
                ui.label(RichText::new(name).color(text_color).strong());
                ui.label(
                    RichText::new(mime_type)
                        .size(11.0)
                        .color(text_color.gamma_multiply(0.7)),
                );
            });
        });
    }

    fn show_streaming_response(&self, ui: &mut egui::Ui, response: &StreamingResponse) {
        ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
            ui.label(
                RichText::new("Babble")
                    .size(12.0)
                    .color(self.theme.text_muted),
            );

            ui.add_space(2.0);

            let max_width = ui.available_width() * 0.75;

            egui::Frame::none()
                .fill(self.theme.assistant_bubble)
                .rounding(self.theme.bubble_rounding)
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_max_width(max_width);

                    if response.text.is_empty() {
                        // Show typing indicator
                        ui.horizontal(|ui| {
                            for i in 0..3 {
                                let t = ui.ctx().input(|i| i.time);
                                let alpha = ((t * 3.0 + i as f64 * 0.5).sin() * 0.5 + 0.5) as f32;
                                ui.label(
                                    RichText::new("●")
                                        .size(10.0)
                                        .color(self.theme.text_muted.gamma_multiply(alpha)),
                                );
                            }
                        });
                    } else {
                        ui.label(
                            RichText::new(&response.text)
                                .color(self.theme.text_primary),
                        );

                        // Show blinking cursor
                        let t = ui.ctx().input(|i| i.time);
                        if (t * 2.0).fract() < 0.5 {
                            ui.label(
                                RichText::new("▎")
                                    .color(self.theme.primary),
                            );
                        }
                    }
                });
        });

        // Request repaint for animations
        ui.ctx().request_repaint();
    }
}
