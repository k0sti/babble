//! Debug panel for displaying application state
//!
//! This module provides a debug UI panel that shows the complete state
//! of the application, useful for development and testing.

use crate::state::{AppState, AppStateSnapshot, LLMState, RecordingState, SharedAppState};
use crate::ui::theme::Theme;
use egui::{Color32, RichText, Ui};

/// Debug panel that displays complete application state
pub struct DebugPanel<'a> {
    state: &'a SharedAppState,
    theme: &'a Theme,
}

impl<'a> DebugPanel<'a> {
    /// Create a new debug panel
    pub fn new(state: &'a SharedAppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    /// Show the debug panel
    pub fn show(&self, ui: &mut Ui) {
        let snapshot = self.state.snapshot();
        self.show_snapshot(ui, &snapshot);
    }

    /// Show the debug panel with a state snapshot
    pub fn show_snapshot(&self, ui: &mut Ui, snapshot: &AppStateSnapshot) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Debug State")
                            .strong()
                            .size(14.0)
                            .color(self.theme.text_primary),
                    );
                    ui.add_space(8.0);
                    let busy_indicator = if snapshot.recording.is_active() || snapshot.llm.is_generating() {
                        RichText::new("BUSY").color(self.theme.warning).strong()
                    } else {
                        RichText::new("IDLE").color(self.theme.success).strong()
                    };
                    ui.label(busy_indicator);
                });

                ui.separator();

                // Use a grid layout for state display
                egui::Grid::new("debug_state_grid")
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Recording State
                        self.state_row(ui, "Recording", &format!("{:?}", snapshot.recording),
                            Self::recording_state_color(snapshot.recording, self.theme));

                        // LLM State
                        self.state_row(ui, "LLM", &format!("{:?}", snapshot.llm),
                            Self::llm_state_color(snapshot.llm, self.theme));

                        // Audio Buffer
                        self.state_row(ui, "Audio Samples", &format!("{}", snapshot.audio_buffer_samples),
                            self.theme.text_secondary);

                        // Audio Duration (assuming 16kHz)
                        let duration_secs = snapshot.audio_buffer_samples as f32 / 16000.0;
                        self.state_row(ui, "Audio Duration", &format!("{:.2}s", duration_secs),
                            self.theme.text_secondary);

                        ui.end_row();
                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        // Transcription State header
                        ui.label(RichText::new("Transcription").strong().color(self.theme.text_primary));
                        ui.end_row();

                        // Has First Word
                        self.state_row(ui, "Has First Word",
                            &format!("{}", snapshot.transcription.has_first_word),
                            Self::bool_color(snapshot.transcription.has_first_word, self.theme));

                        // First Word
                        self.state_row(ui, "First Word",
                            &snapshot.transcription.first_word.as_deref().unwrap_or("(none)"),
                            self.theme.text_secondary);

                        // Last Transcription
                        let transcription_text = snapshot.transcription.last_text.as_deref().unwrap_or("(none)");
                        let truncated = if transcription_text.len() > 50 {
                            format!("{}...", &transcription_text[..50])
                        } else {
                            transcription_text.to_string()
                        };
                        self.state_row(ui, "Last Text", &truncated, self.theme.text_secondary);

                        ui.end_row();
                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        // Response State header
                        ui.label(RichText::new("LLM Response").strong().color(self.theme.text_primary));
                        ui.end_row();

                        // Current Response Length
                        self.state_row(ui, "Current Length",
                            &format!("{} chars", snapshot.response.current_text.len()),
                            self.theme.text_secondary);

                        // Was Interrupted
                        self.state_row(ui, "Was Interrupted",
                            &format!("{}", snapshot.response.was_interrupted),
                            Self::bool_color(snapshot.response.was_interrupted, self.theme));

                        // Current Response Preview
                        let response_preview = if snapshot.response.current_text.is_empty() {
                            "(empty)".to_string()
                        } else if snapshot.response.current_text.len() > 50 {
                            format!("{}...", &snapshot.response.current_text[..50])
                        } else {
                            snapshot.response.current_text.clone()
                        };
                        self.state_row(ui, "Current Text", &response_preview, self.theme.text_secondary);

                        // Last Complete Response
                        let last_complete = snapshot.response.last_complete.as_deref().unwrap_or("(none)");
                        let last_truncated = if last_complete.len() > 50 {
                            format!("{}...", &last_complete[..50])
                        } else {
                            last_complete.to_string()
                        };
                        self.state_row(ui, "Last Complete", &last_truncated, self.theme.text_secondary);

                        ui.end_row();
                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        // Error State
                        ui.label(RichText::new("Error").strong().color(self.theme.text_primary));
                        ui.end_row();

                        let error_text = snapshot.error.as_deref().unwrap_or("(none)");
                        let error_color = if snapshot.error.is_some() {
                            self.theme.error
                        } else {
                            self.theme.text_muted
                        };
                        self.state_row(ui, "Current Error", error_text, error_color);
                    });
            });
        });
    }

    /// Helper to render a state row
    fn state_row(&self, ui: &mut Ui, label: &str, value: &str, value_color: Color32) {
        ui.label(RichText::new(label).color(self.theme.text_muted).size(12.0));
        ui.label(RichText::new(value).color(value_color).monospace().size(12.0));
        ui.end_row();
    }

    /// Get color for recording state
    fn recording_state_color(state: RecordingState, theme: &Theme) -> Color32 {
        match state {
            RecordingState::Idle => theme.text_muted,
            RecordingState::Recording => theme.recording,
            RecordingState::Processing => theme.warning,
        }
    }

    /// Get color for LLM state
    fn llm_state_color(state: LLMState, theme: &Theme) -> Color32 {
        match state {
            LLMState::Idle => theme.text_muted,
            LLMState::Generating => theme.primary,
        }
    }

    /// Get color for boolean value
    fn bool_color(value: bool, theme: &Theme) -> Color32 {
        if value {
            theme.success
        } else {
            theme.text_muted
        }
    }
}

/// Standalone debug panel that can be shown directly from AppState
pub struct DebugPanelStandalone<'a> {
    state: &'a AppState,
    theme: &'a Theme,
}

impl<'a> DebugPanelStandalone<'a> {
    /// Create a new standalone debug panel
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    /// Show the debug panel
    pub fn show(&self, ui: &mut Ui) {
        let snapshot = self.state.snapshot();
        let shared = SharedAppState::new(); // Dummy for borrowing theme
        let panel = DebugPanel::new(&shared, self.theme);
        panel.show_snapshot(ui, &snapshot);
    }
}

/// Collapsible debug panel with toggle
pub struct CollapsibleDebugPanel<'a> {
    state: &'a SharedAppState,
    theme: &'a Theme,
    open: &'a mut bool,
}

impl<'a> CollapsibleDebugPanel<'a> {
    /// Create a new collapsible debug panel
    pub fn new(state: &'a SharedAppState, theme: &'a Theme, open: &'a mut bool) -> Self {
        Self { state, theme, open }
    }

    /// Show the collapsible debug panel
    pub fn show(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.selectable_label(*self.open, "Debug").clicked() {
                *self.open = !*self.open;
            }
        });

        if *self.open {
            ui.add_space(8.0);
            DebugPanel::new(self.state, self.theme).show(ui);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_panel_creation() {
        let state = SharedAppState::new();
        let theme = Theme::dark();
        let _panel = DebugPanel::new(&state, &theme);
    }

    #[test]
    fn test_recording_state_colors() {
        let theme = Theme::dark();

        let idle_color = DebugPanel::recording_state_color(RecordingState::Idle, &theme);
        assert_eq!(idle_color, theme.text_muted);

        let recording_color = DebugPanel::recording_state_color(RecordingState::Recording, &theme);
        assert_eq!(recording_color, theme.recording);

        let processing_color = DebugPanel::recording_state_color(RecordingState::Processing, &theme);
        assert_eq!(processing_color, theme.warning);
    }

    #[test]
    fn test_llm_state_colors() {
        let theme = Theme::dark();

        let idle_color = DebugPanel::llm_state_color(LLMState::Idle, &theme);
        assert_eq!(idle_color, theme.text_muted);

        let generating_color = DebugPanel::llm_state_color(LLMState::Generating, &theme);
        assert_eq!(generating_color, theme.primary);
    }

    #[test]
    fn test_bool_colors() {
        let theme = Theme::dark();

        let true_color = DebugPanel::bool_color(true, &theme);
        assert_eq!(true_color, theme.success);

        let false_color = DebugPanel::bool_color(false, &theme);
        assert_eq!(false_color, theme.text_muted);
    }
}
