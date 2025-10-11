use super::{TranscriptionError, TranscriptionProvider};
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

pub struct OpenAIProvider {
    api_key: String,
    client: reqwest::Client,
    max_retries: u32,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    /// Create a new `OpenAI` provider with custom options
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be initialized
    pub fn new_with_options(
        api_key: String,
        timeout_seconds: Option<u64>,
        max_retries: Option<u32>,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<Self, TranscriptionError> {
        let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| {
                TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                    provider: "OpenAI".to_string(),
                    error_type: "HTTP client error".to_string(),
                    error_message: e.to_string(),
                })
            })?;

        Ok(OpenAIProvider {
            api_key,
            client,
            max_retries: max_retries.unwrap_or(3),
            model: model.unwrap_or_else(|| "whisper-1".to_string()),
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })
    }

    async fn transcribe_attempt(
        &self,
        audio_data: &[u8],
        language: Option<&str>,
    ) -> Result<String, TranscriptionError> {
        let url = format!("{}/audio/transcriptions", self.base_url);

        // Create multipart form
        let audio_part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| {
                TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                    provider: "OpenAI".to_string(),
                    error_type: "HTTP client error".to_string(),
                    error_message: e.to_string(),
                })
            })?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", audio_part)
            .text("model", self.model.clone());

        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                    provider: "OpenAI".to_string(),
                    error_type: if e.is_timeout() {
                        "Request timeout".to_string()
                    } else if e.is_connect() {
                        "Connection failed".to_string()
                    } else if e.is_request() {
                        "Request error".to_string()
                    } else {
                        "Network error".to_string()
                    },
                    error_message: e.to_string(),
                })
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                provider: "OpenAI".to_string(),
                error_type: "Response reading error".to_string(),
                error_message: e.to_string(),
            })
        })?;

        match status {
            reqwest::StatusCode::OK => {
                let json: Value = serde_json::from_str(&response_text)
                    .map_err(|e| TranscriptionError::JsonError(e.to_string()))?;
                let text = json.get("text").and_then(|t| t.as_str()).ok_or_else(|| {
                    TranscriptionError::ApiError(crate::transcription::ApiErrorDetails {
                        provider: "OpenAI".to_string(),
                        status_code: Some(200),
                        error_code: None,
                        error_message: "No text field in response".to_string(),
                        raw_response: Some(response_text.clone()),
                    })
                })?;
                Ok(text.to_string())
            }
            reqwest::StatusCode::UNAUTHORIZED => Err(TranscriptionError::AuthenticationFailed {
                provider: "OpenAI".to_string(),
                details: Some("Invalid API key".to_string()),
            }),
            _ => {
                // Try to parse error details from response
                let (error_code, error_message) =
                    if let Ok(json) = serde_json::from_str::<Value>(&response_text) {
                        let code = json
                            .get("error")
                            .and_then(|e| e.get("code"))
                            .and_then(|c| c.as_str())
                            .map(std::string::ToString::to_string);
                        let message = json
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or(&response_text)
                            .to_string();
                        (code, message)
                    } else {
                        (None, response_text.clone())
                    };

                Err(TranscriptionError::ApiError(
                    crate::transcription::ApiErrorDetails {
                        provider: "OpenAI".to_string(),
                        status_code: Some(status.as_u16()),
                        error_code,
                        error_message,
                        raw_response: Some(response_text),
                    },
                ))
            }
        }
    }
}

#[async_trait]
impl TranscriptionProvider for OpenAIProvider {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError> {
        // Check file size (25MB limit for OpenAI Whisper API)
        const MAX_FILE_SIZE: usize = 25 * 1024 * 1024;
        if audio_data.len() > MAX_FILE_SIZE {
            return Err(TranscriptionError::FileTooLarge(audio_data.len()));
        }

        // Normalize language: treat "auto" or empty as no language (auto-detect)
        let language = language.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() || t.eq_ignore_ascii_case("auto") {
                None
            } else {
                Some(t)
            }
        });

        let mut retries = 0;
        loop {
            match self
                .transcribe_attempt(&audio_data, language.as_deref())
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    retries += 1;
                    if retries > self.max_retries {
                        return Err(e);
                    }

                    // Don't retry on authentication errors
                    if matches!(e, TranscriptionError::AuthenticationFailed { .. }) {
                        return Err(e);
                    }

                    // Exponential backoff
                    let delay = Duration::from_millis(1000 * (1 << (retries - 1)).min(8));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAIProvider::new_with_options(
            "test-key".to_string(),
            Some(30),
            Some(3),
            Some("whisper-1".to_string()),
            None,
        );
        assert!(provider.is_ok());
    }

    #[test]
    fn test_file_size_validation() {
        let provider =
            OpenAIProvider::new_with_options("test-key".to_string(), None, None, None, None)
                .unwrap();

        // Test file too large
        let large_data = vec![0u8; 26 * 1024 * 1024]; // 26MB
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(provider.transcribe_with_language(large_data, None));

        assert!(matches!(result, Err(TranscriptionError::FileTooLarge(_))));
    }

    #[test]
    fn test_openai_provider_configuration() {
        // Test with custom configuration
        let provider = OpenAIProvider::new_with_options(
            "custom-key".to_string(),
            Some(60), // 60 second timeout
            Some(5),  // 5 retries
            Some("whisper-1".to_string()),
            Some("https://custom.api.com/v1".to_string()),
        )
        .unwrap();

        assert_eq!(provider.api_key, "custom-key");
        assert_eq!(provider.max_retries, 5);
        assert_eq!(provider.model, "whisper-1");
        assert_eq!(provider.base_url, "https://custom.api.com/v1");
    }

    #[test]
    fn test_openai_provider_defaults() {
        // Test with default configuration
        let provider =
            OpenAIProvider::new_with_options("test-key".to_string(), None, None, None, None)
                .unwrap();

        assert_eq!(provider.api_key, "test-key");
        assert_eq!(provider.max_retries, 3);
        assert_eq!(provider.model, "whisper-1");
        assert_eq!(provider.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_file_size_boundary_conditions() {
        let provider =
            OpenAIProvider::new_with_options("test-key".to_string(), None, None, None, None)
                .unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();

        // Test exactly at the limit (should pass validation)
        let max_size_data = vec![0u8; 25 * 1024 * 1024]; // Exactly 25MB
        let result = rt.block_on(provider.transcribe_with_language(max_size_data, None));
        // Should fail for different reason (not file size)
        assert!(!matches!(result, Err(TranscriptionError::FileTooLarge(_))));

        // Test just over the limit (should fail validation)
        let over_size_data = vec![0u8; 25 * 1024 * 1024 + 1]; // Just over 25MB
        let result = rt.block_on(provider.transcribe_with_language(over_size_data, None));
        assert!(matches!(result, Err(TranscriptionError::FileTooLarge(_))));
    }
}
