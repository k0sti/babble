use crate::{BabbleError, Result};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use tracing::debug;

/// Audio resampler for converting between different sample rates
pub struct AudioResampler {
    resampler: SincFixedIn<f32>,
    input_rate: u32,
    output_rate: u32,
    channels: usize,
}

impl AudioResampler {
    /// Create a new audio resampler
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate
    /// * `output_rate` - Output sample rate
    /// * `channels` - Number of audio channels
    pub fn new(input_rate: u32, output_rate: u32, channels: u16) -> Result<Self> {
        if input_rate == 0 || output_rate == 0 {
            return Err(BabbleError::ConfigError(
                "Sample rates must be greater than 0".into()
            ));
        }

        if channels == 0 {
            return Err(BabbleError::ConfigError(
                "Number of channels must be greater than 0".into()
            ));
        }

        // Calculate the resampling ratio
        let resample_ratio = output_rate as f64 / input_rate as f64;

        // Configure sinc interpolation parameters for high quality
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        // Create the resampler
        // chunk_size is the number of frames per channel
        let chunk_size = 1024;

        let resampler = SincFixedIn::<f32>::new(
            resample_ratio,
            2.0,
            params,
            chunk_size,
            channels as usize,
        )
        .map_err(|e| BabbleError::AudioProcessingError(format!("Failed to create resampler: {}", e)))?;

        debug!(
            "Created resampler: {} Hz -> {} Hz, {} channels",
            input_rate, output_rate, channels
        );

        Ok(Self {
            resampler,
            input_rate,
            output_rate,
            channels: channels as usize,
        })
    }

    /// Resample audio data
    ///
    /// # Arguments
    /// * `input` - Input audio samples (interleaved if multi-channel)
    ///
    /// # Returns
    /// * Resampled audio samples (interleaved if multi-channel)
    pub fn resample(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Convert interleaved input to planar format (separate channels)
        let frames = input.len() / self.channels;
        let mut input_planar = vec![vec![0.0f32; frames]; self.channels];

        for (frame_idx, chunk) in input.chunks(self.channels).enumerate() {
            for (ch_idx, &sample) in chunk.iter().enumerate() {
                input_planar[ch_idx][frame_idx] = sample;
            }
        }

        // Process the audio
        let output_planar = self.resampler
            .process(&input_planar, None)
            .map_err(|e| BabbleError::AudioProcessingError(format!("Resampling failed: {}", e)))?;

        // Convert planar output back to interleaved format
        let output_frames = output_planar[0].len();
        let mut output = vec![0.0f32; output_frames * self.channels];

        for frame_idx in 0..output_frames {
            for ch_idx in 0..self.channels {
                output[frame_idx * self.channels + ch_idx] = output_planar[ch_idx][frame_idx];
            }
        }

        Ok(output)
    }

    /// Get the input sample rate
    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    /// Get the output sample rate
    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    /// Get the number of channels
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Calculate the expected output length for a given input length
    pub fn output_frames_max(&self) -> usize {
        self.resampler.output_frames_max()
    }

    /// Reset the resampler state
    pub fn reset(&mut self) {
        self.resampler.reset();
    }
}

/// Helper function to resample audio in one step
///
/// # Arguments
/// * `input` - Input audio samples
/// * `input_rate` - Input sample rate
/// * `output_rate` - Output sample rate
/// * `channels` - Number of channels
///
/// # Returns
/// * Resampled audio samples
pub fn resample_audio(
    input: &[f32],
    input_rate: u32,
    output_rate: u32,
    channels: u16,
) -> Result<Vec<f32>> {
    if input_rate == output_rate {
        return Ok(input.to_vec());
    }

    let mut resampler = AudioResampler::new(input_rate, output_rate, channels)?;
    resampler.resample(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_creation() {
        let resampler = AudioResampler::new(16000, 48000, 1);
        assert!(resampler.is_ok());
    }

    #[test]
    fn test_invalid_parameters() {
        assert!(AudioResampler::new(0, 48000, 1).is_err());
        assert!(AudioResampler::new(16000, 0, 1).is_err());
        assert!(AudioResampler::new(16000, 48000, 0).is_err());
    }

    #[test]
    fn test_resample_upsampling() {
        if let Ok(mut resampler) = AudioResampler::new(16000, 48000, 1) {
            let input: Vec<f32> = (0..1024).map(|i| (i as f32).sin()).collect();
            if let Ok(output) = resampler.resample(&input) {
                assert!(!output.is_empty());
                // Output should be roughly 3x the input size (48000/16000 = 3)
                assert!(output.len() > input.len() * 2);
            }
        }
    }

    #[test]
    fn test_resample_downsampling() {
        if let Ok(mut resampler) = AudioResampler::new(48000, 16000, 1) {
            let input: Vec<f32> = (0..3072).map(|i| (i as f32).sin()).collect();
            if let Ok(output) = resampler.resample(&input) {
                assert!(!output.is_empty());
                // Output should be roughly 1/3 the input size
                assert!(output.len() < input.len());
            }
        }
    }

    #[test]
    fn test_resample_empty_input() {
        if let Ok(mut resampler) = AudioResampler::new(16000, 48000, 1) {
            let output = resampler.resample(&[]);
            assert!(output.is_ok());
            assert!(output.unwrap().is_empty());
        }
    }
}
