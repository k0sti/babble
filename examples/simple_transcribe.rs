use babble::speech::stt::{AudioSegment, WhisperConfig, WhisperEngine};
use std::path::PathBuf;
use tracing::{info, Level};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <model-path>", args[0]);
        eprintln!("\nExample:");
        eprintln!("  cargo run --example simple_transcribe -- models/ggml-base.en.bin");
        std::process::exit(1);
    }

    let model_path = PathBuf::from(&args[1]);
    info!("=== Simple Whisper Test ===");
    info!("Loading model: {:?}", model_path);

    let config = WhisperConfig {
        model_path,
        language: Some("en".to_string()),
        n_threads: 4,
        ..Default::default()
    };

    let engine = WhisperEngine::new(config)?;
    info!("Model loaded successfully!");

    let samples = vec![0.0f32; 16000];
    let segment = AudioSegment::new(samples, true, 0.0);

    info!("Transcribing test audio...");
    match engine.transcribe(&segment) {
        Ok(result) => {
            info!("Transcription result: '{}'", result.text);
            info!("Duration: {:.2}s - {:.2}s", result.start_time, result.end_time);
        }
        Err(e) => eprintln!("Transcription failed: {}", e),
    }

    info!("✅ Test complete!");
    Ok(())
}
