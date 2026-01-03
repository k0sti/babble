//! Waveform visualization component
//!
//! Displays audio waveform for recording and playback visualization.

use crate::ui::state::{AppState, RecordingState, PlaybackState};
use crate::ui::theme::Theme;
use egui::{self, Vec2, Rect, Pos2, Color32, Stroke};

/// Waveform visualization component
pub struct Waveform<'a> {
    state: &'a AppState,
    theme: &'a Theme,
    /// Height of the waveform display
    height: f32,
}

impl<'a> Waveform<'a> {
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self {
            state,
            theme,
            height: 60.0,
        }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = Vec2::new(ui.available_width(), self.height);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        let painter = ui.painter();

        // Draw background
        painter.rect_filled(rect, self.theme.card_rounding, self.theme.bg_secondary);

        // Determine which waveform to show and its color
        let (samples, color, is_animated) = self.get_waveform_data();

        if samples.is_empty() {
            // Show placeholder line
            let center_y = rect.center().y;
            painter.line_segment(
                [
                    Pos2::new(rect.left() + 8.0, center_y),
                    Pos2::new(rect.right() - 8.0, center_y),
                ],
                Stroke::new(1.0, self.theme.waveform_inactive),
            );
        } else {
            self.draw_waveform(ui, rect, &samples, color, is_animated);
        }

        // Show recording indicator
        if self.state.recording_state == RecordingState::Recording {
            self.draw_recording_indicator(ui, rect);
        }

        // Show playback progress if playing
        if self.state.audio_player.state == PlaybackState::Playing {
            self.draw_playback_cursor(ui, rect);
        }

        response
    }

    fn get_waveform_data(&self) -> (Vec<f32>, Color32, bool) {
        match self.state.recording_state {
            RecordingState::Recording => {
                (self.state.waveform_data.clone(), self.theme.recording, true)
            }
            RecordingState::Processing => {
                (self.state.waveform_data.clone(), self.theme.warning, false)
            }
            RecordingState::Idle => {
                // Show playback audio waveform if available
                if let Some(audio) = &self.state.audio_player.current_audio {
                    let samples: Vec<f32> = audio
                        .samples
                        .iter()
                        .step_by((audio.samples.len() / 200).max(1))
                        .take(200)
                        .copied()
                        .collect();

                    let color = match self.state.audio_player.state {
                        PlaybackState::Playing => self.theme.waveform_active,
                        _ => self.theme.waveform_inactive,
                    };

                    (samples, color, self.state.audio_player.state == PlaybackState::Playing)
                } else {
                    (Vec::new(), self.theme.waveform_inactive, false)
                }
            }
        }
    }

    fn draw_waveform(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        samples: &[f32],
        color: Color32,
        is_animated: bool,
    ) {
        let painter = ui.painter();

        let padding = 8.0;
        let draw_rect = rect.shrink(padding);
        let center_y = draw_rect.center().y;
        let max_height = draw_rect.height() / 2.0;

        let sample_count = samples.len();
        if sample_count == 0 {
            return;
        }

        // Draw as bars
        let bar_width = (draw_rect.width() / sample_count as f32).max(2.0);
        let bar_gap = 1.0;

        // Animation offset for recording
        let time_offset = if is_animated {
            (ui.ctx().input(|i| i.time) * 10.0) as usize % sample_count.max(1)
        } else {
            0
        };

        for (i, &sample) in samples.iter().enumerate() {
            let sample_idx = (i + time_offset) % sample_count;
            let x = draw_rect.left() + i as f32 * bar_width;

            if x + bar_width > draw_rect.right() {
                break;
            }

            // Calculate bar height based on amplitude
            let amplitude = sample.abs().min(1.0);
            let bar_height = (amplitude * max_height * 2.0).max(2.0);

            // Draw bar centered on the middle
            let bar_rect = Rect::from_center_size(
                Pos2::new(x + bar_width / 2.0, center_y),
                Vec2::new(bar_width - bar_gap, bar_height),
            );

            // Gradient effect - brighter in the middle
            let gradient_factor = 1.0 - (i as f32 / sample_count as f32 - 0.5).abs() * 0.3;
            let bar_color = color.gamma_multiply(gradient_factor);

            painter.rect_filled(bar_rect, 1.0, bar_color);
        }

        // Request repaint for animations
        if is_animated {
            ui.ctx().request_repaint();
        }
    }

    fn draw_recording_indicator(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter();

        // Pulsing red dot
        let t = ui.ctx().input(|i| i.time);
        let pulse = ((t * 2.0).sin() * 0.5 + 0.5) as f32;

        let dot_radius = 6.0 + pulse * 2.0;
        let dot_center = Pos2::new(rect.left() + 16.0, rect.top() + 16.0);

        painter.circle_filled(dot_center, dot_radius, self.theme.recording.gamma_multiply(pulse * 0.5 + 0.5));
        painter.circle_filled(dot_center, 4.0, self.theme.recording);

        // "Recording" text
        let text_pos = Pos2::new(dot_center.x + 12.0, dot_center.y);
        painter.text(
            text_pos,
            egui::Align2::LEFT_CENTER,
            "Recording",
            egui::FontId::proportional(12.0),
            self.theme.recording,
        );

        ui.ctx().request_repaint();
    }

    fn draw_playback_cursor(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter();

        let padding = 8.0;
        let draw_rect = rect.shrink(padding);

        let progress = self.state.audio_player.progress();
        let cursor_x = draw_rect.left() + progress * draw_rect.width();

        // Draw cursor line
        painter.line_segment(
            [
                Pos2::new(cursor_x, draw_rect.top()),
                Pos2::new(cursor_x, draw_rect.bottom()),
            ],
            Stroke::new(2.0, self.theme.primary),
        );

        // Draw cursor head
        painter.circle_filled(
            Pos2::new(cursor_x, draw_rect.top()),
            4.0,
            self.theme.primary,
        );

        ui.ctx().request_repaint();
    }
}

/// Compact waveform for inline display in messages
pub struct MiniWaveform<'a> {
    samples: &'a [f32],
    theme: &'a Theme,
    width: f32,
    height: f32,
}

impl<'a> MiniWaveform<'a> {
    pub fn new(samples: &'a [f32], theme: &'a Theme) -> Self {
        Self {
            samples,
            theme,
            width: 80.0,
            height: 24.0,
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = Vec2::new(self.width, self.height);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        let painter = ui.painter();

        // Background
        painter.rect_filled(rect, 4.0, self.theme.bg_tertiary);

        if self.samples.is_empty() {
            return response;
        }

        // Downsample for display
        let bar_count = 20;
        let samples_per_bar = self.samples.len() / bar_count;
        if samples_per_bar == 0 {
            return response;
        }

        let bar_width = rect.width() / bar_count as f32;
        let center_y = rect.center().y;
        let max_height = rect.height() * 0.8;

        for i in 0..bar_count {
            let start = i * samples_per_bar;
            let end = (start + samples_per_bar).min(self.samples.len());

            // Calculate RMS for this segment
            let rms: f32 = self.samples[start..end]
                .iter()
                .map(|s| s * s)
                .sum::<f32>()
                / (end - start) as f32;
            let rms = rms.sqrt();

            let height = (rms * max_height * 4.0).min(max_height).max(2.0);
            let x = rect.left() + i as f32 * bar_width + bar_width * 0.5;

            painter.line_segment(
                [
                    Pos2::new(x, center_y - height / 2.0),
                    Pos2::new(x, center_y + height / 2.0),
                ],
                Stroke::new(2.0, self.theme.waveform_active),
            );
        }

        response
    }
}
