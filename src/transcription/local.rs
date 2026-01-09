use super::{ApiErrorDetails, TranscriptionError, TranscriptionProvider};
use async_trait::async_trait;
use hound;
use std::ffi::c_int;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Configuration for local Whisper GPU acceleration
#[derive(Debug, Clone, Default)]
pub struct LocalWhisperGpuConfig {
    /// Override GPU usage. None = use compile-time default.
    pub use_gpu: Option<bool>,
    /// GPU device ID (default: 0)
    pub gpu_device: i32,
    /// Enable flash attention optimization (default: false)
    pub flash_attn: bool,
}

pub struct LocalWhisperProvider {
    context: WhisperContext,
}

impl LocalWhisperProvider {
    /// Helper function to validate and convert model path to string
    fn validate_model_path(model_path: &Path) -> Result<&str, TranscriptionError> {
        if !model_path.exists() {
            return Err(TranscriptionError::ConfigurationError(format!(
                "Model file not found: {}",
                model_path.display()
            )));
        }

        model_path
            .to_str()
            .ok_or_else(|| TranscriptionError::ConfigurationError("Invalid model path".to_string()))
    }

    /// Create a new LocalWhisperProvider with default (CPU) configuration.
    /// Kept for backward compatibility - prefer `new_with_gpu_config` for GPU support.
    #[allow(dead_code)]
    pub fn new(model_path: &Path) -> Result<Self, TranscriptionError> {
        let model_str = Self::validate_model_path(model_path)?;

        let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
            .map_err(|e| {
            TranscriptionError::ConfigurationError(format!("Failed to load model: {}", e))
        })?;

        Ok(Self { context: ctx })
    }

    /// Create a new LocalWhisperProvider with GPU configuration
    pub fn new_with_gpu_config(
        model_path: &Path,
        gpu_config: LocalWhisperGpuConfig,
    ) -> Result<Self, TranscriptionError> {
        let model_str = Self::validate_model_path(model_path)?;

        let mut params = WhisperContextParameters::default();

        // Log GPU configuration
        match gpu_config.use_gpu {
            Some(true) => eprintln!(
                "Initializing local Whisper provider with GPU enabled (device: {}, flash_attn: {})",
                gpu_config.gpu_device, gpu_config.flash_attn
            ),
            Some(false) => {
                eprintln!("Initializing local Whisper provider with GPU explicitly disabled")
            }
            None => {
                // Using compile-time default - only log if explicitly configuring device/flash_attn
                if gpu_config.gpu_device != 0 || gpu_config.flash_attn {
                    eprintln!("Initializing local Whisper provider with compile-time GPU default (device: {}, flash_attn: {})",
                        gpu_config.gpu_device, gpu_config.flash_attn);
                }
            }
        }

        if let Some(use_gpu) = gpu_config.use_gpu {
            params.use_gpu(use_gpu);
        }
        params.gpu_device(gpu_config.gpu_device as c_int);
        params.flash_attn(gpu_config.flash_attn);

        let ctx = WhisperContext::new_with_params(model_str, params).map_err(|e| {
            TranscriptionError::ConfigurationError(format!("Failed to load model: {}", e))
        })?;

        Ok(Self { context: ctx })
    }
}

#[async_trait]
impl TranscriptionProvider for LocalWhisperProvider {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError> {
        // Decode WAV to PCM samples
        let reader = hound::WavReader::new(std::io::Cursor::new(audio_data)).map_err(|e| {
            TranscriptionError::ConfigurationError(format!("Failed to read WAV data: {}", e))
        })?;
        let samples: Result<Vec<f32>, _> = reader
            .into_samples::<i16>()
            .map(|s| s.map(|v| f32::from(v) / f32::from(i16::MAX)))
            .collect();
        let samples = samples.map_err(|e| {
            TranscriptionError::ConfigurationError(format!("Failed to parse WAV samples: {}", e))
        })?;

        let mut state = self.context.create_state().map_err(|e| {
            TranscriptionError::ApiError(ApiErrorDetails {
                provider: "Local".to_string(),
                status_code: None,
                error_code: None,
                error_message: format!("Failed to create state: {}", e),
                raw_response: None,
            })
        })?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        if let Some(ref lang) = language {
            params.set_language(Some(lang));
        }
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_suppress_blank(true);

        state.full(params, &samples).map_err(|e| {
            TranscriptionError::ApiError(ApiErrorDetails {
                provider: "Local".to_string(),
                status_code: None,
                error_code: None,
                error_message: e.to_string(),
                raw_response: None,
            })
        })?;

        let mut result = String::new();
        let num_segments = state.full_n_segments();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(text) = segment.to_str() {
                    result.push_str(text);
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_whisper_gpu_config_default() {
        let config = LocalWhisperGpuConfig::default();
        assert_eq!(config.use_gpu, None);
        assert_eq!(config.gpu_device, 0);
        assert!(!config.flash_attn);
    }

    #[test]
    fn test_local_whisper_gpu_config_custom() {
        let config = LocalWhisperGpuConfig {
            use_gpu: Some(true),
            gpu_device: 1,
            flash_attn: true,
        };
        assert_eq!(config.use_gpu, Some(true));
        assert_eq!(config.gpu_device, 1);
        assert!(config.flash_attn);
    }

    #[test]
    fn test_new_with_gpu_config_missing_model() {
        let gpu_config = LocalWhisperGpuConfig::default();
        let result = LocalWhisperProvider::new_with_gpu_config(
            std::path::Path::new("/nonexistent/model.bin"),
            gpu_config,
        );
        assert!(result.is_err());
        if let Err(TranscriptionError::ConfigurationError(msg)) = result {
            assert!(msg.contains("Model file not found"));
        } else {
            panic!("Expected ConfigurationError");
        }
    }
}

// GPU-specific tests that only compile when a GPU feature is enabled
#[cfg(test)]
#[cfg(any(
    feature = "cuda",
    feature = "vulkan",
    feature = "hipblas",
    feature = "metal"
))]
mod gpu_tests {
    use super::*;

    #[test]
    fn test_gpu_feature_enabled() {
        // Verify that GPU features are properly detected at compile time.
        // When compiled with GPU features, WhisperContextParameters::default()
        // should have use_gpu set to true.
        let params = WhisperContextParameters::default();
        assert!(
            params.use_gpu,
            "GPU should be enabled by default when compiled with GPU feature"
        );
    }
}
