//! GUI implementation with egui/eframe
//!
//! This module provides the desktop user interface for Babble using the eframe framework.

mod app;
mod components;
mod state;
mod theme;

pub use app::BabbleApp;
pub use state::{AppState, RecordingState, PlaybackState};
pub use theme::Theme;

/// Run the Babble application
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("Babble Voice Assistant"),
        ..Default::default()
    };

    eframe::run_native(
        "Babble",
        options,
        Box::new(|cc| Ok(Box::new(BabbleApp::new(cc)))),
    )
}
