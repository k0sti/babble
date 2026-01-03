use crate::audio::resampler::AudioResampler;
use crate::{BabbleError, Result};
use tracing::debug;

/// Audio preprocessor for preparing audio for speech recognition
pub struct AudioPreprocessor {
    resampler: Option<AudioResampler>,
    target_sample_rate: u32,
    normalization_enabled: bool,
}

impl AudioPreprocessor {
    /// Create a new audio preprocessor
    pub fn new(
        input_sample_rate: u32,
        target_sample_rate: u32,
        channels: u16,
        normalization_enabled: bool,
    ) -> Result<Self> {
        let resampler = if input_sample_rate != target_sample_rate {
            Some(AudioResampler::new(
                input_sample_rate,
                target_sample_rate,
                channels,
            )?)
        } else {
            None
        };

        Ok(Self {
            resampler,
            target_sample_rate,
            normalization_enabled,
        })
    }

    /// Create a preprocessor for 16kHz output (standard for Whisper)
    pub fn for_whisper(input_sample_rate: u32, channels: u16) -> Result<Self> {
        Self::new(input_sample_rate, 16000, channels, true)
    }

    /// Process audio samples
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Step 1: Resample if necessary
        let resampled = if let Some(ref mut resampler) = self.resampler {
            resampler.resample(input)?
        } else {
            input.to_vec()
        };

        // Step 2: Normalize if enabled
        let normalized = if self.normalization_enabled {
            normalize_audio(&resampled)
        } else {
            resampled
        };

        Ok(normalized)
    }

    /// Convert stereo to mono by averaging channels
    pub fn stereo_to_mono(input: &[f32]) -> Vec<f32> {
        if input.len() % 2 != 0 {
            return input.to_vec();
        }

        let frames = input.len() / 2;
        let mut output = Vec::with_capacity(frames);

        for i in 0..frames {
            let left = input[i * 2];
            let right = input[i * 2 + 1];
            output.push((left + right) / 2.0);
        }

        output
    }

    /// Get the target sample rate
    pub fn target_sample_rate(&self) -> u32 {
        self.target_sample_rate
    }
}

/// Normalize audio to have peak amplitude of 1.0
pub fn normalize_audio(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let peak = samples
        .iter()
        .map(|&s| s.abs())
        .fold(0.0f32, |max, val| max.max(val));

    if peak == 0.0 || peak.is_nan() {
        return samples.to_vec();
    }

    let target_peak = 0.95;
    let gain = target_peak / peak;

    samples.iter().map(|&s| s * gain).collect()
}

/// Apply a simple high-pass filter to remove DC offset
pub fn remove_dc_offset(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let mean: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
    samples.iter().map(|&s| s - mean).collect()
}

/// Apply RMS normalization
pub fn normalize_rms(samples: &[f32], target_rms: f32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    let rms = (sum_squares / samples.len() as f32).sqrt();

    if rms == 0.0 || rms.is_nan() {
        return samples.to_vec();
    }

    let gain = target_rms / rms;

    samples
        .iter()
        .map(|&s| (s * gain).clamp(-1.0, 1.0))
        .collect()
}

/// Preprocess audio for Whisper transcription
pub fn preprocess_for_whisper(
    input: &[f32],
    input_sample_rate: u32,
    is_stereo: bool,
) -> Result<Vec<f32>> {
    debug!(
        "Preprocessing audio: {} samples, {}Hz, {} channels",
        input.len(),
        input_sample_rate,
        if is_stereo { 2 } else { 1 }
    );

    let mono = if is_stereo {
        AudioPreprocessor::stereo_to_mono(input)
    } else {
        input.to_vec()
    };

    let no_dc = remove_dc_offset(&mono);

    let resampled = if input_sample_rate != 16000 {
        let mut preprocessor = AudioPreprocessor::for_whisper(input_sample_rate, 1)?;
        preprocessor.process(&no_dc)?
    } else {
        no_dc
    };

    let normalized = normalize_audio(&resampled);

    debug!(
        "Preprocessing complete: {} samples at 16kHz",
        normalized.len()
    );

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_audio() {
        let input = vec![0.5, -0.3, 0.8, -0.2];
        let output = normalize_audio(&input);
        let peak = output.iter().map(|&s| s.abs()).fold(0.0, f32::max);
        assert!((peak - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_stereo_to_mono() {
        let input = vec![1.0, -1.0, 0.5, -0.5, 0.8, -0.8];
        let output = AudioPreprocessor::stereo_to_mono(&input);
        assert_eq!(output.len(), 3);
        assert_eq!(output[0], 0.0);
    }

    #[test]
    fn test_remove_dc_offset() {
        let input = vec![1.0, 1.1, 0.9, 1.0];
        let output = remove_dc_offset(&input);
        let mean: f32 = output.iter().sum::<f32>() / output.len() as f32;
        assert!(mean.abs() < 0.0001);
    }
}
