//! Main Proto application struct and eframe integration
//!
//! This module contains the main ProtoApp that implements eframe::App.

use egui::{CentralPanel, RichText};
use tracing::info;

/// Main Proto application
pub struct ProtoApp {
    /// Whether the app has been initialized
    initialized: bool,
}

impl ProtoApp {
    /// Create a new Proto application
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Set dark visuals
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        Self { initialized: false }
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
                ui.add_space(100.0);

                ui.label(
                    RichText::new("Proto")
                        .size(48.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                );

                ui.add_space(20.0);

                ui.label(
                    RichText::new("Voice-controlled LLM Assistant")
                        .size(16.0)
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(40.0);

                ui.label(
                    RichText::new("Application skeleton initialized successfully")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(100, 200, 100)),
                );
            });
        });
    }
}
