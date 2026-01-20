//! Response display component for showing LLM responses
//!
//! This module provides a UI component that displays LLM responses with:
//! - Real-time streaming token display during generation
//! - Generation status indicator
//! - Scrollable area for long responses
//! - Interruption status display

use crate::state::{AppStateSnapshot, LLMState, ResponseState, SharedAppState};
use crate::ui::theme::Theme;
use egui::{RichText, ScrollArea, Ui};

/// Response display component that shows LLM output
///
/// Displays the current LLM response with streaming support,
/// generation indicators, and scrollable content for long responses.
pub struct ResponseDisplay<'a> {
    state: &'a SharedAppState,
    theme: &'a Theme,
    max_height: f32,
}

impl<'a> ResponseDisplay<'a> {
    /// Create a new response display
    pub fn new(state: &'a SharedAppState, theme: &'a Theme) -> Self {
        Self {
            state,
            theme,
            max_height: 150.0,
        }
    }

    /// Set the maximum height for the scrollable area
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// Show the response display
    pub fn show(&self, ui: &mut Ui) {
        let snapshot = self.state.snapshot();
        self.show_snapshot(ui, &snapshot);
    }

    /// Show the response display with a state snapshot
    pub fn show_snapshot(&self, ui: &mut Ui, snapshot: &AppStateSnapshot) {
        // Only show when there's content to display or generation is happening
        let has_content = !snapshot.response.current_text.is_empty()
            || snapshot.response.last_complete.is_some();
        let is_generating = snapshot.llm.is_generating();

        if !has_content && !is_generating {
            return;
        }

        ui.group(|ui| {
            ui.vertical(|ui| {
                // Header with status
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Response")
                            .strong()
                            .size(14.0)
                            .color(self.theme.text_primary),
                    );

                    ui.add_space(8.0);

                    // Status indicator
                    let status = Self::status_indicator(snapshot, self.theme);
                    ui.label(status);
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Response content
                self.show_response_content(ui, snapshot);
            });
        });
    }

    /// Get the status indicator text
    fn status_indicator(snapshot: &AppStateSnapshot, theme: &Theme) -> RichText {
        if snapshot.llm.is_generating() {
            RichText::new("Generating...")
                .color(theme.primary)
                .strong()
                .size(12.0)
        } else if snapshot.response.was_interrupted {
            RichText::new("Interrupted")
                .color(theme.warning)
                .size(12.0)
        } else if !snapshot.response.current_text.is_empty()
            || snapshot.response.last_complete.is_some()
        {
            RichText::new("Complete")
                .color(theme.success)
                .size(12.0)
        } else {
            RichText::new("").size(12.0)
        }
    }

    /// Show the response content in a scrollable area
    fn show_response_content(&self, ui: &mut Ui, snapshot: &AppStateSnapshot) {
        // Determine which text to show
        let text = if !snapshot.response.current_text.is_empty() {
            &snapshot.response.current_text
        } else if let Some(ref last) = snapshot.response.last_complete {
            last
        } else {
            // Generating but no tokens yet
            ui.label(
                RichText::new("Waiting for response...")
                    .color(self.theme.text_muted)
                    .italics()
                    .size(14.0),
            );
            return;
        };

        // Scrollable text area
        ScrollArea::vertical()
            .max_height(self.max_height)
            .auto_shrink([false, true])
            .stick_to_bottom(snapshot.llm.is_generating()) // Auto-scroll during generation
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Display response text with word wrapping
                ui.label(
                    RichText::new(text)
                        .color(self.theme.text_secondary)
                        .size(14.0),
                );

                // Show cursor animation during generation
                if snapshot.llm.is_generating() {
                    // Use time for blinking cursor effect
                    let time = ui.ctx().input(|i| i.time);
                    let cursor_visible = (time * 2.0) as i32 % 2 == 0;

                    if cursor_visible {
                        ui.label(
                            RichText::new("▌")
                                .color(self.theme.primary)
                                .size(14.0),
                        );
                    }
                }
            });
    }
}

/// Standalone response display that works directly with ResponseState
pub struct ResponseDisplayStandalone<'a> {
    response: &'a ResponseState,
    llm_state: LLMState,
    theme: &'a Theme,
    max_height: f32,
}

impl<'a> ResponseDisplayStandalone<'a> {
    /// Create a new standalone response display
    pub fn new(response: &'a ResponseState, llm_state: LLMState, theme: &'a Theme) -> Self {
        Self {
            response,
            llm_state,
            theme,
            max_height: 150.0,
        }
    }

    /// Set the maximum height for the scrollable area
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// Show the response display
    pub fn show(&self, ui: &mut Ui) {
        // Only show when there's content to display or generation is happening
        let has_content =
            !self.response.current_text.is_empty() || self.response.last_complete.is_some();
        let is_generating = self.llm_state.is_generating();

        if !has_content && !is_generating {
            return;
        }

        ui.group(|ui| {
            ui.vertical(|ui| {
                // Header with status
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Response")
                            .strong()
                            .size(14.0)
                            .color(self.theme.text_primary),
                    );

                    ui.add_space(8.0);

                    // Status indicator
                    let status = self.status_indicator();
                    ui.label(status);
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Response content
                self.show_response_content(ui);
            });
        });
    }

    /// Get the status indicator text
    fn status_indicator(&self) -> RichText {
        if self.llm_state.is_generating() {
            RichText::new("Generating...")
                .color(self.theme.primary)
                .strong()
                .size(12.0)
        } else if self.response.was_interrupted {
            RichText::new("Interrupted")
                .color(self.theme.warning)
                .size(12.0)
        } else if !self.response.current_text.is_empty() || self.response.last_complete.is_some() {
            RichText::new("Complete")
                .color(self.theme.success)
                .size(12.0)
        } else {
            RichText::new("").size(12.0)
        }
    }

    /// Show the response content in a scrollable area
    fn show_response_content(&self, ui: &mut Ui) {
        // Determine which text to show
        let text = if !self.response.current_text.is_empty() {
            &self.response.current_text
        } else if let Some(ref last) = self.response.last_complete {
            last
        } else {
            // Generating but no tokens yet
            ui.label(
                RichText::new("Waiting for response...")
                    .color(self.theme.text_muted)
                    .italics()
                    .size(14.0),
            );
            return;
        };

        // Scrollable text area
        ScrollArea::vertical()
            .max_height(self.max_height)
            .auto_shrink([false, true])
            .stick_to_bottom(self.llm_state.is_generating()) // Auto-scroll during generation
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Display response text with word wrapping
                ui.label(
                    RichText::new(text)
                        .color(self.theme.text_secondary)
                        .size(14.0),
                );

                // Show cursor animation during generation
                if self.llm_state.is_generating() {
                    // Use time for blinking cursor effect
                    let time = ui.ctx().input(|i| i.time);
                    let cursor_visible = (time * 2.0) as i32 % 2 == 0;

                    if cursor_visible {
                        ui.label(RichText::new("▌").color(self.theme.primary).size(14.0));
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_display_creation() {
        let state = SharedAppState::new();
        let theme = Theme::dark();
        let _display = ResponseDisplay::new(&state, &theme);
    }

    #[test]
    fn test_response_display_max_height() {
        let state = SharedAppState::new();
        let theme = Theme::dark();
        let display = ResponseDisplay::new(&state, &theme).max_height(200.0);
        assert_eq!(display.max_height, 200.0);
    }

    #[test]
    fn test_status_indicator_generating() {
        let theme = Theme::dark();
        let mut snapshot = SharedAppState::new().snapshot();
        snapshot.llm = LLMState::Generating;

        // Just verify it doesn't panic - RichText doesn't implement Debug
        let _status = ResponseDisplay::status_indicator(&snapshot, &theme);
    }

    #[test]
    fn test_status_indicator_interrupted() {
        let theme = Theme::dark();
        let state = SharedAppState::new();
        {
            let mut s = state.write();
            s.response.was_interrupted = true;
            s.response.current_text = "Test".to_string();
        }
        let snapshot = state.snapshot();

        // Just verify it doesn't panic - RichText doesn't implement Debug
        let _status = ResponseDisplay::status_indicator(&snapshot, &theme);
    }

    #[test]
    fn test_status_indicator_complete() {
        let theme = Theme::dark();
        let state = SharedAppState::new();
        {
            let mut s = state.write();
            s.response.current_text = "Hello world".to_string();
        }
        let snapshot = state.snapshot();

        // Just verify it doesn't panic - RichText doesn't implement Debug
        let _status = ResponseDisplay::status_indicator(&snapshot, &theme);
    }

    #[test]
    fn test_standalone_display_creation() {
        let response = ResponseState::new();
        let theme = Theme::dark();
        let _display = ResponseDisplayStandalone::new(&response, LLMState::Idle, &theme);
    }
}
