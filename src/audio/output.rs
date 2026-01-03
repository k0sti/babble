use crate::{BabbleError, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use crossbeam_channel::Receiver;
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{debug, error, info, warn};

pub struct AudioOutput {
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    is_playing: Arc<Mutex<bool>>,
}

impl AudioOutput {
    /// Create a new audio output with the default output device
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or_else(|| BabbleError::AudioDeviceError("No output device available".into()))?;

        info!("Using output device: {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));

        let config = device
            .default_output_config()
            .map_err(|e| BabbleError::AudioDeviceError(format!("Failed to get output config: {}", e)))?
            .into();

        Ok(Self {
            device,
            config,
            stream: None,
            is_playing: Arc::new(Mutex::new(false)),
        })
    }

    /// Get the sample rate of the output device
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }

    /// Get the number of channels
    pub fn channels(&self) -> u16 {
        self.config.channels
    }

    /// Start playing audio from the provided channel
    pub fn start_playback(&mut self, audio_rx: Receiver<Vec<f32>>) -> Result<()> {
        if *self.is_playing.lock() {
            warn!("Already playing");
            return Ok(());
        }

        let channels = self.config.channels as usize;
        let is_playing = Arc::clone(&self.is_playing);
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = Arc::clone(&buffer);

        // Spawn a thread to receive audio data
        std::thread::spawn(move || {
            while let Ok(samples) = audio_rx.recv() {
                let mut buf = buffer_clone.lock();
                buf.extend_from_slice(&samples);
            }
        });

        let err_fn = |err| {
            error!("Audio output stream error: {}", err);
        };

        let stream = self
            .device
            .build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !*is_playing.lock() {
                        // Fill with silence
                        data.fill(0.0);
                        return;
                    }

                    let mut buf = buffer.lock();
                    let samples_needed = data.len() / channels;
                    let samples_available = buf.len().min(samples_needed);

                    if samples_available > 0 {
                        // Fill the output buffer
                        for i in 0..samples_available {
                            let sample = buf[i];
                            for c in 0..channels {
                                data[i * channels + c] = sample;
                            }
                        }

                        // Remove used samples
                        buf.drain(0..samples_available);

                        // Fill the rest with silence if needed
                        for i in (samples_available * channels)..data.len() {
                            data[i] = 0.0;
                        }
                    } else {
                        // No samples available, fill with silence
                        data.fill(0.0);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| BabbleError::AudioDeviceError(format!("Failed to build output stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| BabbleError::AudioDeviceError(format!("Failed to start output stream: {}", e)))?;

        *self.is_playing.lock() = true;
        self.stream = Some(stream);

        info!("Started audio playback");
        Ok(())
    }

    /// Stop playing audio
    pub fn stop_playback(&mut self) -> Result<()> {
        *self.is_playing.lock() = false;

        if let Some(stream) = self.stream.take() {
            drop(stream);
            info!("Stopped audio playback");
        }

        Ok(())
    }

    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock()
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        let _ = self.stop_playback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    #[test]
    fn test_audio_output_creation() {
        // This test might fail in CI environments without audio devices
        if let Ok(output) = AudioOutput::new() {
            assert!(output.sample_rate() > 0);
            assert!(output.channels() > 0);
        }
    }

    #[test]
    fn test_playback_state() {
        if let Ok(mut output) = AudioOutput::new() {
            assert!(!output.is_playing());

            let (_tx, rx) = bounded(10);
            if output.start_playback(rx).is_ok() {
                assert!(output.is_playing());

                let _ = output.stop_playback();
                assert!(!output.is_playing());
            }
        }
    }
}
