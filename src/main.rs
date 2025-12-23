use anyhow::{Context, Result};
use clap::Parser;
use rodio::{Decoder, OutputStream, Sink};
use serde::Serialize;
use std::io::{self, BufRead, Cursor, Read, Write};
use std::time::Duration;

/// Text-to-speech CLI using Chatterbox TTS
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Text to synthesize (if not provided, reads from stdin)
    #[arg(short, long)]
    text: Option<String>,

    /// Server URL
    #[arg(short, long, default_value = "http://127.0.0.1:8787")]
    server: String,

    /// Path to reference audio for voice cloning
    #[arg(short, long)]
    audio_prompt: Option<String>,

    /// Exaggeration factor (0.0-1.0, for original model)
    #[arg(short, long, default_value = "0.5")]
    exaggeration: f32,

    /// CFG weight (0.0-1.0, for original model)
    #[arg(short, long, default_value = "0.5")]
    cfg_weight: f32,

    /// Save audio to file instead of playing
    #[arg(short, long)]
    output: Option<String>,

    /// Read input line by line from stdin
    #[arg(long)]
    interactive: bool,
}

#[derive(Serialize)]
struct SynthesizeRequest {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_prompt: Option<String>,
    exaggeration: f32,
    cfg_weight: f32,
}

fn synthesize(
    client: &reqwest::blocking::Client,
    server: &str,
    request: &SynthesizeRequest,
) -> Result<Vec<u8>> {
    let url = format!("{}/synthesize", server);

    let mut response = client
        .post(&url)
        .json(request)
        .timeout(Duration::from_secs(120))
        .send()
        .context("Failed to connect to TTS server")?;

    if !response.status().is_success() {
        let mut body = String::new();
        response.read_to_string(&mut body).ok();
        anyhow::bail!("Server error: {}", body);
    }

    let mut audio_bytes = Vec::new();
    response
        .read_to_end(&mut audio_bytes)
        .context("Failed to read audio data")?;

    Ok(audio_bytes)
}

fn play_audio(audio_bytes: &[u8]) -> Result<()> {
    let (_stream, stream_handle) =
        OutputStream::try_default().context("Failed to open audio output")?;

    let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

    let cursor = Cursor::new(audio_bytes.to_vec());
    let source = Decoder::new(cursor).context("Failed to decode audio")?;

    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

fn save_audio(audio_bytes: &[u8], path: &str) -> Result<()> {
    std::fs::write(path, audio_bytes).context("Failed to write audio file")?;
    println!("Saved audio to: {}", path);
    Ok(())
}

fn check_server(client: &reqwest::blocking::Client, server: &str) -> Result<()> {
    let url = format!("{}/health", server);
    client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .context("TTS server is not running. Start it with: python server.py")?;
    Ok(())
}

fn process_text(client: &reqwest::blocking::Client, args: &Args, text: &str) -> Result<()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }

    println!("Synthesizing: \"{}\"", text);

    let request = SynthesizeRequest {
        text: text.to_string(),
        audio_prompt: args.audio_prompt.clone(),
        exaggeration: args.exaggeration,
        cfg_weight: args.cfg_weight,
    };

    let audio_bytes = synthesize(client, &args.server, &request)?;

    if let Some(ref output_path) = args.output {
        save_audio(&audio_bytes, output_path)?;
    } else {
        play_audio(&audio_bytes)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let client = reqwest::blocking::Client::new();

    // Check if server is running
    check_server(&client, &args.server)?;

    if args.interactive {
        // Interactive mode: read lines from stdin
        println!("Interactive mode. Enter text to synthesize (Ctrl+D to exit):");
        print!("> ");
        io::stdout().flush()?;

        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            if let Err(e) = process_text(&client, &args, &line) {
                eprintln!("Error: {}", e);
            }
            print!("> ");
            io::stdout().flush()?;
        }
        println!();
    } else if let Some(ref text) = args.text {
        // Text provided via argument
        process_text(&client, &args, text)?;
    } else {
        // Read all text from stdin
        let mut text = String::new();
        io::stdin()
            .read_line(&mut text)
            .context("Failed to read from stdin")?;
        process_text(&client, &args, &text)?;
    }

    Ok(())
}
