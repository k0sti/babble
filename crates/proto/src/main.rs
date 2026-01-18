//! Proto - Voice-controlled LLM assistant
//!
//! Main entry point for the Proto application.

use eframe::egui;
use proto::ui::ProtoApp;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "proto=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Proto voice assistant");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("Proto"),
        ..Default::default()
    };

    eframe::run_native(
        "Proto",
        options,
        Box::new(|cc| Ok(Box::new(ProtoApp::new(cc)))),
    )
}
