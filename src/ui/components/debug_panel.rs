//! Debug panel component
//!
//! Displays internal state information for debugging.

use crate::ui::state::{AppState, RecordingState, PlaybackState};
use crate::ui::theme::Theme;
use egui::{self, RichText, ScrollArea};

/// Debug panel component
pub struct DebugPanel<'a> {
    state: &'a AppState,
    theme: &'a Theme,
}

impl<'a> DebugPanel<'a> {
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    pub fn show(self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(self.theme.bg_secondary)
            .rounding(self.theme.card_rounding)
            .inner_margin(self.theme.spacing)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Debug Panel")
                                .strong()
                                .color(self.theme.text_primary),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{:.1} FPS", self.state.debug_info.fps))
                                    .size(12.0)
                                    .family(egui::FontFamily::Monospace)
                                    .color(self.fps_color()),
                            );
                        });
                    });

                    ui.separator();

                    // Stats grid
                    egui::Grid::new("debug_stats")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            self.stat_row(ui, "Recording", &self.recording_status());
                            self.stat_row(ui, "Playback", &self.playback_status());
                            self.stat_row(ui, "Messages", &self.state.messages.len().to_string());
                            self.stat_row(ui, "Transcription", &self.state.debug_info.transcription_status);
                            self.stat_row(ui, "LLM Stats", &self.state.debug_info.llm_stats);
                            self.stat_row(ui, "TTS Queue", &self.state.debug_info.tts_queue_status);
                            self.stat_row(ui, "Audio Buffer", &self.state.debug_info.audio_buffer_status);
                            self.stat_row(ui, "Waveform Samples", &self.state.waveform_data.len().to_string());
                        });

                    // Error display
                    if let Some(error) = &self.state.last_error {
                        ui.add_space(self.theme.spacing_sm);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("⚠").color(self.theme.error));
                            ui.label(
                                RichText::new(error)
                                    .size(12.0)
                                    .color(self.theme.error),
                            );
                        });
                    }

                    // Streaming response info
                    if self.state.streaming_response.is_generating {
                        ui.add_space(self.theme.spacing_sm);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Generating:").color(self.theme.text_muted));
                            ui.label(
                                RichText::new(format!("{} chars", self.state.streaming_response.text.len()))
                                    .family(egui::FontFamily::Monospace)
                                    .color(self.theme.primary),
                            );
                        });
                    }

                    ui.add_space(self.theme.spacing_sm);
                    ui.separator();

                    // Log messages
                    ui.label(
                        RichText::new("Recent Logs")
                            .size(12.0)
                            .strong()
                            .color(self.theme.text_secondary),
                    );

                    let log_height = 100.0;
                    ScrollArea::vertical()
                        .max_height(log_height)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                for msg in &self.state.debug_info.log_messages {
                                    ui.label(
                                        RichText::new(msg)
                                            .size(11.0)
                                            .family(egui::FontFamily::Monospace)
                                            .color(self.theme.text_muted),
                                    );
                                }

                                if self.state.debug_info.log_messages.is_empty() {
                                    ui.label(
                                        RichText::new("No log messages")
                                            .size(11.0)
                                            .color(self.theme.text_muted)
                                            .italics(),
                                    );
                                }
                            });
                        });
                });
            });
    }

    fn stat_row(&self, ui: &mut egui::Ui, label: &str, value: &str) {
        ui.label(
            RichText::new(label)
                .size(12.0)
                .color(self.theme.text_muted),
        );

        let display_value = if value.is_empty() { "—" } else { value };

        ui.label(
            RichText::new(display_value)
                .size(12.0)
                .family(egui::FontFamily::Monospace)
                .color(self.theme.text_primary),
        );

        ui.end_row();
    }

    fn recording_status(&self) -> String {
        match self.state.recording_state {
            RecordingState::Idle => "Idle".to_string(),
            RecordingState::Recording => {
                let samples = self.state.recording_buffer.lock().len();
                format!("Recording ({} samples)", samples)
            }
            RecordingState::Processing => "Processing...".to_string(),
        }
    }

    fn playback_status(&self) -> String {
        match self.state.audio_player.state {
            PlaybackState::Stopped => "Stopped".to_string(),
            PlaybackState::Playing => {
                let current = self.state.audio_player.current_time();
                let total = self.state.audio_player.total_time();
                format!("Playing {:.1}s / {:.1}s", current, total)
            }
            PlaybackState::Paused => {
                let current = self.state.audio_player.current_time();
                format!("Paused at {:.1}s", current)
            }
        }
    }

    fn fps_color(&self) -> egui::Color32 {
        let fps = self.state.debug_info.fps;
        if fps >= 55.0 {
            self.theme.success
        } else if fps >= 30.0 {
            self.theme.warning
        } else {
            self.theme.error
        }
    }
}

/// Collapsible debug panel with toggle button
pub struct CollapsibleDebugPanel<'a> {
    state: &'a mut AppState,
    theme: &'a Theme,
}

impl<'a> CollapsibleDebugPanel<'a> {
    pub fn new(state: &'a mut AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    pub fn show(self, ui: &mut egui::Ui) {
        // Toggle button in the corner
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                let toggle_text = if self.state.show_debug_panel {
                    "Hide Debug ▲"
                } else {
                    "Show Debug ▼"
                };

                if ui.small_button(toggle_text).clicked() {
                    self.state.show_debug_panel = !self.state.show_debug_panel;
                }
            });
        });

        if self.state.show_debug_panel {
            // Need to borrow state immutably for DebugPanel
            let state_ref = &*self.state;
            DebugPanel::new(state_ref, self.theme).show(ui);
        }
    }
}
