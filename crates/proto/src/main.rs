//! Proto - Voice-controlled LLM assistant
//!
//! Main entry point for the Proto application.

use eframe::egui;
use proto::processor::{Orchestrator, OrchestratorConfig};
use proto::state::SharedAppState;
use proto::testconfig::TestConfig;
use proto::ui::{DebugConfig, ProtoApp};
use std::env;
use std::thread::JoinHandle;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Command line arguments for Proto
struct Args {
    /// Path to test configuration file
    test_config: Option<String>,
    /// Debug mode enabled
    debug_mode: bool,
    /// Max frames before exit (0 = unlimited)
    max_frames: u64,
}

impl Args {
    fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut test_config = None;
        let mut debug_mode = false;
        let mut max_frames: u64 = 0;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--test" => {
                    if i + 1 < args.len() {
                        test_config = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: --test requires a path to a test config file");
                        std::process::exit(1);
                    }
                }
                "--debug" => {
                    debug_mode = true;
                    // Check if next arg is a number (optional max_frames)
                    if i + 1 < args.len() {
                        if let Ok(n) = args[i + 1].parse::<u64>() {
                            max_frames = n;
                            i += 2;
                            continue;
                        }
                    }
                    i += 1;
                }
                "-h" | "--help" => {
                    println!("Proto - Voice-controlled LLM assistant");
                    println!();
                    println!("USAGE:");
                    println!("    proto [OPTIONS]");
                    println!();
                    println!("OPTIONS:");
                    println!("    --test <FILE>    Run predefined tests from a TOML config file");
                    println!("    --debug [FRAMES] Enable debug mode, optionally exit after FRAMES frames");
                    println!("    -h, --help       Print this help message");
                    std::process::exit(0);
                }
                other => {
                    eprintln!("Error: Unknown argument '{}'", other);
                    std::process::exit(1);
                }
            }
        }

        Self {
            test_config,
            debug_mode,
            max_frames,
        }
    }
}

fn main() -> eframe::Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "proto=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Proto voice assistant");

    // Load test configuration if specified
    let test_config = if let Some(path) = args.test_config {
        tracing::info!("Loading test configuration from: {}", path);
        match TestConfig::load(&path) {
            Ok(config) => {
                tracing::info!(
                    "Test configuration loaded: {} ({} actions)",
                    config.test.name,
                    config.actions.len()
                );
                Some(config)
            }
            Err(e) => {
                tracing::error!("Failed to load test configuration: {}", e);
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    // Create debug config from arguments
    let debug_config = if args.debug_mode {
        tracing::info!(
            "Debug mode enabled{}",
            if args.max_frames > 0 {
                format!(", will exit after {} frames", args.max_frames)
            } else {
                String::new()
            }
        );
        Some(DebugConfig {
            enabled: true,
            max_frames: args.max_frames,
        })
    } else {
        None
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("Proto"),
        ..Default::default()
    };

    // Create shared state and orchestrator
    let shared_state = SharedAppState::new();
    let orchestrator_config = OrchestratorConfig::default();

    // Create orchestrator with shared state
    let orchestrator_setup = match Orchestrator::with_state(orchestrator_config, shared_state.clone()) {
        Ok((orchestrator, handle)) => {
            // Start orchestrator worker threads
            match orchestrator.start() {
                Ok(handles) => {
                    tracing::info!("Orchestrator started with {} worker threads", handles.len());
                    // Store handles for cleanup (they'll be dropped when main exits)
                    // We leak the handles intentionally - they'll be cleaned up on process exit
                    let _: Vec<JoinHandle<()>> = handles;
                    Some((shared_state, handle))
                }
                Err(e) => {
                    tracing::error!("Failed to start orchestrator: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to create orchestrator: {}", e);
            None
        }
    };

    eframe::run_native(
        "Proto",
        options,
        Box::new(move |cc| Ok(Box::new(ProtoApp::with_orchestrator(cc, test_config, debug_config, orchestrator_setup)))),
    )
}
