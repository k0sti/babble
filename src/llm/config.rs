//! LLM configuration for mistral.rs integration

use std::path::PathBuf;

/// Quantization type for model weights
#[derive(Clone, Debug, Default)]
pub enum QuantizationType {
    /// No quantization (full precision)
    None,
    /// 4-bit quantization (Q4K)
    #[default]
    Q4K,
    /// 8-bit quantization (Q8_0)
    Q8_0,
    /// 4-bit quantization (Q4_0)
    Q4_0,
}

/// Configuration for the LLM engine
#[derive(Clone, Debug)]
pub struct LLMConfig {
    /// Model identifier (HuggingFace model ID or local path)
    pub model_id: String,

    /// Quantization type for model weights
    pub quantization: QuantizationType,

    /// Temperature for sampling (0.0 = deterministic, 1.0+ = creative)
    pub temperature: f32,

    /// Top-p (nucleus) sampling parameter
    pub top_p: f32,

    /// Top-k sampling parameter (0 = disabled)
    pub top_k: usize,

    /// Maximum tokens to generate per response
    pub max_tokens: usize,

    /// Repetition penalty (1.0 = no penalty)
    pub repetition_penalty: f32,

    /// Number of CPU threads for inference
    pub n_threads: usize,

    /// Whether to use GPU acceleration
    pub use_gpu: bool,

    /// Context window size (max tokens in conversation)
    pub context_size: usize,

    /// Local model cache directory
    pub cache_dir: Option<PathBuf>,

    /// Enable logging of inference details
    pub enable_logging: bool,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            model_id: "microsoft/Phi-3.5-mini-instruct".to_string(),
            quantization: QuantizationType::Q4K,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            max_tokens: 2048,
            repetition_penalty: 1.1,
            n_threads: 4,
            use_gpu: true,
            context_size: 4096,
            cache_dir: None,
            enable_logging: false,
        }
    }
}

impl LLMConfig {
    /// Create a new LLM configuration with the specified model
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            ..Default::default()
        }
    }

    /// Set the quantization type
    pub fn with_quantization(mut self, quantization: QuantizationType) -> Self {
        self.quantization = quantization;
        self
    }

    /// Set the temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set top-p sampling
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = top_p;
        self
    }

    /// Set maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the number of CPU threads
    pub fn with_threads(mut self, n_threads: usize) -> Self {
        self.n_threads = n_threads;
        self
    }

    /// Enable or disable GPU acceleration
    pub fn with_gpu(mut self, use_gpu: bool) -> Self {
        self.use_gpu = use_gpu;
        self
    }

    /// Set the context window size
    pub fn with_context_size(mut self, context_size: usize) -> Self {
        self.context_size = context_size;
        self
    }

    /// Set the cache directory for downloaded models
    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
        self
    }

    /// Enable inference logging
    pub fn with_logging(mut self, enable: bool) -> Self {
        self.enable_logging = enable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LLMConfig::default();
        assert_eq!(config.model_id, "microsoft/Phi-3.5-mini-instruct");
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.top_p, 0.9);
        assert_eq!(config.max_tokens, 2048);
    }

    #[test]
    fn test_builder_pattern() {
        let config = LLMConfig::new("meta-llama/Llama-3.2-1B")
            .with_temperature(0.5)
            .with_max_tokens(1024)
            .with_gpu(false);

        assert_eq!(config.model_id, "meta-llama/Llama-3.2-1B");
        assert_eq!(config.temperature, 0.5);
        assert_eq!(config.max_tokens, 1024);
        assert!(!config.use_gpu);
    }
}
