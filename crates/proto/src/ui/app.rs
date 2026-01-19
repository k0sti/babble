//! Main Proto application struct and eframe integration
//!
//! This module contains the main ProtoApp that implements eframe::App.

use crate::ui::components::record_button::StandaloneRecordButton;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use egui::{CentralPanel, RichText};
use tracing::info;

/// Main Proto application
pub struct ProtoApp {
    /// Whether the app has been initialized
    initialized: bool,
    /// Application state
    state: AppState,
    /// UI theme
    theme: Theme,
}

impl ProtoApp {
    /// Create a new Proto application
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::dark();

        // Apply theme to egui context
        theme.apply(&cc.egui_ctx);

        Self {
            initialized: false,
            state: AppState::new(),
            theme,
        }
    }

    /// Initialize the application (called on first frame)
    fn initialize(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        info!("Proto UI initialized");
    }
}

impl eframe::App for ProtoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Initialize on first frame
        self.initialize();

        // Render main UI
        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);

                ui.label(
                    RichText::new("Proto")
                        .size(48.0)
                        .strong()
                        .color(self.theme.text_primary),
                );

                ui.add_space(12.0);

                ui.label(
                    RichText::new("Voice-controlled LLM Assistant")
                        .size(16.0)
                        .color(self.theme.text_secondary),
                );

                ui.add_space(60.0);

                // Record button
                StandaloneRecordButton::new(&mut self.state, &self.theme).show(ui);

                ui.add_space(40.0);

                // Status indicator
                let status_text = match self.state.recording_state {
                    crate::ui::state::RecordingState::Idle => "Ready to record",
                    crate::ui::state::RecordingState::Recording => "Recording audio...",
                    crate::ui::state::RecordingState::Processing => "Processing speech...",
                };

                ui.label(
                    RichText::new(status_text)
                        .size(14.0)
                        .color(self.theme.text_muted),
                );

                // Keyboard hint
                ui.add_space(20.0);
                ui.label(
                    RichText::new("Press Space or click to toggle recording")
                        .size(12.0)
                        .color(self.theme.text_muted.gamma_multiply(0.7)),
                );
            });
        });
    }
}
