//! Main application struct and eframe integration
//!
//! This module contains the main BabbleApp that implements eframe::App.

use crate::integration::{IntegrationConfig, Orchestrator, OrchestratorHandle};
use crate::ui::components::{AudioPlayer, DebugPanel, InputBar, MessageList, Waveform};
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use egui::{self, CentralPanel, RichText, SidePanel, TopBottomPanel};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;
use tracing::{error, info};

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
    /// Orchestrator handle for backend communication
    orchestrator_handle: Option<Arc<OrchestratorHandle>>,
    /// Worker thread handles
    worker_handles: Vec<JoinHandle<()>>,
    /// Backend initialization error
    backend_error: Option<String>,
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
            orchestrator_handle: None,
            worker_handles: Vec::new(),
            backend_error: None,
        }
    }

    /// Initialize backend connections (called on first frame)
    fn initialize(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        self.state
            .debug_info
            .add_log("Babble UI initialized".to_string());

        // Try to initialize the orchestrator
        match self.initialize_orchestrator() {
            Ok(()) => {
                info!("Backend initialized successfully");
                self.state
                    .debug_info
                    .add_log("Backend connected".to_string());
            }
            Err(e) => {
                error!("Failed to initialize backend: {}", e);
                self.backend_error = Some(e.clone());
                self.state
                    .debug_info
                    .add_log(format!("Backend error: {}", e));
            }
        }
    }

    /// Initialize the orchestrator and connect to app state
    fn initialize_orchestrator(&mut self) -> Result<(), String> {
        // Create configuration (text-only mode for now since models may not be available)
        let config = IntegrationConfig::default()
            .without_audio_input()
            .without_audio_output();

        // Create orchestrator
        let (orchestrator, handle) = Orchestrator::new(config)
            .map_err(|e| format!("Failed to create orchestrator: {}", e))?;

        // Connect state to orchestrator
        self.state.connect_orchestrator(&handle);

        // Store handle
        self.orchestrator_handle = Some(Arc::new(handle));

        // Start the orchestrator
        let handles = orchestrator
            .start()
            .map_err(|e| format!("Failed to start orchestrator: {}", e))?;
        self.worker_handles = handles;

        Ok(())
    }

    /// Show the top header bar
    fn show_header(&mut self, ctx: &egui::Context) {
        TopBottomPanel::top("header")
            .frame(
                egui::Frame::none()
                    .fill(self.theme.bg_secondary)
                    .inner_margin(12.0),
            )
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
                        let debug_text = if self.state.show_debug_panel {
                            "🔍"
                        } else {
                            "🔍"
                        };
                        if ui
                            .button(debug_text)
                            .on_hover_text("Toggle Debug Panel")
                            .clicked()
                        {
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
            .frame(
                egui::Frame::none()
                    .fill(self.theme.bg_primary)
                    .inner_margin(self.theme.spacing),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Waveform visualization (when recording or playing)
                    let show_waveform = self.state.recording_state
                        != crate::ui::state::RecordingState::Idle
                        || self.state.audio_player.state
                            != crate::ui::state::PlaybackState::Stopped;

                    if show_waveform {
                        Waveform::new(&self.state, &self.theme)
                            .height(50.0)
                            .show(ui);
                        ui.add_space(self.theme.spacing_sm);
                    }

                    // Audio player controls (always visible)
                    AudioPlayer::new(&mut self.state, &self.theme).show(ui);
                    ui.add_space(self.theme.spacing_sm);

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
            .frame(
                egui::Frame::none()
                    .fill(self.theme.bg_primary)
                    .inner_margin(self.theme.spacing),
            )
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
        self.state
            .debug_info
            .add_log("Babble shutting down".to_string());
        info!("Babble shutting down");

        // Shutdown orchestrator via LLM command (sends shutdown signal)
        if let Some(tx) = &self.state.llm_command_tx {
            let _ = tx.send(crate::llm::LLMCommand::Shutdown);
        }

        // Wait for worker threads to finish (with timeout)
        for handle in self.worker_handles.drain(..) {
            // Give threads a moment to shutdown gracefully
            let _ = handle.join();
        }
    }
}
