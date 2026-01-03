//! Main application struct and eframe integration
//!
//! This module contains the main BabbleApp that implements eframe::App.

use crate::ui::components::{AudioPlayer, DebugPanel, InputBar, MessageList, Waveform};
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use egui::{self, CentralPanel, TopBottomPanel, SidePanel, RichText};
use std::time::Instant;

/// Main Babble application
pub struct BabbleApp {
    /// Application state
    state: AppState,
    /// Visual theme
    theme: Theme,
    /// Last frame time for FPS calculation
    last_frame_time: Instant,
    /// Whether the app has been initialized
    initialized: bool,
}

impl BabbleApp {
    /// Create a new Babble application
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::dark();
        theme.apply(&cc.egui_ctx);

        // Request continuous repainting for animations
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        Self {
            state: AppState::new(),
            theme,
            last_frame_time: Instant::now(),
            initialized: false,
        }
    }

    /// Initialize backend connections (called on first frame)
    fn initialize(&mut self) {
        if self.initialized {
            return;
        }

        self.state.debug_info.add_log("Babble UI initialized".to_string());
        self.initialized = true;
    }

    /// Show the top header bar
    fn show_header(&mut self, ctx: &egui::Context) {
        TopBottomPanel::top("header")
            .frame(egui::Frame::none().fill(self.theme.bg_secondary).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // App title
                    ui.label(
                        RichText::new("Babble")
                            .size(20.0)
                            .strong()
                            .color(self.theme.text_primary),
                    );

                    ui.label(
                        RichText::new("Voice Assistant")
                            .size(14.0)
                            .color(self.theme.text_muted),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Settings button
                        if ui.button("⚙").on_hover_text("Settings").clicked() {
                            // TODO: Open settings
                        }

                        // Debug toggle
                        let debug_text = if self.state.show_debug_panel { "🔍" } else { "🔍" };
                        if ui.button(debug_text).on_hover_text("Toggle Debug Panel").clicked() {
                            self.state.show_debug_panel = !self.state.show_debug_panel;
                        }

                        // Clear chat button
                        if ui.button("🗑").on_hover_text("Clear Chat").clicked() {
                            self.state.clear_messages();
                        }

                        // FPS indicator
                        ui.label(
                            RichText::new(format!("{:.0} FPS", self.state.debug_info.fps))
                                .size(11.0)
                                .family(egui::FontFamily::Monospace)
                                .color(self.theme.text_muted),
                        );
                    });
                });
            });
    }

    /// Show the bottom input area
    fn show_input_area(&mut self, ctx: &egui::Context) {
        TopBottomPanel::bottom("input_area")
            .frame(egui::Frame::none().fill(self.theme.bg_primary).inner_margin(self.theme.spacing))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Waveform visualization (when recording or playing)
                    let show_waveform = self.state.recording_state != crate::ui::state::RecordingState::Idle
                        || self.state.audio_player.state != crate::ui::state::PlaybackState::Stopped;

                    if show_waveform {
                        Waveform::new(&self.state, &self.theme).height(50.0).show(ui);
                        ui.add_space(self.theme.spacing_sm);
                    }

                    // Audio player controls (when audio is available)
                    if self.state.audio_player.current_audio.is_some() {
                        AudioPlayer::new(&mut self.state, &self.theme).show(ui);
                        ui.add_space(self.theme.spacing_sm);
                    }

                    // Input bar
                    InputBar::new(&mut self.state, &self.theme).show(ui);
                });
            });
    }

    /// Show the debug panel on the side
    fn show_debug_panel(&mut self, ctx: &egui::Context) {
        if !self.state.show_debug_panel {
            return;
        }

        SidePanel::right("debug_panel")
            .resizable(true)
            .default_width(300.0)
            .min_width(250.0)
            .max_width(500.0)
            .frame(egui::Frame::none().fill(self.theme.bg_primary).inner_margin(self.theme.spacing))
            .show(ctx, |ui| {
                DebugPanel::new(&self.state, &self.theme).show(ui);
            });
    }

    /// Show the main content area (message list)
    fn show_content(&mut self, ctx: &egui::Context) {
        CentralPanel::default()
            .frame(egui::Frame::none().fill(self.theme.bg_primary))
            .show(ctx, |ui| {
                MessageList::new(&self.state, &self.theme).show(ui);
            });
    }
}

impl eframe::App for BabbleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Calculate delta time for FPS
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time).as_secs_f64();
        self.last_frame_time = now;
        self.state.update_fps(delta);

        // Initialize on first frame
        self.initialize();

        // Poll backend events
        self.state.poll_events();

        // Render UI
        self.show_header(ctx);
        self.show_debug_panel(ctx);
        self.show_input_area(ctx);
        self.show_content(ctx);

        // Request repaint for animations
        if self.state.streaming_response.is_generating
            || self.state.recording_state != crate::ui::state::RecordingState::Idle
            || self.state.audio_player.state == crate::ui::state::PlaybackState::Playing
        {
            ctx.request_repaint();
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // TODO: Save app state if needed
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Cleanup on exit
        self.state.debug_info.add_log("Babble shutting down".to_string());
    }
}
