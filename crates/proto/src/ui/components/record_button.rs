//! Record button component
//!
//! Provides the main record button that toggles audio recording on/off.

use crate::ui::state::{AppState, RecordingState};
use crate::ui::theme::Theme;
use egui::{Color32, Key, Rect, RichText, Sense, Vec2};

/// Record button component for voice input
pub struct RecordButton<'a> {
    state: &'a mut AppState,
    theme: &'a Theme,
}

impl<'a> RecordButton<'a> {
    /// Create a new record button component
    pub fn new(state: &'a mut AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    /// Show the record button and return the response
    pub fn show(mut self, ui: &mut egui::Ui) -> egui::Response {
        let size = Vec2::new(60.0, 60.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        if ui.is_rect_visible(rect) {
            self.paint_button(ui, rect, &response);
        }

        // Handle interactions
        self.handle_interactions(ui, &response);

        // Handle keyboard shortcut (Space to toggle)
        self.handle_keyboard_shortcut(ui);

        // Show tooltip with keyboard hint
        self.show_tooltip(ui, &response);

        response
    }

    /// Paint the button appearance
    fn paint_button(&self, ui: &mut egui::Ui, rect: Rect, response: &egui::Response) {
        let painter = ui.painter();
        let is_recording = self.state.recording_state == RecordingState::Recording;
        let is_processing = self.state.recording_state == RecordingState::Processing;

        // Calculate background color based on state
        let bg_color = if is_recording {
            self.theme.recording
        } else if is_processing {
            self.theme.warning.gamma_multiply(0.8)
        } else if response.hovered() {
            self.theme.primary.gamma_multiply(1.2)
        } else {
            self.theme.primary
        };

        // Draw background circle
        painter.circle_filled(rect.center(), 28.0, bg_color);

        // Draw outer ring for hover effect
        if response.hovered() && !is_recording && !is_processing {
            painter.circle_stroke(
                rect.center(),
                29.0,
                egui::Stroke::new(2.0, self.theme.primary.gamma_multiply(0.6)),
            );
        }

        // Draw icon based on state
        if is_recording {
            self.draw_stop_icon(painter, rect.center());
        } else if is_processing {
            self.draw_processing_icon(ui, painter, rect.center());
        } else {
            self.draw_mic_icon(painter, rect.center());
        }

        // Draw pulsing animation when recording
        if is_recording {
            self.draw_pulsing_ring(ui, painter, rect.center());
        }
    }

    /// Draw the stop square icon (when recording)
    fn draw_stop_icon(&self, painter: &egui::Painter, center: egui::Pos2) {
        let stop_size = 16.0;
        painter.rect_filled(
            Rect::from_center_size(center, Vec2::splat(stop_size)),
            2.0,
            Color32::WHITE,
        );
    }

    /// Draw the processing indicator (spinner-like)
    fn draw_processing_icon(&self, ui: &egui::Ui, painter: &egui::Painter, center: egui::Pos2) {
        let t = ui.ctx().input(|i| i.time);
        let angle = t * 3.0;

        // Draw rotating dots
        for i in 0..3 {
            let dot_angle = angle + (i as f64 * std::f64::consts::TAU / 3.0);
            let radius = 8.0;
            let dot_pos = egui::pos2(
                center.x + (dot_angle.cos() as f32 * radius),
                center.y + (dot_angle.sin() as f32 * radius),
            );

            let alpha = 1.0 - (i as f32 * 0.3);
            let color = Color32::from_white_alpha((255.0 * alpha) as u8);
            painter.circle_filled(dot_pos, 3.0, color);
        }

        ui.ctx().request_repaint();
    }

    /// Draw the microphone icon (when idle)
    fn draw_mic_icon(&self, painter: &egui::Painter, center: egui::Pos2) {
        let color = Color32::WHITE;

        // Mic body (rounded rectangle)
        let mic_width = 8.0;
        let mic_height = 14.0;
        let mic_rect = Rect::from_center_size(
            egui::pos2(center.x, center.y - 2.0),
            Vec2::new(mic_width, mic_height),
        );
        painter.rect_filled(mic_rect, 4.0, color);

        // Mic stand arc (bottom half circle)
        let arc_center = egui::pos2(center.x, center.y + 3.0);
        let arc_radius = 10.0;

        // Draw the arc as a series of lines (approximation)
        let num_segments = 8;
        for i in 0..num_segments {
            let start_angle = std::f32::consts::PI * (i as f32 / num_segments as f32);
            let end_angle = std::f32::consts::PI * ((i + 1) as f32 / num_segments as f32);

            let start = egui::pos2(
                arc_center.x - arc_radius * start_angle.cos(),
                arc_center.y + arc_radius * start_angle.sin(),
            );
            let end = egui::pos2(
                arc_center.x - arc_radius * end_angle.cos(),
                arc_center.y + arc_radius * end_angle.sin(),
            );

            painter.line_segment([start, end], egui::Stroke::new(2.0, color));
        }

        // Mic stand stem
        let stem_start = egui::pos2(center.x, center.y + 3.0 + arc_radius);
        let stem_end = egui::pos2(center.x, center.y + 3.0 + arc_radius + 5.0);
        painter.line_segment([stem_start, stem_end], egui::Stroke::new(2.0, color));

        // Mic base
        let base_width = 12.0;
        let base_y = center.y + 3.0 + arc_radius + 5.0;
        painter.line_segment(
            [
                egui::pos2(center.x - base_width / 2.0, base_y),
                egui::pos2(center.x + base_width / 2.0, base_y),
            ],
            egui::Stroke::new(2.0, color),
        );
    }

    /// Draw pulsing ring animation when recording
    fn draw_pulsing_ring(&self, ui: &egui::Ui, painter: &egui::Painter, center: egui::Pos2) {
        let t = ui.ctx().input(|i| i.time);
        let pulse = ((t * 3.0).sin() * 0.5 + 0.5) as f32;

        // Draw expanding pulsing ring
        let radius = 30.0 + pulse * 8.0;
        let alpha = (1.0 - pulse) * 0.6;

        painter.circle_stroke(
            center,
            radius,
            egui::Stroke::new(
                2.0 + pulse * 2.0,
                self.theme.recording.gamma_multiply(alpha),
            ),
        );

        // Second ring (offset phase)
        let pulse2 = (((t * 3.0) + std::f64::consts::PI).sin() * 0.5 + 0.5) as f32;
        let radius2 = 30.0 + pulse2 * 8.0;
        let alpha2 = (1.0 - pulse2) * 0.4;

        painter.circle_stroke(
            center,
            radius2,
            egui::Stroke::new(
                1.5 + pulse2 * 1.5,
                self.theme.recording.gamma_multiply(alpha2),
            ),
        );

        ui.ctx().request_repaint();
    }

    /// Handle button interactions (click, push-to-talk)
    fn handle_interactions(&mut self, ui: &egui::Ui, response: &egui::Response) {
        let is_recording = self.state.recording_state == RecordingState::Recording;
        let is_processing = self.state.recording_state == RecordingState::Processing;

        // Don't handle interactions when processing
        if is_processing {
            return;
        }

        // Handle click to toggle
        if response.clicked() {
            if is_recording {
                self.state.stop_recording();
            } else {
                self.state.start_recording();
            }
        }

        // Handle right-click to cancel (when recording)
        if response.secondary_clicked() && is_recording {
            self.state.cancel_recording();
        }

        // Push-to-talk: detect mouse button held
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let _pointer_over_button = response.hovered();

        // If button was being held and is now released while recording, stop
        if is_recording && !primary_down && !response.clicked() {
            // Only stop if we're in push-to-talk mode (started by holding)
            // For now, we'll rely on click toggle behavior
        }
    }

    /// Handle keyboard shortcut (Space to toggle recording)
    fn handle_keyboard_shortcut(&mut self, ui: &egui::Ui) {
        let is_recording = self.state.recording_state == RecordingState::Recording;
        let is_processing = self.state.recording_state == RecordingState::Processing;

        // Don't handle shortcuts when processing
        if is_processing {
            return;
        }

        // Space bar to toggle recording
        let space_pressed = ui.input(|i| i.key_pressed(Key::Space));

        // Only trigger if no widget has focus (to avoid conflicts with text input)
        let any_widget_focused = ui.memory(|m| m.focused().is_some());

        if space_pressed && !any_widget_focused {
            if is_recording {
                self.state.stop_recording();
            } else {
                self.state.start_recording();
            }
        }
    }

    /// Show tooltip with state info and keyboard hint
    fn show_tooltip(&self, _ui: &egui::Ui, response: &egui::Response) {
        if !response.hovered() {
            return;
        }

        let tooltip_text = match self.state.recording_state {
            RecordingState::Idle => "Click to record (Space)",
            RecordingState::Recording => "Click to stop (Space)\nRight-click to cancel",
            RecordingState::Processing => "Processing audio...",
        };

        response.clone().on_hover_text(tooltip_text);
    }
}

/// Standalone record button that can be used without full InputBar
pub struct StandaloneRecordButton<'a> {
    state: &'a mut AppState,
    theme: &'a Theme,
    size: f32,
}

impl<'a> StandaloneRecordButton<'a> {
    /// Create a new standalone record button
    pub fn new(state: &'a mut AppState, theme: &'a Theme) -> Self {
        Self {
            state,
            theme,
            size: 60.0,
        }
    }

    /// Set custom button size
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Show the button centered with label
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical_centered(|ui| {
            // Show the record button
            let response = RecordButton::new(self.state, self.theme).show(ui);

            ui.add_space(8.0);

            // Show status label below button
            let status_text = match self.state.recording_state {
                RecordingState::Idle => "Press to record",
                RecordingState::Recording => "Recording...",
                RecordingState::Processing => "Processing...",
            };

            let text_color = match self.state.recording_state {
                RecordingState::Idle => self.theme.text_muted,
                RecordingState::Recording => self.theme.recording,
                RecordingState::Processing => self.theme.warning,
            };

            ui.label(RichText::new(status_text).size(12.0).color(text_color));

            response
        })
        .inner
    }
}
