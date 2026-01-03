use crossbeam_channel::{bounded, Receiver, Sender};
use crate::messages::AudioData;

pub struct AudioChannels {
    pub raw_audio_tx: Sender<Vec<f32>>,
    pub raw_audio_rx: Receiver<Vec<f32>>,
    pub processed_audio_tx: Sender<AudioData>,
    pub processed_audio_rx: Receiver<AudioData>,
    pub playback_tx: Sender<AudioData>,
    pub playback_rx: Receiver<AudioData>,
}

impl AudioChannels {
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

pub struct ProcessingChannels {
    pub transcription_tx: Sender<String>,
    pub transcription_rx: Receiver<String>,
    pub llm_request_tx: Sender<String>,
    pub llm_request_rx: Receiver<String>,
    pub llm_response_tx: Sender<String>,
    pub llm_response_rx: Receiver<String>,
    pub tts_request_tx: Sender<String>,
    pub tts_request_rx: Receiver<String>,
}

impl ProcessingChannels {
    pub fn new(buffer_size: usize) -> Self {
        let (transcription_tx, transcription_rx) = bounded(buffer_size);
        let (llm_request_tx, llm_request_rx) = bounded(buffer_size);
        let (llm_response_tx, llm_response_rx) = bounded(buffer_size);
        let (tts_request_tx, tts_request_rx) = bounded(buffer_size);

        Self {
            transcription_tx,
            transcription_rx,
            llm_request_tx,
            llm_request_rx,
            llm_response_tx,
            llm_response_rx,
            tts_request_tx,
            tts_request_rx,
        }
    }
}

pub struct BabbleChannels {
    pub audio: AudioChannels,
    pub processing: ProcessingChannels,
}

impl BabbleChannels {
    pub fn new() -> Self {
        Self {
            audio: AudioChannels::new(10),
            processing: ProcessingChannels::new(10),
        }
    }
}

impl Default for BabbleChannels {
    fn default() -> Self {
        Self::new()
    }
}
