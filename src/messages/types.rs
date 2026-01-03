use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Sender {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioData {
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channels,
        }
    }

    pub fn duration_seconds(&self) -> f32 {
        self.samples.len() as f32 / (self.sample_rate as f32 * self.channels as f32)
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub data: Vec<u8>,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    pub name: String,
    pub data: Vec<u8>,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    Audio(AudioData),
    Image(ImageData),
    File(FileData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub is_speech: bool,
    pub processing_time_ms: Option<u64>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            is_speech: false,
            processing_time_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub sender: Sender,
    pub content: MessageContent,
    pub timestamp: DateTime<Utc>,
    pub metadata: MessageMetadata,
}

impl Message {
    pub fn new(sender: Sender, content: MessageContent) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender,
            content,
            timestamp: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    pub fn with_metadata(mut self, metadata: MessageMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}
