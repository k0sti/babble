//! Main Proto application struct and eframe integration
//!
//! This module contains the main ProtoApp that implements eframe::App.

/// Debug configuration passed from CLI arguments
#[derive(Clone, Debug)]
pub struct DebugConfig {
    /// Debug mode enabled
    pub enabled: bool,
    /// Max frames before exit (0 = unlimited)
    pub max_frames: u64,
}

use crate::audio::{AudioRecorder, AudioRingBuffer};
use crate::processor::{STTConfig, STTEvent, STTProcessor};
use crate::state::SharedAppState;
use crate::testconfig::{AssertionContext, AssertionResult, TestCommand, TestConfig, TestRunner};
use crate::ui::components::debug_panel::DebugPanel;
use crate::ui::components::record_button::StandaloneRecordButton;
use crate::ui::components::waveform::StateWaveform;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use babble::audio::resampler::resample_audio;
use crossbeam_channel::{bounded, Receiver, Sender};
use egui::{CentralPanel, RichText};
use std::path::PathBuf;
use std::thread::JoinHandle;
use tracing::{debug, error, info, warn};

/// Main Proto application
pub struct ProtoApp {
    /// Whether the app has been initialized
    initialized: bool,
    /// Application state (local UI state)
    state: AppState,
    /// Shared application state (for debug panel and future orchestrator integration)
    shared_state: SharedAppState,
    /// UI theme
    theme: Theme,
    /// Test runner (if running automated tests)
    test_runner: Option<TestRunner>,
    /// Audio recorder
    audio_recorder: Option<AudioRecorder>,
    /// Audio sample rate (from recorder)
    audio_sample_rate: u32,
    /// Channel for receiving audio samples
    audio_rx: Option<Receiver<Vec<f32>>>,
    /// Channel for sending audio samples (kept to give to recorder)
    audio_tx: Option<Sender<Vec<f32>>>,
    /// Audio buffer for storing recorded samples
    audio_buffer: AudioRingBuffer,
    /// Exit code requested by test (if any)
    pending_exit: Option<i32>,
    /// STT processor for speech-to-text
    stt_processor: Option<STTProcessor>,
    /// STT worker thread handle
    stt_worker_handle: Option<JoinHandle<()>>,
    /// Last transcription text
    last_transcription: Option<String>,
    /// Whether we've received a first word
    has_first_word: bool,
    /// Whether we've received a transcription
    has_transcription: bool,
    /// Whether debug panel is open
    debug_panel_open: bool,
    /// Debug configuration from CLI
    debug_config: Option<DebugConfig>,
}

impl ProtoApp {
    /// Create a new Proto application
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        test_config: Option<TestConfig>,
        debug_config: Option<DebugConfig>,
    ) -> Self {
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
        let (audio_recorder, audio_sample_rate) = match AudioRecorder::new() {
            Ok(recorder) => {
                let rate = recorder.sample_rate();
                info!(
                    "[AUDIO] Recorder initialized: {}Hz, {} channels",
                    rate,
                    recorder.channels()
                );
                (Some(recorder), rate)
            }
            Err(e) => {
                warn!("[AUDIO] Failed to initialize recorder: {}", e);
                (None, 48000) // Default fallback
            }
        };

        // Initialize STT processor
        let (stt_processor, stt_worker_handle) = Self::init_stt();

        // Create shared state with debug config values
        let shared_state = SharedAppState::new();
        if let Some(ref debug) = debug_config {
            let mut state = shared_state.write();
            state.debug_mode = debug.enabled;
            state.max_frames = debug.max_frames;
        }

        Self {
            initialized: false,
            state: AppState::new(),
            shared_state,
            theme,
            test_runner,
            audio_recorder,
            audio_sample_rate,
            audio_rx: Some(audio_rx),
            audio_tx: Some(audio_tx),
            audio_buffer,
            pending_exit: None,
            stt_processor,
            stt_worker_handle,
            last_transcription: None,
            has_first_word: false,
            has_transcription: false,
            debug_panel_open: true, // Start with debug panel open
            debug_config,
        }
    }

    /// Initialize the STT processor and worker
    fn init_stt() -> (Option<STTProcessor>, Option<JoinHandle<()>>) {
        // Look for whisper model in common locations
        let model_paths = [
            PathBuf::from("models/ggml-base.en.bin"),
            PathBuf::from("../models/ggml-base.en.bin"),
            PathBuf::from("../../models/ggml-base.en.bin"),
            dirs::data_dir()
                .map(|p| p.join("whisper/ggml-base.en.bin"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|p| p.join(".cache/whisper/ggml-base.en.bin"))
                .unwrap_or_default(),
        ];

        let model_path = model_paths.iter().find(|p| p.exists());

        let model_path = match model_path {
            Some(p) => {
                info!("[STT] Found Whisper model at: {:?}", p);
                p.clone()
            }
            None => {
                warn!(
                    "[STT] Whisper model not found. Tried: {:?}",
                    model_paths
                        .iter()
                        .filter(|p| !p.as_os_str().is_empty())
                        .collect::<Vec<_>>()
                );
                warn!("[STT] STT will be disabled. Download a model to enable speech recognition.");
                return (None, None);
            }
        };

        let config = STTConfig {
            model_path,
            language: Some("en".to_string()),
            n_threads: 4,
            min_segment_duration: 0.3,
            max_segment_duration: 30.0,
            silence_threshold: 0.5,
            vad_threshold: 0.5,
        };

        match STTProcessor::new(config) {
            Ok((processor, worker)) => {
                // Start the worker thread
                match worker.start() {
                    Ok(handle) => {
                        info!("[STT] Processor initialized and worker started");
                        (Some(processor), Some(handle))
                    }
                    Err(e) => {
                        error!("[STT] Failed to start worker: {}", e);
                        (None, None)
                    }
                }
            }
            Err(e) => {
                error!("[STT] Failed to initialize processor: {}", e);
                (None, None)
            }
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

    /// Sync local state to shared state for debug panel
    fn sync_shared_state(&self) {
        let mut shared = self.shared_state.write();

        // Sync recording state
        shared.recording = match self.state.recording_state {
            crate::ui::state::RecordingState::Idle => crate::state::RecordingState::Idle,
            crate::ui::state::RecordingState::Recording => crate::state::RecordingState::Recording,
            crate::ui::state::RecordingState::Processing => {
                crate::state::RecordingState::Processing
            }
        };

        // Sync audio buffer samples
        shared.audio_buffer_samples = self.audio_buffer.len();

        // Sync transcription state
        if self.has_first_word {
            // We don't have the actual first word stored, so just mark it
            shared.transcription.has_first_word = true;
        }
        if let Some(ref text) = self.last_transcription {
            shared.transcription.last_text = Some(text.clone());
        }
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

    /// Process STT events from the worker
    fn process_stt_events(&mut self) {
        if let Some(ref processor) = self.stt_processor {
            while let Some(event) = processor.try_recv_event() {
                match event {
                    STTEvent::FirstWord(word) => {
                        info!("[STT] First word detected: '{}'", word);
                        self.has_first_word = true;
                    }
                    STTEvent::Partial(text) => {
                        debug!("[STT] Partial transcription: '{}'", text);
                    }
                    STTEvent::Final(result) => {
                        info!("[STT] Final transcription: '{}'", result.text);
                        self.last_transcription = Some(result.text);
                        self.has_transcription = true;
                        // Processing complete, return to idle
                        self.state.finish_processing();
                    }
                    STTEvent::Error(err) => {
                        error!("[STT] Error: {}", err);
                        // On error, return to idle
                        self.state.finish_processing();
                    }
                    STTEvent::Shutdown => {
                        warn!("[STT] Worker shut down unexpectedly");
                        self.state.finish_processing();
                    }
                }
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
        self.has_first_word = false;
        self.has_transcription = false;
        self.last_transcription = None;

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

    /// Stop recording audio and send to STT
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

        // Send audio to STT processor
        if let Some(ref processor) = self.stt_processor {
            if sample_count > 0 {
                // Read audio from buffer (already mono from recorder)
                let audio_samples = self.audio_buffer.read_all();
                let input_rate = self.audio_sample_rate;

                // Calculate audio statistics for debugging
                let max_amplitude = audio_samples
                    .iter()
                    .map(|s| s.abs())
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                let rms = (audio_samples.iter().map(|s| s * s).sum::<f32>()
                    / audio_samples.len() as f32)
                    .sqrt();

                info!(
                    "[STT] Preparing audio: {} samples at {}Hz ({:.2}s), max={:.4}, rms={:.4}",
                    audio_samples.len(),
                    input_rate,
                    audio_samples.len() as f32 / input_rate as f32,
                    max_amplitude,
                    rms
                );

                // Resample to 16kHz for Whisper (audio is already mono)
                match resample_audio(&audio_samples, input_rate, 16000, 1) {
                    Ok(audio_16khz) => {
                        // Calculate resampled audio statistics
                        let max_16k = audio_16khz
                            .iter()
                            .map(|s| s.abs())
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap_or(0.0);
                        let rms_16k = (audio_16khz.iter().map(|s| s * s).sum::<f32>()
                            / audio_16khz.len() as f32)
                            .sqrt();

                        info!(
                            "[STT] Resampled to {} samples at 16kHz ({:.2}s), max={:.4}, rms={:.4}",
                            audio_16khz.len(),
                            audio_16khz.len() as f32 / 16000.0,
                            max_16k,
                            rms_16k
                        );

                        // Send directly for transcription (bypass VAD for batch mode)
                        if let Err(e) = processor.transcribe_direct(audio_16khz) {
                            error!("[STT] Failed to send audio: {}", e);
                            self.state.finish_processing();
                        } else {
                            info!("[STT] Audio sent for transcription...");
                        }
                    }
                    Err(e) => {
                        error!("[STT] Failed to resample audio: {}", e);
                        self.state.finish_processing();
                    }
                }
            } else {
                info!("[STT] No audio to process");
                self.state.finish_processing();
            }
        } else {
            // No STT processor available
            info!("[AUDIO] STT not available, returning to idle state");
            self.state.finish_processing();
        }
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
                    stt_phase: None, // TODO: expose phase from STT processor
                    stt_speech_chunks: 0,
                    stt_has_transcription: self.has_transcription,
                    stt_has_first_word: self.has_first_word,
                    stt_last_transcription: self.last_transcription.clone(),
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
        // Increment frame counter and check for frame-limited exit
        {
            let mut state = self.shared_state.write();
            state.frame_count += 1;

            // Check for frame-limited exit in debug mode
            if state.max_frames > 0 && state.frame_count >= state.max_frames {
                info!("[DEBUG] Reached max frames ({}), exiting", state.max_frames);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // Initialize on first frame
        self.initialize();

        // Process audio data
        self.process_audio();

        // Process STT events
        self.process_stt_events();

        // Process test commands (if in test mode)
        self.process_test_commands(ctx);

        // Sync local state to shared state for debug panel
        self.sync_shared_state();

        // Request repaint continuously if in test mode, debug mode with max_frames, or processing
        if self.test_runner.is_some()
            || self.state.is_processing()
            || self.debug_config.as_ref().is_some_and(|d| d.max_frames > 0)
        {
            ctx.request_repaint();
        }

        // Debug panel in a side panel (right side)
        egui::SidePanel::right("debug_panel")
            .resizable(true)
            .default_width(280.0)
            .show_animated(ctx, self.debug_panel_open, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    DebugPanel::new(&self.shared_state, &self.theme).show(ui);
                });
            });

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

                // Waveform visualization (always visible)
                ui.add_space(20.0);
                StateWaveform::new(&self.state, &self.theme)
                    .height(60.0)
                    .show(ui);
                ui.add_space(20.0);

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

                ui.add_space(20.0);

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

                // Show last transcription
                if let Some(ref transcription) = self.last_transcription {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(format!("\"{}\"", transcription))
                            .size(16.0)
                            .color(self.theme.text_secondary),
                    );
                }

                // Keyboard hint
                ui.add_space(20.0);
                ui.label(
                    RichText::new("Press Space or click to toggle recording")
                        .size(12.0)
                        .color(self.theme.text_muted.gamma_multiply(0.7)),
                );

                // STT status
                if self.stt_processor.is_none() {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("(STT disabled - no Whisper model found)")
                            .size(11.0)
                            .color(self.theme.warning.gamma_multiply(0.8)),
                    );
                }

                // Debug panel toggle at the bottom
                ui.add_space(30.0);
                ui.separator();
                ui.add_space(8.0);
                let debug_label = if self.debug_panel_open {
                    "Hide Debug"
                } else {
                    "Show Debug"
                };
                if ui.small_button(debug_label).clicked() {
                    self.debug_panel_open = !self.debug_panel_open;
                }
            });
        });
    }
}

impl Drop for ProtoApp {
    fn drop(&mut self) {
        // Shutdown STT processor gracefully
        if let Some(ref processor) = self.stt_processor {
            let _ = processor.shutdown();
        }

        // Wait for worker to finish
        if let Some(handle) = self.stt_worker_handle.take() {
            let _ = handle.join();
        }
    }
}
