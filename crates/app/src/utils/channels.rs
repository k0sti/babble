//! Channel management for inter-component communication
//!
//! Provides typed channels for audio, transcription, LLM, and TTS pipelines.

use crate::llm::{LLMCommand, LLMEvent};
use crate::messages::AudioData;
use crate::speech::tts::{TTSCommand, TTSEvent};
use crossbeam_channel::{bounded, Receiver, Sender};

/// Channels for audio data flow
pub struct AudioChannels {
    /// Raw audio samples from microphone
    pub raw_audio_tx: Sender<Vec<f32>>,
    pub raw_audio_rx: Receiver<Vec<f32>>,

    /// Processed audio data (resampled, normalized)
    pub processed_audio_tx: Sender<AudioData>,
    pub processed_audio_rx: Receiver<AudioData>,

    /// Audio data for playback
    pub playback_tx: Sender<AudioData>,
    pub playback_rx: Receiver<AudioData>,
}

impl AudioChannels {
    /// Create new audio channels with specified buffer size
    pub fn new(buffer_size: usize) -> Self {
        let (raw_audio_tx, raw_audio_rx) = bounded(buffer_size);
        let (processed_audio_tx, processed_audio_rx) = bounded(buffer_size);
        let (playback_tx, playback_rx) = bounded(buffer_size);

        Self {
            raw_audio_tx,
            raw_audio_rx,
            processed_audio_tx,
            processed_audio_rx,
            playback_tx,
            playback_rx,
        }
    }
}

/// Channels for text and AI processing
pub struct ProcessingChannels {
    /// Transcribed text from STT
    pub transcription_tx: Sender<String>,
    pub transcription_rx: Receiver<String>,

    /// Commands to LLM pipeline
    pub llm_command_tx: Sender<LLMCommand>,
    pub llm_command_rx: Receiver<LLMCommand>,

    /// Events from LLM pipeline
    pub llm_event_tx: Sender<LLMEvent>,
    pub llm_event_rx: Receiver<LLMEvent>,

    /// Commands to TTS pipeline
    pub tts_command_tx: Sender<TTSCommand>,
    pub tts_command_rx: Receiver<TTSCommand>,

    /// Events from TTS pipeline
    pub tts_event_tx: Sender<TTSEvent>,
    pub tts_event_rx: Receiver<TTSEvent>,
}

impl ProcessingChannels {
    /// Create new processing channels with specified buffer size
    pub fn new(buffer_size: usize) -> Self {
        let (transcription_tx, transcription_rx) = bounded(buffer_size);
        let (llm_command_tx, llm_command_rx) = bounded(buffer_size);
        let (llm_event_tx, llm_event_rx) = bounded(buffer_size);
        let (tts_command_tx, tts_command_rx) = bounded(buffer_size);
        let (tts_event_tx, tts_event_rx) = bounded(buffer_size);

        Self {
            transcription_tx,
            transcription_rx,
            llm_command_tx,
            llm_command_rx,
            llm_event_tx,
            llm_event_rx,
            tts_command_tx,
            tts_command_rx,
            tts_event_tx,
            tts_event_rx,
        }
    }
}

/// All channels used by the Babble application
pub struct BabbleChannels {
    /// Audio input/output channels
    pub audio: AudioChannels,

    /// Processing pipeline channels
    pub processing: ProcessingChannels,
}

impl BabbleChannels {
    /// Create a new set of channels with default buffer sizes
    pub fn new() -> Self {
        Self::with_buffer_size(10)
    }

    /// Create channels with custom buffer size
    pub fn with_buffer_size(buffer_size: usize) -> Self {
        Self {
            audio: AudioChannels::new(buffer_size),
            processing: ProcessingChannels::new(buffer_size),
        }
    }
}

impl Default for BabbleChannels {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_audio_channels() {
        let channels = AudioChannels::new(10);

        // Test raw audio channel
        channels.raw_audio_tx.send(vec![0.0, 0.1, 0.2]).unwrap();
        let received = channels.raw_audio_rx.recv().unwrap();
        assert_eq!(received, vec![0.0, 0.1, 0.2]);
    }

    #[test]
    fn test_llm_channels() {
        let channels = ProcessingChannels::new(10);

        // Test LLM command channel
        let cmd = LLMCommand::Generate {
            user_message: "Hello".to_string(),
            request_id: Uuid::new_v4(),
        };
        channels.llm_command_tx.send(cmd).unwrap();

        match channels.llm_command_rx.recv().unwrap() {
            LLMCommand::Generate { user_message, .. } => {
                assert_eq!(user_message, "Hello");
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_babble_channels() {
        let channels = BabbleChannels::new();

        // Verify all channels are accessible
        let _ = &channels.audio.raw_audio_tx;
        let _ = &channels.processing.llm_command_tx;
        let _ = &channels.processing.tts_command_tx;
        let _ = &channels.processing.tts_event_tx;
    }

    #[test]
    fn test_custom_buffer_size() {
        let channels = BabbleChannels::with_buffer_size(50);

        // Fill up to buffer size without blocking
        for i in 0..50 {
            channels.audio.raw_audio_tx.send(vec![i as f32]).unwrap();
        }
    }
}
