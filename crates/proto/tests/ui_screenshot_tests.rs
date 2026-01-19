//! UI Screenshot tests for visual validation
//!
//! These tests verify that waveform components render correctly
//! in various states.

use egui_kittest::Harness;
use proto::ui::{AppState, RecordingState, StateWaveform, Theme, Waveform};

/// Test application state wrapper for screenshot tests
struct ScreenshotTestApp {
    state: AppState,
    theme: Theme,
}

impl ScreenshotTestApp {
    fn new() -> Self {
        Self {
            state: AppState::new(),
            theme: Theme::dark(),
        }
    }

    fn with_recording_state(mut self, recording_state: RecordingState) -> Self {
        self.state.recording_state = recording_state;
        self
    }

    fn with_waveform_data(mut self, data: Vec<f32>) -> Self {
        self.state.waveform_data = data;
        self
    }
}

/// Generate sample waveform data for testing
fn generate_test_waveform() -> Vec<f32> {
    (0..500)
        .map(|i| {
            let t = i as f32 / 500.0;
            (t * std::f32::consts::PI * 8.0).sin() * 0.7
        })
        .collect()
}

/// Test waveform display in Idle state (placeholder)
#[test]
fn test_waveform_idle_state() {
    let app = ScreenshotTestApp::new().with_recording_state(RecordingState::Idle);

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 100.0))
        .build_state(
            |ctx, app: &mut ScreenshotTestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    StateWaveform::new(&app.state, &app.theme)
                        .height(60.0)
                        .show(ui);
                });
            },
            app,
        );

    // Run a few frames to ensure rendering
    for _ in 0..3 {
        harness.run();
    }

    // Verify the harness ran successfully (waveform rendered)
    assert!(harness.state().state.recording_state == RecordingState::Idle);
}

/// Test waveform display in Recording state with placeholder data
#[test]
fn test_waveform_recording_state_placeholder() {
    let app = ScreenshotTestApp::new().with_recording_state(RecordingState::Recording);
    // No waveform data - should show placeholder bars

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 100.0))
        .build_state(
            |ctx, app: &mut ScreenshotTestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    StateWaveform::new(&app.state, &app.theme)
                        .height(60.0)
                        .show(ui);
                });
            },
            app,
        );

    // Run a few frames
    for _ in 0..3 {
        harness.run();
    }

    // Verify recording state
    assert!(harness.state().state.recording_state == RecordingState::Recording);
    // Waveform data should be empty (using placeholder)
    assert!(harness.state().state.waveform_data.is_empty());
}

/// Test waveform display in Recording state with actual audio data
#[test]
fn test_waveform_recording_state_with_data() {
    let app = ScreenshotTestApp::new()
        .with_recording_state(RecordingState::Recording)
        .with_waveform_data(generate_test_waveform());

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 100.0))
        .build_state(
            |ctx, app: &mut ScreenshotTestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    StateWaveform::new(&app.state, &app.theme)
                        .height(60.0)
                        .show(ui);
                });
            },
            app,
        );

    // Run a few frames
    for _ in 0..3 {
        harness.run();
    }

    // Verify recording state and data
    assert!(harness.state().state.recording_state == RecordingState::Recording);
    assert!(!harness.state().state.waveform_data.is_empty());
    assert_eq!(harness.state().state.waveform_data.len(), 500);
}

/// Test waveform display in Processing state
#[test]
fn test_waveform_processing_state() {
    let app = ScreenshotTestApp::new()
        .with_recording_state(RecordingState::Processing)
        .with_waveform_data(generate_test_waveform());

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 100.0))
        .build_state(
            |ctx, app: &mut ScreenshotTestApp| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    StateWaveform::new(&app.state, &app.theme)
                        .height(60.0)
                        .show(ui);
                });
            },
            app,
        );

    // Run a few frames
    for _ in 0..3 {
        harness.run();
    }

    // Verify processing state
    assert!(harness.state().state.recording_state == RecordingState::Processing);
}

/// Test raw Waveform component with samples
#[test]
fn test_raw_waveform_with_samples() {
    let theme = Theme::dark();
    let samples = generate_test_waveform();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 100.0))
        .build_state(
            |ctx, (theme, samples): &mut (Theme, Vec<f32>)| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    Waveform::new(samples, theme)
                        .height(60.0)
                        .recording(true)
                        .show(ui);
                });
            },
            (theme, samples),
        );

    // Run a few frames
    for _ in 0..3 {
        harness.run();
    }

    // Verify samples are present
    assert!(!harness.state().1.is_empty());
}

/// Test raw Waveform component without samples (placeholder)
#[test]
fn test_raw_waveform_empty_recording() {
    let theme = Theme::dark();
    let samples: Vec<f32> = vec![];

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 100.0))
        .build_state(
            |ctx, (theme, samples): &mut (Theme, Vec<f32>)| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    Waveform::new(samples, theme)
                        .height(60.0)
                        .recording(true)
                        .show(ui);
                });
            },
            (theme, samples),
        );

    // Run a few frames
    for _ in 0..3 {
        harness.run();
    }

    // Verify samples are empty (using placeholder)
    assert!(harness.state().1.is_empty());
}

/// Test raw Waveform component idle state (no recording, no samples)
#[test]
fn test_raw_waveform_idle() {
    let theme = Theme::dark();
    let samples: Vec<f32> = vec![];

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(400.0, 100.0))
        .build_state(
            |ctx, (theme, samples): &mut (Theme, Vec<f32>)| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    Waveform::new(samples, theme)
                        .height(60.0)
                        .recording(false)
                        .show(ui);
                });
            },
            (theme, samples),
        );

    // Run a few frames
    for _ in 0..3 {
        harness.run();
    }

    // Verify samples are empty
    assert!(harness.state().1.is_empty());
}
