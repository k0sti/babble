use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SayParams {
    /// The text to speak
    pub text: String,
}

#[derive(Serialize)]
struct SynthesizeRequest {
    text: String,
}

#[derive(Clone)]
pub struct BabbleMcp {
    server_url: String,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl BabbleMcp {
    pub fn new() -> Self {
        Self {
            server_url: std::env::var("BABBLE_SERVER")
                .unwrap_or_else(|_| "http://127.0.0.1:8787".to_string()),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(name = "say", description = "Convert text to speech and play it aloud")]
    pub async fn say(&self, params: Parameters<SayParams>) -> Result<String, String> {
        self.speak(&params.0.text, true).await
    }

    #[tool(
        name = "say_async",
        description = "Convert text to speech and play in background (returns immediately)"
    )]
    pub async fn say_async(&self, params: Parameters<SayParams>) -> Result<String, String> {
        self.speak(&params.0.text, false).await
    }
}

impl BabbleMcp {
    async fn speak(&self, text: &str, wait: bool) -> Result<String, String> {
        let server_url = self.server_url.clone();
        let text = text.to_string();
        let text_clone = text.clone();

        let handle = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let text = text_clone;
            use std::io::Read;

            let client = reqwest::blocking::Client::new();

            // Check server health
            let health_url = format!("{}/health", server_url);
            client
                .get(&health_url)
                .timeout(Duration::from_secs(5))
                .send()
                .map_err(|_| "TTS server is not running. Start it with: just serve".to_string())?;

            // Synthesize
            let url = format!("{}/synthesize", server_url);
            let request = SynthesizeRequest { text: text.clone() };

            let mut response = client
                .post(&url)
                .json(&request)
                .timeout(Duration::from_secs(120))
                .send()
                .map_err(|e| format!("Failed to connect to TTS server: {}", e))?;

            if !response.status().is_success() {
                let mut body = String::new();
                response.read_to_string(&mut body).ok();
                return Err(format!("Server error: {}", body));
            }

            let mut audio_bytes = Vec::new();
            response
                .read_to_end(&mut audio_bytes)
                .map_err(|e| format!("Failed to read audio: {}", e))?;

            // Play audio
            use rodio::{Decoder, OutputStream, Sink};
            use std::io::Cursor;

            let (_stream, stream_handle) =
                OutputStream::try_default().map_err(|e| format!("Failed to open audio: {}", e))?;

            let sink = Sink::try_new(&stream_handle)
                .map_err(|e| format!("Failed to create sink: {}", e))?;

            let cursor = Cursor::new(audio_bytes);
            let source =
                Decoder::new(cursor).map_err(|e| format!("Failed to decode audio: {}", e))?;

            sink.append(source);
            sink.sleep_until_end();

            Ok(format!("Spoke: \"{}\"", text))
        });

        if wait {
            handle.await.map_err(|e| format!("Task failed: {}", e))?
        } else {
            // Don't wait, let it play in background
            tokio::spawn(async move {
                let _ = handle.await;
            });
            Ok(format!("Speaking: \"{}\"", text))
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for BabbleMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Text-to-speech server using Chatterbox TTS".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = BabbleMcp::new();
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
