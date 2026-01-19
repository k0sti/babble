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

        let chunk_size = self.resampler.input_frames_max();
        let total_frames = input.len() / self.channels;

        // Estimate output size
        let ratio = self.output_rate as f64 / self.input_rate as f64;
        let estimated_output_frames = (total_frames as f64 * ratio * 1.1) as usize;
        let mut output = Vec::with_capacity(estimated_output_frames * self.channels);

        // Process in chunks
        let mut frame_offset = 0;
        while frame_offset < total_frames {
            let frames_remaining = total_frames - frame_offset;
            let frames_to_read = frames_remaining.min(chunk_size);

            // SincFixedIn requires exactly chunk_size frames per call
            // Pad with zeros if we have fewer frames remaining
            let mut input_planar = vec![vec![0.0f32; chunk_size]; self.channels];

            for frame_idx in 0..frames_to_read {
                let src_idx = (frame_offset + frame_idx) * self.channels;
                for ch_idx in 0..self.channels {
                    input_planar[ch_idx][frame_idx] = input[src_idx + ch_idx];
                }
            }
            // Remaining frames in input_planar are already zero-padded from initialization

            // Process this chunk (None means all channels are active)
            let output_planar = self.resampler
                .process(&input_planar, None)
                .map_err(|e| BabbleError::AudioProcessingError(format!("Resampling failed: {}", e)))?;

            // Convert planar output back to interleaved format
            // Only take the non-padded portion on the last chunk
            let output_frames = output_planar[0].len();
            let frames_to_take = if frames_remaining < chunk_size {
                // Last chunk: only take the proportion of output corresponding to actual input
                let output_ratio = self.output_rate as f64 / self.input_rate as f64;
                ((frames_to_read as f64) * output_ratio).ceil() as usize
            } else {
                output_frames
            };

            for frame_idx in 0..frames_to_take.min(output_frames) {
                for ch_idx in 0..self.channels {
                    output.push(output_planar[ch_idx][frame_idx]);
                }
            }

            frame_offset += frames_to_read;
        }

        debug!(
            "Resampled {} frames -> {} frames",
            total_frames,
            output.len() / self.channels
        );

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
