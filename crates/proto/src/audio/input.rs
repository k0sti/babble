//! Microphone audio recording module
//!
//! Provides cross-platform audio input capture using cpal,
//! with automatic mono conversion and channel-based output.

use crate::error::{ProtoError, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Audio input device information
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    /// Device name
    pub name: String,
    /// Whether this is the default input device
    pub is_default: bool,
}

/// Audio recorder for capturing microphone input
///
/// Captures audio from the default input device and sends samples
/// via a crossbeam channel for processing.
pub struct AudioRecorder {
    stream: Option<Stream>,
    sample_rate: u32,
    channels: u16,
    is_recording: Arc<AtomicBool>,
    device: Device,
    config: StreamConfig,
}

impl AudioRecorder {
    /// Create a new audio recorder with the default input device
    ///
    /// # Errors
    /// Returns an error if no input device is available or configuration fails
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();

        let device = host.default_input_device().ok_or_else(|| {
            ProtoError::AudioDeviceError("No input device available".into())
        })?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Using input device: {}", device_name);

        let supported_config = device.default_input_config().map_err(|e| {
            ProtoError::AudioDeviceError(format!("Failed to get input config: {}", e))
        })?;

        let config: StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;

        info!(
            "Audio config: {}Hz, {} channel(s)",
            sample_rate, channels
        );

        Ok(Self {
            stream: None,
            sample_rate,
            channels,
            is_recording: Arc::new(AtomicBool::new(false)),
            device,
            config,
        })
    }

    /// Start recording audio
    ///
    /// Audio samples are sent as `Vec<f32>` through the provided channel.
    /// Stereo audio is automatically converted to mono.
    ///
    /// # Arguments
    /// * `audio_tx` - Channel sender for audio sample chunks
    ///
    /// # Errors
    /// Returns an error if the stream cannot be built or started
    pub fn start(&mut self, audio_tx: Sender<Vec<f32>>) -> Result<()> {
        if self.is_recording.load(Ordering::SeqCst) {
            warn!("Already recording, ignoring start request");
            return Ok(());
        }

        let channels = self.channels as usize;
        let sample_rate = self.sample_rate;
        let is_recording = Arc::clone(&self.is_recording);

        // Sample counter for debug logging
        let sample_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sample_count_clone = Arc::clone(&sample_count);

        let err_fn = |err| {
            error!("Audio input stream error: {}", err);
        };

        info!(
            "Building audio input stream: {}Hz, {} channel(s)",
            sample_rate, channels
        );

        let stream = self
            .device
            .build_input_stream(
                &self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !is_recording.load(Ordering::SeqCst) {
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

                    let count = sample_count_clone
                        .fetch_add(samples.len(), Ordering::Relaxed);

                    // Log approximately every second of audio
                    if count % (sample_rate as usize) < samples.len() {
                        debug!(
                            "Audio captured: {} samples ({:.1}s)",
                            count + samples.len(),
                            (count + samples.len()) as f32 / sample_rate as f32
                        );
                    }

                    if let Err(e) = audio_tx.try_send(samples) {
                        warn!("Failed to send audio data: {}", e);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| {
                ProtoError::AudioDeviceError(format!("Failed to build input stream: {}", e))
            })?;

        stream.play().map_err(|e| {
            ProtoError::AudioDeviceError(format!("Failed to start input stream: {}", e))
        })?;

        self.is_recording.store(true, Ordering::SeqCst);
        self.stream = Some(stream);

        info!("Audio recording started");
        Ok(())
    }

    /// Stop recording audio
    ///
    /// # Errors
    /// Currently always succeeds, but returns Result for API consistency
    pub fn stop(&mut self) -> Result<()> {
        self.is_recording.store(false, Ordering::SeqCst);

        if let Some(stream) = self.stream.take() {
            drop(stream);
            info!("Audio recording stopped");
        }

        Ok(())
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Get the sample rate in Hz
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of input channels
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// List available audio input devices
///
/// # Returns
/// A vector of device information for all available input devices
pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_device_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    host.input_devices()
        .map(|devices| {
            devices
                .filter_map(|device| {
                    let name = device.name().ok()?;
                    let is_default = default_device_name
                        .as_ref()
                        .map(|d| d == &name)
                        .unwrap_or(false);
                    Some(AudioDeviceInfo { name, is_default })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    #[test]
    fn test_list_devices() {
        // Should not panic even without audio devices
        let devices = list_input_devices();
        // In CI, might be empty; on dev machines, should have at least one
        println!("Found {} input devices", devices.len());
        for device in &devices {
            println!(
                "  - {} {}",
                device.name,
                if device.is_default { "(default)" } else { "" }
            );
        }
    }

    #[test]
    fn test_audio_recorder_creation() {
        // This test might fail in CI environments without audio devices
        match AudioRecorder::new() {
            Ok(recorder) => {
                assert!(recorder.sample_rate() > 0);
                assert!(recorder.channels() > 0);
                assert!(!recorder.is_recording());
                println!(
                    "Created recorder: {}Hz, {} channels",
                    recorder.sample_rate(),
                    recorder.channels()
                );
            }
            Err(e) => {
                println!("Could not create recorder (expected in CI): {}", e);
            }
        }
    }

    #[test]
    fn test_recording_state() {
        if let Ok(mut recorder) = AudioRecorder::new() {
            assert!(!recorder.is_recording());

            let (tx, _rx) = bounded(10);
            if recorder.start(tx).is_ok() {
                assert!(recorder.is_recording());

                let _ = recorder.stop();
                assert!(!recorder.is_recording());
            }
        }
    }

    #[test]
    fn test_double_start() {
        if let Ok(mut recorder) = AudioRecorder::new() {
            let (tx1, _rx1) = bounded(10);
            let (tx2, _rx2) = bounded(10);

            if recorder.start(tx1).is_ok() {
                // Second start should be a no-op
                assert!(recorder.start(tx2).is_ok());
                assert!(recorder.is_recording());
            }
        }
    }

    #[test]
    fn test_stop_when_not_recording() {
        if let Ok(mut recorder) = AudioRecorder::new() {
            // Should not panic or error
            assert!(recorder.stop().is_ok());
        }
    }
}
