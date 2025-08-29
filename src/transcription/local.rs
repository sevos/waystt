use super::{ApiErrorDetails, TranscriptionError, TranscriptionProvider};
use async_trait::async_trait;
use hound;
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct LocalWhisperProvider {
    context: WhisperContext,
}

impl LocalWhisperProvider {
    pub fn new(model_path: PathBuf) -> Result<Self, TranscriptionError> {
        if !model_path.exists() {
            return Err(TranscriptionError::ConfigurationError(format!(
                "Model file not found: {}",
                model_path.display()
            )));
        }

        let model_str = model_path.to_str().ok_or_else(|| {
            TranscriptionError::ConfigurationError("Invalid model path".to_string())
        })?;

        let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
            .map_err(|e| {
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
            .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
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
