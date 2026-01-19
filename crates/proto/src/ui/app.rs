//! Main Proto application struct and eframe integration
//!
//! This module contains the main ProtoApp that implements eframe::App.

use crate::audio::{AudioRecorder, AudioRingBuffer};
use crate::testconfig::{AssertionContext, AssertionResult, TestCommand, TestConfig, TestRunner};
use crate::ui::components::record_button::StandaloneRecordButton;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use crossbeam_channel::{bounded, Receiver, Sender};
use egui::{CentralPanel, RichText};
use tracing::{debug, error, info, warn};

/// Main Proto application
pub struct ProtoApp {
    /// Whether the app has been initialized
    initialized: bool,
    /// Application state
    state: AppState,
    /// UI theme
    theme: Theme,
    /// Test runner (if running automated tests)
    test_runner: Option<TestRunner>,
    /// Audio recorder
    audio_recorder: Option<AudioRecorder>,
    /// Channel for receiving audio samples
    audio_rx: Option<Receiver<Vec<f32>>>,
    /// Channel for sending audio samples (kept to give to recorder)
    audio_tx: Option<Sender<Vec<f32>>>,
    /// Audio buffer for storing recorded samples
    audio_buffer: AudioRingBuffer,
    /// Exit code requested by test (if any)
    pending_exit: Option<i32>,
}

impl ProtoApp {
    /// Create a new Proto application
    pub fn new(cc: &eframe::CreationContext<'_>, test_config: Option<TestConfig>) -> Self {
        let theme = Theme::dark();

        // Apply theme to egui context
        theme.apply(&cc.egui_ctx);

        // Create test runner if test config was provided
        let test_runner = test_config.map(TestRunner::new);

        // Create audio channel (bounded to prevent unbounded memory growth)
        let (audio_tx, audio_rx) = bounded(1024);

        // Create audio buffer (5 seconds at 48kHz should be plenty)
        let audio_buffer = AudioRingBuffer::new(48000 * 5);

        // Try to create audio recorder
        let audio_recorder = match AudioRecorder::new() {
            Ok(recorder) => {
                info!(
                    "[AUDIO] Recorder initialized: {}Hz, {} channels",
                    recorder.sample_rate(),
                    recorder.channels()
                );
                Some(recorder)
            }
            Err(e) => {
                warn!("[AUDIO] Failed to initialize recorder: {}", e);
                None
            }
        };

        Self {
            initialized: false,
            state: AppState::new(),
            theme,
            test_runner,
            audio_recorder,
            audio_rx: Some(audio_rx),
            audio_tx: Some(audio_tx),
            audio_buffer,
            pending_exit: None,
        }
    }

    /// Initialize the application (called on first frame)
    fn initialize(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        // Start test runner if present
        if let Some(ref mut runner) = self.test_runner {
            runner.start();
        }

        info!("Proto UI initialized");
    }

    /// Process pending audio data from the channel
    fn process_audio(&mut self) {
        if let Some(ref rx) = self.audio_rx {
            // Drain all available audio chunks into the buffer
            while let Ok(samples) = rx.try_recv() {
                let sample_count = samples.len();
                self.audio_buffer.write(&samples);

                // Update waveform data for visualization
                self.state.waveform_data.extend(samples);
                // Keep only recent samples for visualization
                if self.state.waveform_data.len() > 4096 {
                    let excess = self.state.waveform_data.len() - 4096;
                    self.state.waveform_data.drain(0..excess);
                }

                debug!(
                    "[AUDIO] Buffered {} samples, total: {}",
                    sample_count,
                    self.audio_buffer.len()
                );
            }
        }
    }

    /// Start recording audio
    fn start_recording(&mut self) {
        if self.state.is_recording() {
            debug!("[AUDIO] Already recording, ignoring start request");
            return;
        }

        // Clear the audio buffer for new recording
        self.audio_buffer.clear();
        self.state.waveform_data.clear();

        if let Some(ref mut recorder) = self.audio_recorder {
            if let Some(tx) = self.audio_tx.clone() {
                match recorder.start(tx) {
                    Ok(()) => {
                        self.state.start_recording();
                        info!(
                            "[AUDIO] Recording started, buffer cleared (capacity: {})",
                            self.audio_buffer.capacity()
                        );
                    }
                    Err(e) => {
                        error!("[AUDIO] Failed to start recording: {}", e);
                    }
                }
            }
        } else {
            // No audio recorder, but still update state for testing
            self.state.start_recording();
            info!("[AUDIO] Recording started (no audio device)");
        }
    }

    /// Stop recording audio
    fn stop_recording(&mut self) {
        if !self.state.is_recording() {
            debug!("[AUDIO] Not recording, ignoring stop request");
            return;
        }

        if let Some(ref mut recorder) = self.audio_recorder {
            if let Err(e) = recorder.stop() {
                error!("[AUDIO] Failed to stop recording: {}", e);
            }
        }

        self.state.stop_recording();

        let sample_count = self.audio_buffer.len();
        info!(
            "[AUDIO] Recording stopped, buffer contains {} samples",
            sample_count
        );
    }

    /// Cancel recording without processing
    fn cancel_recording(&mut self) {
        if !self.state.is_recording() {
            return;
        }

        if let Some(ref mut recorder) = self.audio_recorder {
            let _ = recorder.stop();
        }

        self.state.cancel_recording();
        self.audio_buffer.clear();
        info!("[AUDIO] Recording cancelled, buffer cleared");
    }

    /// Process test runner commands
    fn process_test_commands(&mut self, ctx: &egui::Context) {
        // First, collect all pending commands from the runner
        let mut pending_commands = Vec::new();

        if let Some(ref mut runner) = self.test_runner {
            while let Some(cmd) = runner.poll() {
                pending_commands.push(cmd);
            }
        }

        // Now execute the commands (no longer borrowing test_runner)
        for (command, assertion) in pending_commands {
            // Execute the command
            match command {
                TestCommand::ClickRecord => {
                    info!("[TEST] Executing: ClickRecord");
                    if self.state.is_recording() {
                        self.stop_recording();
                    } else {
                        self.start_recording();
                    }
                }
                TestCommand::StopRecord => {
                    info!("[TEST] Executing: StopRecord");
                    self.stop_recording();
                }
                TestCommand::CancelRecord => {
                    info!("[TEST] Executing: CancelRecord");
                    self.cancel_recording();
                }
                TestCommand::Exit { code } => {
                    if code != -999 {
                        // -999 is sentinel for Log action, skip it
                        info!("[TEST] Executing: Exit with code {}", code);
                        self.pending_exit = Some(code);
                    }
                }
            }

            // Check assertion if present
            if let Some(ref assertion) = assertion {
                let context = AssertionContext {
                    is_recording: self.state.is_recording(),
                    is_processing: self.state.is_processing(),
                    is_idle: !self.state.is_recording() && !self.state.is_processing(),
                    audio_buffer_samples: self.audio_buffer.len(),
                };

                if let Some(ref mut runner) = self.test_runner {
                    let result = runner.check_assertion(assertion, &context);

                    // If assertion failed and we have a pending exit, change to failure code
                    if matches!(result, AssertionResult::Failed(_)) {
                        if self.pending_exit == Some(0) {
                            self.pending_exit = Some(1);
                        }
                    }
                }
            }
        }

        // Check if test is complete
        if let Some(ref runner) = self.test_runner {
            if runner.is_completed() {
                info!("{}", runner.summary());

                // Handle exit
                if let Some(code) = self.pending_exit.take() {
                    let final_code = if runner.test_passed() { code } else { 1 };
                    info!("[TEST] Exiting with code {}", final_code);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    std::process::exit(final_code);
                }
            }
        }
    }
}

impl eframe::App for ProtoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Initialize on first frame
        self.initialize();

        // Process audio data
        self.process_audio();

        // Process test commands (if in test mode)
        self.process_test_commands(ctx);

        // Request repaint continuously if in test mode (for timing accuracy)
        if self.test_runner.is_some() {
            ctx.request_repaint();
        }

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
                let response = StandaloneRecordButton::new(&mut self.state, &self.theme).show(ui);

                // Handle button clicks - must be done here to properly manage audio recorder
                if response.clicked() {
                    if self.state.is_recording() {
                        self.stop_recording();
                    } else if !self.state.is_processing() {
                        self.start_recording();
                    }
                }

                // Handle keyboard shortcut (Space to toggle recording)
                let space_pressed = ui.input(|i| i.key_pressed(egui::Key::Space));
                let any_widget_focused = ui.memory(|m| m.focused().is_some());
                if space_pressed && !any_widget_focused && !self.state.is_processing() {
                    if self.state.is_recording() {
                        self.stop_recording();
                    } else {
                        self.start_recording();
                    }
                }

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

                // Show audio buffer info in debug mode
                if self.state.is_recording() || self.audio_buffer.len() > 0 {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!("Audio buffer: {} samples", self.audio_buffer.len()))
                            .size(12.0)
                            .color(self.theme.text_muted.gamma_multiply(0.7)),
                    );
                }

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
