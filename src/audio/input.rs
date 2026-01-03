use crate::{BabbleError, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use crossbeam_channel::Sender;
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{debug, error, info, warn};

pub struct AudioInput {
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    is_recording: Arc<Mutex<bool>>,
}

impl AudioInput {
    /// Create a new audio input with the default input device
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or_else(|| BabbleError::AudioDeviceError("No input device available".into()))?;

        info!("Using input device: {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));

        let config = device
            .default_input_config()
            .map_err(|e| BabbleError::AudioDeviceError(format!("Failed to get input config: {}", e)))?
            .into();

        Ok(Self {
            device,
            config,
            stream: None,
            is_recording: Arc::new(Mutex::new(false)),
        })
    }

    /// Get the sample rate of the input device
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }

    /// Get the number of channels
    pub fn channels(&self) -> u16 {
        self.config.channels
    }

    /// Start recording audio and send samples to the provided channel
    pub fn start_recording(&mut self, audio_tx: Sender<Vec<f32>>) -> Result<()> {
        if *self.is_recording.lock() {
            warn!("Already recording");
            return Ok(());
        }

        let channels = self.config.channels as usize;
        let is_recording = Arc::clone(&self.is_recording);

        let err_fn = |err| {
            error!("Audio input stream error: {}", err);
        };

        let stream = self
            .device
            .build_input_stream(
                &self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !*is_recording.lock() {
                        return;
                    }

                    // Convert to mono if necessary
                    let samples = if channels == 1 {
                        data.to_vec()
                    } else {
                        // Average all channels to create mono
                        data.chunks(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                            .collect()
                    };

                    if let Err(e) = audio_tx.try_send(samples) {
                        debug!("Failed to send audio data: {}", e);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| BabbleError::AudioDeviceError(format!("Failed to build input stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| BabbleError::AudioDeviceError(format!("Failed to start input stream: {}", e)))?;

        *self.is_recording.lock() = true;
        self.stream = Some(stream);

        info!("Started audio recording");
        Ok(())
    }

    /// Stop recording audio
    pub fn stop_recording(&mut self) -> Result<()> {
        *self.is_recording.lock() = false;

        if let Some(stream) = self.stream.take() {
            drop(stream);
            info!("Stopped audio recording");
        }

        Ok(())
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock()
    }
}

impl Drop for AudioInput {
    fn drop(&mut self) {
        let _ = self.stop_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    #[test]
    fn test_audio_input_creation() {
        // This test might fail in CI environments without audio devices
        if let Ok(input) = AudioInput::new() {
            assert!(input.sample_rate() > 0);
            assert!(input.channels() > 0);
        }
    }

    #[test]
    fn test_recording_state() {
        if let Ok(mut input) = AudioInput::new() {
            assert!(!input.is_recording());

            let (tx, _rx) = bounded(10);
            if input.start_recording(tx).is_ok() {
                assert!(input.is_recording());

                let _ = input.stop_recording();
                assert!(!input.is_recording());
            }
        }
    }
}
