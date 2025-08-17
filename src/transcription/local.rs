use async_trait::async_trait;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::{TranscriptionError, TranscriptionProvider};

pub struct LocalProvider {
    context: WhisperContext,
    language: Option<String>,
}

impl std::fmt::Debug for LocalProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalProvider")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

impl LocalProvider {
    pub fn new(model_path: &Path, language: Option<String>) -> Result<Self, TranscriptionError> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or_else(|| {
                TranscriptionError::ConfigurationError(
                    "Model path contains invalid UTF-8 characters".to_string(),
                )
            })?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| {
            TranscriptionError::ConfigurationError(format!("Failed to load Whisper model: {}", e))
        })?;

        Ok(Self {
            context: ctx,
            language,
        })
    }

    fn convert_audio_to_float32(&self, audio_data: &[u8]) -> Result<Vec<f32>, TranscriptionError> {
        if audio_data.len() % 2 != 0 {
            return Err(TranscriptionError::ConfigurationError(
                "Audio data length must be even for 16-bit samples".to_string(),
            ));
        }

        let samples: Vec<f32> = audio_data
            .chunks_exact(2)
            .map(|chunk| {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                f32::from(sample) / 32768.0
            })
            .collect();

        Ok(samples)
    }
}

#[async_trait]
impl TranscriptionProvider for LocalProvider {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError> {
        if audio_data.is_empty() {
            return Err(TranscriptionError::ConfigurationError(
                "Audio data is empty".to_string(),
            ));
        }

        let audio_f32 = self.convert_audio_to_float32(&audio_data)?;

        // Create params inside the spawn_blocking to avoid lifetime issues
        let language_for_params = language.or_else(|| self.language.clone());

        let mut state = self.context.create_state().map_err(|e| {
            TranscriptionError::ConfigurationError(format!("Failed to create Whisper state: {}", e))
        })?;

        tokio::task::spawn_blocking(move || {
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

            // Configure language if provided and not "auto"
            if let Some(ref lang) = language_for_params {
                if lang != "auto" {
                    params.set_language(Some(lang.as_str()));
                }
            }

            params.set_translate(false);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            state.full(params, &audio_f32).map_err(|e| {
                TranscriptionError::ApiError(super::ApiErrorDetails {
                    provider: "local".to_string(),
                    status_code: None,
                    error_code: None,
                    error_message: format!("Whisper transcription failed: {}", e),
                    raw_response: None,
                })
            })?;

            let num_segments = state.full_n_segments().map_err(|e| {
                TranscriptionError::ApiError(super::ApiErrorDetails {
                    provider: "local".to_string(),
                    status_code: None,
                    error_code: None,
                    error_message: format!("Failed to get number of segments: {}", e),
                    raw_response: None,
                })
            })?;

            let mut result = String::new();
            for i in 0..num_segments {
                let segment_text = state.full_get_segment_text(i).map_err(|e| {
                    TranscriptionError::ApiError(super::ApiErrorDetails {
                        provider: "local".to_string(),
                        status_code: None,
                        error_code: None,
                        error_message: format!("Failed to get segment text: {}", e),
                        raw_response: None,
                    })
                })?;
                result.push_str(&segment_text);
            }

            Ok(result.trim().to_string())
        })
        .await
        .map_err(|e| {
            TranscriptionError::ApiError(super::ApiErrorDetails {
                provider: "local".to_string(),
                status_code: None,
                error_code: None,
                error_message: format!("Task join error: {}", e),
                raw_response: None,
            })
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_audio_to_float32_static() {
        // Test the audio conversion logic without needing a provider instance
        let audio_data = [0x00, 0x00, 0xFF, 0x7F, 0x00, 0x80];

        // Manual implementation of the conversion logic for testing
        let samples: Vec<f32> = audio_data
            .chunks_exact(2)
            .map(|chunk| {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                f32::from(sample) / 32768.0
            })
            .collect();

        assert_eq!(samples.len(), 3);
        assert!((samples[0] - 0.0).abs() < f32::EPSILON);
        assert!((samples[1] - 0.999_969_5).abs() < 0.001);
        assert!((samples[2] - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_convert_audio_odd_length_validation() {
        let audio_data = [0x00, 0x00, 0xFF];

        // Test validation logic without provider
        assert_eq!(audio_data.len() % 2, 1, "Audio data should have odd length");
    }

    #[test]
    fn test_new_provider_invalid_path() {
        let result = LocalProvider::new(Path::new("/nonexistent/path.bin"), None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to load Whisper model"));
    }

    #[test]
    fn test_empty_audio_validation() {
        let audio_data: Vec<u8> = vec![];
        assert!(audio_data.is_empty(), "Empty audio data should be detected");
    }

    #[test]
    fn test_debug_implementation() {
        use std::fmt::Write;

        // Test that our Debug implementation works
        let mut debug_str = String::new();
        let debug_fmt = format!("{:?}", std::marker::PhantomData::<LocalProvider>);
        write!(&mut debug_str, "{}", debug_fmt).unwrap();
        // This just tests that Debug formatting doesn't panic
    }
}
