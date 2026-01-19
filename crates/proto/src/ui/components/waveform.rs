//! Waveform visualization component
//!
//! Displays an animated waveform that visualizes recorded speech in real-time.

use crate::ui::state::{AppState, RecordingState};
use crate::ui::theme::Theme;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Waveform visualization component for audio recording
pub struct Waveform<'a> {
    samples: &'a [f32],
    theme: &'a Theme,
    height: f32,
    is_recording: bool,
}

impl<'a> Waveform<'a> {
    /// Create a new waveform visualization
    pub fn new(samples: &'a [f32], theme: &'a Theme) -> Self {
        Self {
            samples,
            theme,
            height: 60.0,
            is_recording: false,
        }
    }

    /// Set the height of the waveform display
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Set whether the waveform is in recording state (enables animation)
    pub fn recording(mut self, is_recording: bool) -> Self {
        self.is_recording = is_recording;
        self
    }

    /// Show the waveform and return the response
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = Vec2::new(ui.available_width(), self.height);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Draw background
            painter.rect_filled(rect, self.theme.card_rounding, self.theme.bg_secondary);

            if self.samples.is_empty() {
                // Show placeholder line when no audio data
                let center_y = rect.center().y;
                painter.line_segment(
                    [
                        Pos2::new(rect.left() + 8.0, center_y),
                        Pos2::new(rect.right() - 8.0, center_y),
                    ],
                    Stroke::new(1.0, self.theme.waveform_inactive),
                );
            } else {
                // Draw the waveform bars
                self.draw_waveform(ui, rect);
            }

            // Show recording indicator when recording
            if self.is_recording {
                self.draw_recording_indicator(ui, rect);
            }
        }

        // Request continuous repaints when recording for animation
        if self.is_recording {
            ui.ctx().request_repaint();
        }

        response
    }

    /// Draw the bar-style waveform visualization
    fn draw_waveform(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter();

        let padding = 8.0;
        let draw_rect = rect.shrink(padding);
        let center_y = draw_rect.center().y;
        let max_height = draw_rect.height() / 2.0;

        // Downsample to target bar count for consistent display
        let bar_count = 60;
        let samples_per_bar = (self.samples.len() / bar_count).max(1);

        let bar_width = draw_rect.width() / bar_count as f32;
        let bar_gap = 1.0;

        // Animation offset when recording (scrolling effect)
        let time_offset = if self.is_recording {
            let time = ui.ctx().input(|i| i.time);
            (time * 10.0) as usize % bar_count.max(1)
        } else {
            0
        };

        // Get the color with optional pulsing effect
        let base_color = self.color(ui);

        for i in 0..bar_count {
            let x = draw_rect.left() + i as f32 * bar_width;

            if x + bar_width > draw_rect.right() {
                break;
            }

            // Calculate RMS amplitude for this segment
            let start = ((i + time_offset) % bar_count) * samples_per_bar;
            let end = (start + samples_per_bar).min(self.samples.len());

            let rms = if start < self.samples.len() && start < end {
                calculate_rms(&self.samples[start..end])
            } else {
                0.0
            };

            // Scale bar height based on amplitude
            let bar_height = (rms * max_height * 4.0).clamp(2.0, max_height);

            // Draw centered bar
            let bar_rect = Rect::from_center_size(
                Pos2::new(x + bar_width / 2.0, center_y),
                Vec2::new(bar_width - bar_gap, bar_height),
            );

            // Apply gradient effect - brighter in the middle
            let gradient_factor = 1.0 - (i as f32 / bar_count as f32 - 0.5).abs() * 0.3;
            let bar_color = base_color.gamma_multiply(gradient_factor);

            painter.rect_filled(bar_rect, 1.0, bar_color);
        }
    }

    /// Get the waveform color, with pulsing effect when recording
    fn color(&self, ui: &egui::Ui) -> Color32 {
        if self.is_recording {
            // Pulsing red effect when recording
            let time = ui.ctx().input(|i| i.time);
            let pulse = ((time * 2.0).sin() * 0.3 + 0.7) as f32;

            let base = self.theme.recording;
            Color32::from_rgba_unmultiplied(
                (base.r() as f32 * pulse) as u8,
                (base.g() as f32 * pulse.min(0.8)) as u8,
                (base.b() as f32 * pulse.min(0.8)) as u8,
                255,
            )
        } else {
            self.theme.waveform_inactive
        }
    }

    /// Draw the recording indicator (pulsing red dot + text)
    fn draw_recording_indicator(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter();

        // Pulsing animation
        let time = ui.ctx().input(|i| i.time);
        let pulse = ((time * 2.0).sin() * 0.5 + 0.5) as f32;

        // Outer pulsing circle
        let dot_radius = 6.0 + pulse * 2.0;
        let dot_center = Pos2::new(rect.left() + 16.0, rect.top() + 16.0);

        painter.circle_filled(
            dot_center,
            dot_radius,
            self.theme.recording.gamma_multiply(pulse * 0.5 + 0.5),
        );

        // Inner solid dot
        painter.circle_filled(dot_center, 4.0, self.theme.recording);

        // "Recording" text label
        let text_pos = Pos2::new(dot_center.x + 12.0, dot_center.y);
        painter.text(
            text_pos,
            egui::Align2::LEFT_CENTER,
            "Recording",
            egui::FontId::proportional(12.0),
            self.theme.recording,
        );
    }
}

/// Calculate RMS (Root Mean Square) amplitude of audio samples
fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt()
}

/// Waveform widget that integrates with AppState directly
///
/// This provides a convenient wrapper that extracts waveform data and
/// recording state from the application state.
pub struct StateWaveform<'a> {
    state: &'a AppState,
    theme: &'a Theme,
    height: f32,
}

impl<'a> StateWaveform<'a> {
    /// Create a new state-aware waveform
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self {
            state,
            theme,
            height: 60.0,
        }
    }

    /// Set the height of the waveform display
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Show the waveform and return the response
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let is_recording = self.state.recording_state == RecordingState::Recording;

        Waveform::new(&self.state.waveform_data, self.theme)
            .height(self.height)
            .recording(is_recording)
            .show(ui)
    }
}
