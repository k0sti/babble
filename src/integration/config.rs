//! Configuration for the integration layer
//!
//! Provides centralized configuration for all components.

use crate::llm::config::LLMConfig;
use crate::speech::stt::WhisperConfig;
use crate::speech::tts::TTSConfig;
use std::path::PathBuf;

/// Configuration for the complete integration
#[derive(Clone, Debug)]
pub struct IntegrationConfig {
    /// LLM configuration
    pub llm: LLMConfig,

    /// STT (Whisper) configuration
    pub stt: WhisperConfig,

    /// TTS configuration
    pub tts: TTSConfig,

    /// Whether to enable audio input
    pub enable_audio_input: bool,

    /// Whether to enable audio output
    pub enable_audio_output: bool,

    /// Sample rate for audio recording (before resampling)
    pub input_sample_rate: u32,

    /// Sample rate for audio playback
    pub output_sample_rate: u32,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            llm: LLMConfig::default(),
            stt: WhisperConfig::default(),
            tts: TTSConfig::default(),
            enable_audio_input: true,
            enable_audio_output: true,
            input_sample_rate: 16000,
            output_sample_rate: 22050,
        }
    }
}

impl IntegrationConfig {
    /// Create a new configuration with model paths
    pub fn with_models(
        whisper_model: impl Into<PathBuf>,
        tts_model: impl Into<String>,
        tts_tokens: impl Into<String>,
    ) -> Self {
        let mut config = Self::default();
        config.stt.model_path = whisper_model.into();
        config.tts.model_path = tts_model.into();
        config.tts.tokens_path = tts_tokens.into();
        config
    }

    /// Set the LLM configuration
    pub fn with_llm(mut self, llm: LLMConfig) -> Self {
        self.llm = llm;
        self
    }

    /// Disable audio input (text-only mode)
    pub fn without_audio_input(mut self) -> Self {
        self.enable_audio_input = false;
        self
    }

    /// Disable audio output (text-only mode)
    pub fn without_audio_output(mut self) -> Self {
        self.enable_audio_output = false;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Check STT model exists
        if self.enable_audio_input && !self.stt.model_path.exists() {
            return Err(format!(
                "Whisper model not found: {:?}",
                self.stt.model_path
            ));
        }

        // Check TTS model exists
        if self.enable_audio_output {
            if self.tts.model_path.is_empty() {
                return Err("TTS model path is required".to_string());
            }
            let tts_path = std::path::Path::new(&self.tts.model_path);
            if !tts_path.exists() {
                return Err(format!("TTS model not found: {}", self.tts.model_path));
            }

            let tokens_path = std::path::Path::new(&self.tts.tokens_path);
            if !tokens_path.exists() {
                return Err(format!(
                    "TTS tokens file not found: {}",
                    self.tts.tokens_path
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = IntegrationConfig::default();
        assert!(config.enable_audio_input);
        assert!(config.enable_audio_output);
        assert_eq!(config.input_sample_rate, 16000);
    }

    #[test]
    fn test_config_builder() {
        let config = IntegrationConfig::default()
            .without_audio_input()
            .without_audio_output();

        assert!(!config.enable_audio_input);
        assert!(!config.enable_audio_output);
    }
}
