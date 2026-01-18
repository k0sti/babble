use crate::{BabbleError, Result};
use voice_activity_detector::VoiceActivityDetector as VadDetector;
use tracing::info;

/// Voice Activity Detection using Silero VAD
pub struct VoiceActivityDetector {
    detector: VadDetector,
    sample_rate: u32,
    threshold: f32,
}

impl VoiceActivityDetector {
    /// Create a new VAD instance
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate of the audio (8000 or 16000)
    /// * `threshold` - Probability threshold for speech detection (0.0-1.0, default: 0.5)
    pub fn new(sample_rate: u32, threshold: f32) -> Result<Self> {
        // Validate sample rate - voice_activity_detector supports 8000 and 16000
        if ![8000, 16000].contains(&sample_rate) {
            return Err(BabbleError::ConfigError(
                format!("Invalid sample rate: {}. Must be 8000 or 16000", sample_rate)
            ));
        }

        // Calculate chunk size based on sample rate (512 samples for 16kHz)
        let chunk_size: usize = match sample_rate {
            8000 => 256,  // 32ms at 8kHz
            16000 => 512, // 32ms at 16kHz
            _ => 512,
        };

        let detector = VadDetector::builder()
            .sample_rate(sample_rate as i32)
            .chunk_size(chunk_size)
            .build()
            .map_err(|e| BabbleError::AudioProcessingError(format!("Failed to create VAD: {:?}", e)))?;

        info!("Initialized VAD with sample rate: {}, threshold: {}", sample_rate, threshold);

        Ok(Self {
            detector,
            sample_rate,
            threshold,
        })
    }

    /// Create a VAD instance with default parameters (16kHz, 0.5 threshold)
    pub fn default_16khz() -> Result<Self> {
        Self::new(16000, 0.5)
    }

    /// Detect if the audio chunk contains speech
    ///
    /// # Arguments
    /// * `audio` - Audio samples (mono, f32)
    ///
    /// # Returns
    /// * `true` if speech is detected, `false` otherwise
    pub fn is_speech(&mut self, audio: &[f32]) -> Result<bool> {
        let probability = self.detector.predict(audio.iter().copied());
        Ok(probability >= self.threshold)
    }

    /// Get the speech probability for the audio chunk
    ///
    /// # Arguments
    /// * `audio` - Audio samples (mono, f32)
    ///
    /// # Returns
    /// * Speech probability (0.0-1.0)
    pub fn get_probability(&mut self, audio: &[f32]) -> Result<f32> {
        Ok(self.detector.predict(audio.iter().copied()))
    }

    /// Reset the VAD session state
    pub fn reset(&mut self) -> Result<()> {
        self.detector.reset();
        Ok(())
    }

    /// Set the speech probability threshold
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
    }

    /// Get the current threshold
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Get the sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the recommended chunk size for this VAD configuration (in samples)
    /// For Silero VAD: 512 samples for 16kHz (32ms)
    pub fn chunk_size(&self) -> usize {
        match self.sample_rate {
            8000 => 256,  // 32ms at 8kHz
            16000 => 512, // 32ms at 16kHz
            _ => 512,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_creation() {
        let vad = VoiceActivityDetector::new(16000, 0.5);
        assert!(vad.is_ok());
    }

    #[test]
    fn test_invalid_sample_rate() {
        let vad = VoiceActivityDetector::new(44100, 0.5);
        assert!(vad.is_err());
    }

    #[test]
    fn test_silence_detection() {
        if let Ok(mut vad) = VoiceActivityDetector::new(16000, 0.5) {
            let silence = vec![0.0f32; 512];
            if let Ok(is_speech) = vad.is_speech(&silence) {
                // Silence should not be detected as speech
                assert!(!is_speech);
            }
        }
    }

    #[test]
    fn test_chunk_size() {
        if let Ok(vad) = VoiceActivityDetector::new(16000, 0.5) {
            assert_eq!(vad.chunk_size(), 512);
        }
    }

    #[test]
    fn test_threshold() {
        if let Ok(mut vad) = VoiceActivityDetector::new(16000, 0.5) {
            assert_eq!(vad.threshold(), 0.5);
            vad.set_threshold(0.7);
            assert_eq!(vad.threshold(), 0.7);
        }
    }
}
