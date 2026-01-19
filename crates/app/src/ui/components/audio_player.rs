//! Audio player controls component
//!
//! Provides play/pause/stop, next/prev controls, and progress display.

use crate::ui::state::{AppState, PlaybackState};
use crate::ui::theme::Theme;
use egui::{self, Rect, RichText, Sense, Vec2};

/// Audio player component
pub struct AudioPlayer<'a> {
    state: &'a mut AppState,
    theme: &'a Theme,
}

impl<'a> AudioPlayer<'a> {
    pub fn new(state: &'a mut AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    pub fn show(mut self, ui: &mut egui::Ui) {
        let has_audio = self.state.audio_player.current_audio.is_some();

        egui::Frame::none()
            .fill(self.theme.bg_secondary)
            .rounding(self.theme.card_rounding)
            .inner_margin(self.theme.spacing)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Track info
                    ui.vertical(|ui| {
                        ui.set_width(120.0);

                        if has_audio {
                            ui.label(
                                RichText::new("Voice Response")
                                    .strong()
                                    .color(self.theme.text_primary),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "Track {} of {}",
                                    self.state.audio_player.current_index + 1,
                                    self.state.audio_player.playlist.len().max(1)
                                ))
                                .size(12.0)
                                .color(self.theme.text_muted),
                            );
                        } else {
                            ui.label(RichText::new("No audio").color(self.theme.text_muted));
                        }
                    });

                    ui.add_space(self.theme.spacing);

                    // Playback controls
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::splat(4.0);

                        // Previous button
                        let prev_enabled = self.state.audio_player.has_previous();
                        let prev_btn = ui.add_enabled(
                            prev_enabled,
                            egui::Button::new(RichText::new("⏮").size(18.0))
                                .min_size(Vec2::splat(36.0)),
                        );
                        if prev_btn.clicked() {
                            self.state.audio_player.previous();
                        }

                        // Play/Pause button
                        let (icon, tooltip) = match self.state.audio_player.state {
                            PlaybackState::Playing => ("⏸", "Pause"),
                            _ => ("▶", "Play"),
                        };

                        let play_btn = ui.add_enabled(
                            has_audio,
                            egui::Button::new(RichText::new(icon).size(22.0))
                                .min_size(Vec2::splat(44.0)),
                        );
                        if play_btn.clicked() {
                            self.state.toggle_playback();
                        }
                        play_btn.on_hover_text(tooltip);

                        // Stop button
                        let stop_btn = ui.add_enabled(
                            has_audio && self.state.audio_player.state != PlaybackState::Stopped,
                            egui::Button::new(RichText::new("⏹").size(18.0))
                                .min_size(Vec2::splat(36.0)),
                        );
                        if stop_btn.clicked() {
                            self.state.stop_playback();
                        }
                        stop_btn.on_hover_text("Stop");

                        // Next button
                        let next_enabled = self.state.audio_player.has_next();
                        let next_btn = ui.add_enabled(
                            next_enabled,
                            egui::Button::new(RichText::new("⏭").size(18.0))
                                .min_size(Vec2::splat(36.0)),
                        );
                        if next_btn.clicked() {
                            self.state.audio_player.next();
                        }
                    });

                    ui.add_space(self.theme.spacing);

                    // Progress bar and time
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Time display
                        let current = self.state.audio_player.current_time();
                        let total = self.state.audio_player.total_time();

                        ui.label(
                            RichText::new(format!(
                                "{} / {}",
                                format_time(current),
                                format_time(total)
                            ))
                            .size(12.0)
                            .color(self.theme.text_muted)
                            .family(egui::FontFamily::Monospace),
                        );

                        ui.add_space(self.theme.spacing_sm);

                        // Progress bar
                        let progress = self.state.audio_player.progress();
                        self.show_progress_bar(ui, progress, has_audio);

                        ui.add_space(self.theme.spacing_sm);

                        // Volume control
                        self.show_volume_control(ui);
                    });
                });
            });
    }

    fn show_progress_bar(&mut self, ui: &mut egui::Ui, progress: f32, interactive: bool) {
        let desired_size = Vec2::new(200.0, 8.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        let painter = ui.painter();

        // Background
        painter.rect_filled(rect, 4.0, self.theme.bg_tertiary);

        // Progress fill
        let fill_width = rect.width() * progress;
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_width, rect.height()));
        painter.rect_filled(fill_rect, 4.0, self.theme.primary);

        // Handle seeking
        if interactive && response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let seek_progress = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                if let Some(audio) = &self.state.audio_player.current_audio {
                    self.state.audio_player.playback_position =
                        (seek_progress * audio.samples.len() as f32) as usize;
                }
            }
        }

        // Hover effect
        if response.hovered() && interactive {
            painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, self.theme.primary));

            // Show position indicator
            if let Some(pos) = ui.ctx().pointer_hover_pos() {
                if rect.contains(pos) {
                    let hover_progress = (pos.x - rect.left()) / rect.width();
                    let hover_time = hover_progress * self.state.audio_player.total_time();

                    egui::show_tooltip(
                        ui.ctx(),
                        response.layer_id,
                        egui::Id::new("seek_tooltip"),
                        |ui| {
                            ui.label(format_time(hover_time));
                        },
                    );
                }
            }
        }
    }

    fn show_volume_control(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Volume icon
            let icon = if self.state.audio_player.volume == 0.0 {
                "🔇"
            } else if self.state.audio_player.volume < 0.5 {
                "🔉"
            } else {
                "🔊"
            };

            if ui.button(icon).clicked() {
                // Toggle mute
                if self.state.audio_player.volume > 0.0 {
                    self.state.audio_player.volume = 0.0;
                } else {
                    self.state.audio_player.volume = 0.8;
                }
            }

            // Volume slider
            let slider = egui::Slider::new(&mut self.state.audio_player.volume, 0.0..=1.0)
                .show_value(false)
                .clamping(egui::SliderClamping::Always);

            ui.add_sized(Vec2::new(60.0, 20.0), slider);
        });
    }
}

/// Format time in MM:SS format
fn format_time(seconds: f32) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    format!("{:02}:{:02}", mins, secs)
}
