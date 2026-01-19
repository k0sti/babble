//! Status bar component
//!
//! Displays color-coded status indicators for each concurrent processor.

use crate::ui::state::{AppState, RecordingState};
use crate::ui::theme::Theme;
use egui::{self, Color32, RichText, Vec2};

/// Status of a processor
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorStatus {
    /// Processor is waiting/idle (Orange)
    Waiting,
    /// Processor is actively running (Green)
    Running,
    /// Processor encountered an error (Red)
    Error,
}

/// A single status indicator
#[derive(Clone, Debug)]
pub struct StatusIndicator {
    /// Name of the processor
    pub name: &'static str,
    /// Current status
    pub status: ProcessorStatus,
}

/// Status bar displaying processor indicators
pub struct StatusBar<'a> {
    state: &'a AppState,
    theme: &'a Theme,
    /// Animation phase for pulsing effect (0.0 to 1.0)
    pulse_phase: f32,
}

impl<'a> StatusBar<'a> {
    /// Create a new status bar
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self {
            state,
            theme,
            pulse_phase: 0.0,
        }
    }

    /// Get the current processor statuses
    pub fn get_processor_statuses(&self) -> Vec<StatusIndicator> {
        vec![
            StatusIndicator {
                name: "Audio",
                status: match self.state.recording_state {
                    RecordingState::Recording => ProcessorStatus::Running,
                    RecordingState::Processing => ProcessorStatus::Running,
                    RecordingState::Idle => ProcessorStatus::Waiting,
                },
            },
            StatusIndicator {
                name: "STT",
                status: if self.state.recording_state == RecordingState::Processing {
                    ProcessorStatus::Running
                } else {
                    ProcessorStatus::Waiting
                },
            },
            StatusIndicator {
                name: "LLM",
                status: if self.state.streaming_response.is_generating {
                    ProcessorStatus::Running
                } else if self.state.last_error.is_some() {
                    ProcessorStatus::Error
                } else {
                    ProcessorStatus::Waiting
                },
            },
        ]
    }

    /// Show the status bar
    pub fn show(mut self, ui: &mut egui::Ui) -> egui::Response {
        // Calculate pulse phase from time
        let time = ui.ctx().input(|i| i.time);
        self.pulse_phase = ((time * 2.0).sin() * 0.5 + 0.5) as f32;

        let indicators = self.get_processor_statuses();
        let has_running = indicators
            .iter()
            .any(|i| i.status == ProcessorStatus::Running);

        let response = ui.horizontal(|ui| {
            for indicator in &indicators {
                self.draw_indicator(ui, indicator);
                ui.add_space(8.0);
            }
        });

        // Request repaint for pulse animation when any processor is running
        if has_running {
            ui.ctx().request_repaint();
        }

        response.response
    }

    /// Draw a single status indicator
    fn draw_indicator(&self, ui: &mut egui::Ui, indicator: &StatusIndicator) {
        let base_color = match indicator.status {
            ProcessorStatus::Waiting => self.theme.warning, // Orange
            ProcessorStatus::Running => self.theme.success, // Green
            ProcessorStatus::Error => self.theme.error,     // Red
        };

        // Apply pulse effect for running status
        let color = if indicator.status == ProcessorStatus::Running {
            // Pulse between full color and slightly dimmed
            let alpha = 0.6 + 0.4 * self.pulse_phase;
            Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                (255.0 * alpha) as u8,
            )
        } else {
            base_color
        };

        ui.horizontal(|ui| {
            // Colored dot
            let dot_size = 10.0;
            let (rect, _response) =
                ui.allocate_exact_size(Vec2::splat(dot_size), egui::Sense::hover());

            // Draw outer glow for running state
            if indicator.status == ProcessorStatus::Running {
                let glow_alpha = (0.3 * self.pulse_phase) as u8;
                let glow_color = Color32::from_rgba_unmultiplied(
                    base_color.r(),
                    base_color.g(),
                    base_color.b(),
                    (255.0 * glow_alpha as f32 / 255.0) as u8,
                );
                ui.painter().circle_filled(rect.center(), 7.0, glow_color);
            }

            // Draw the main dot
            ui.painter().circle_filled(rect.center(), 5.0, color);

            // Label
            ui.label(
                RichText::new(indicator.name)
                    .size(12.0)
                    .color(self.theme.text_secondary),
            );
        });
    }
}

/// Compact status bar for the header area
pub struct CompactStatusBar<'a> {
    state: &'a AppState,
    theme: &'a Theme,
}

impl<'a> CompactStatusBar<'a> {
    /// Create a new compact status bar
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    /// Show the compact status bar (just dots, no labels)
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let status_bar = StatusBar::new(self.state, self.theme);
        let indicators = status_bar.get_processor_statuses();
        let has_running = indicators
            .iter()
            .any(|i| i.status == ProcessorStatus::Running);

        // Calculate pulse phase
        let time = ui.ctx().input(|i| i.time);
        let pulse_phase = ((time * 2.0).sin() * 0.5 + 0.5) as f32;

        let response = ui.horizontal(|ui| {
            for (i, indicator) in indicators.iter().enumerate() {
                let base_color = match indicator.status {
                    ProcessorStatus::Waiting => self.theme.warning,
                    ProcessorStatus::Running => self.theme.success,
                    ProcessorStatus::Error => self.theme.error,
                };

                let color = if indicator.status == ProcessorStatus::Running {
                    let alpha = 0.6 + 0.4 * pulse_phase;
                    Color32::from_rgba_unmultiplied(
                        base_color.r(),
                        base_color.g(),
                        base_color.b(),
                        (255.0 * alpha) as u8,
                    )
                } else {
                    base_color
                };

                let dot_size = 8.0;
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::splat(dot_size), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, color);

                // Tooltip on hover
                response.on_hover_text(format!("{}: {:?}", indicator.name, indicator.status));

                if i < indicators.len() - 1 {
                    ui.add_space(4.0);
                }
            }
        });

        if has_running {
            ui.ctx().request_repaint();
        }

        response.response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_status_variants() {
        assert_ne!(ProcessorStatus::Waiting, ProcessorStatus::Running);
        assert_ne!(ProcessorStatus::Running, ProcessorStatus::Error);
        assert_ne!(ProcessorStatus::Error, ProcessorStatus::Waiting);
    }

    #[test]
    fn test_status_indicator_creation() {
        let indicator = StatusIndicator {
            name: "Test",
            status: ProcessorStatus::Running,
        };
        assert_eq!(indicator.name, "Test");
        assert_eq!(indicator.status, ProcessorStatus::Running);
    }
}
