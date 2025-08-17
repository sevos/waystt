use async_trait::async_trait;
use std::fmt;
use std::fmt::Write;
use std::path::Path;

pub mod openai;
// Secure Google provider using google-api-proto
pub mod google_v2;
// Google provider using REST API
pub mod google_v2_rest;
// Local provider using whisper-rs
pub mod local;

#[derive(Debug)]
pub struct ApiErrorDetails {
    pub provider: String,
    pub status_code: Option<u16>,
    pub error_code: Option<String>,
    pub error_message: String,
    pub raw_response: Option<String>,
}

#[derive(Debug)]
pub struct NetworkErrorDetails {
    pub provider: String,
    pub error_type: String,
    pub error_message: String,
}

#[derive(Debug)]
pub enum TranscriptionError {
    AuthenticationFailed {
        provider: String,
        details: Option<String>,
    },
    NetworkError(NetworkErrorDetails),
    FileTooLarge(usize),
    ApiError(ApiErrorDetails),
    JsonError(String),
    ConfigurationError(String),
    UnsupportedProvider(String),
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranscriptionError::AuthenticationFailed { provider, details } => {
                if let Some(details) = details {
                    write!(f, "Authentication failed with {}: {}", provider, details)
                } else {
                    write!(f, "Authentication failed with {}", provider)
                }
            }
            TranscriptionError::NetworkError(details) => {
                write!(
                    f,
                    "Network error with {}: {} - {}",
                    details.provider, details.error_type, details.error_message
                )
            }
            TranscriptionError::FileTooLarge(size) => {
                write!(f, "File too large: {} bytes (max 25MB)", size)
            }
            TranscriptionError::ApiError(details) => {
                let mut msg = format!("API error with {}", details.provider);

                if let Some(status) = details.status_code {
                    write!(&mut msg, " (HTTP {})", status).unwrap();
                }

                if let Some(code) = &details.error_code {
                    write!(&mut msg, " [{}]", code).unwrap();
                }

                write!(&mut msg, ": {}", details.error_message).unwrap();

                write!(f, "{}", msg)
            }
            TranscriptionError::JsonError(msg) => write!(f, "JSON error: {}", msg),
            TranscriptionError::ConfigurationError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            TranscriptionError::UnsupportedProvider(provider) => {
                write!(f, "Unsupported provider: {}", provider)
            }
        }
    }
}

impl std::error::Error for TranscriptionError {}

#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError>;
}

pub struct TranscriptionFactory;

impl TranscriptionFactory {
    pub async fn create_provider(
        provider_type: &str,
    ) -> Result<Box<dyn TranscriptionProvider>, TranscriptionError> {
        match provider_type.to_lowercase().as_str() {
            "openai" => {
                let config = crate::config::load_config();
                let api_key = config.openai_api_key.ok_or_else(|| {
                    TranscriptionError::ConfigurationError("OpenAI API key not found".to_string())
                })?;

                let client = openai::OpenAIProvider::new_with_options(
                    api_key,
                    Some(config.whisper_timeout_seconds),
                    Some(config.whisper_max_retries),
                    Some(config.whisper_model),
                    config.openai_base_url,
                )?;

                Ok(Box::new(client))
            }
            "google" => {
                let config = crate::config::load_config();
                let credentials_path = config.google_application_credentials.ok_or_else(|| {
                    TranscriptionError::ConfigurationError(
                        "Google application credentials not found".to_string(),
                    )
                })?;

                let client = google_v2_rest::GoogleV2RestProvider::new(
                    credentials_path,
                    config.google_speech_language_code,
                    config.google_speech_model,
                    config.google_speech_alternative_languages,
                )
                .await?;

                Ok(Box::new(client))
            }
            "local" => {
                let config = crate::config::load_config();
                let model_path = config.local_model_path.ok_or_else(|| {
                    TranscriptionError::ConfigurationError("Local model path not found".to_string())
                })?;

                let language = if config.whisper_language == "auto" {
                    None
                } else {
                    Some(config.whisper_language)
                };

                let client = local::LocalProvider::new(Path::new(&model_path), language)?;
                Ok(Box::new(client))
            }
            _ => Err(TranscriptionError::UnsupportedProvider(
                provider_type.to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_MUTEX;

    #[test]
    fn test_transcription_error_display() {
        let error = TranscriptionError::AuthenticationFailed {
            provider: "OpenAI".to_string(),
            details: None,
        };
        assert_eq!(error.to_string(), "Authentication failed with OpenAI");

        let error = TranscriptionError::AuthenticationFailed {
            provider: "Google".to_string(),
            details: Some("Invalid API key".to_string()),
        };
        assert_eq!(
            error.to_string(),
            "Authentication failed with Google: Invalid API key"
        );

        let error = TranscriptionError::NetworkError(NetworkErrorDetails {
            provider: "OpenAI".to_string(),
            error_type: "Connection timeout".to_string(),
            error_message: "Request timed out after 30s".to_string(),
        });
        assert_eq!(
            error.to_string(),
            "Network error with OpenAI: Connection timeout - Request timed out after 30s"
        );

        let error = TranscriptionError::ApiError(ApiErrorDetails {
            provider: "Google".to_string(),
            status_code: Some(400),
            error_code: Some("INVALID_ARGUMENT".to_string()),
            error_message: "Invalid language code".to_string(),
            raw_response: None,
        });
        assert_eq!(
            error.to_string(),
            "API error with Google (HTTP 400) [INVALID_ARGUMENT]: Invalid language code"
        );

        let error = TranscriptionError::FileTooLarge(30_000_000);
        assert_eq!(
            error.to_string(),
            "File too large: 30000000 bytes (max 25MB)"
        );

        let error = TranscriptionError::UnsupportedProvider("azure".to_string());
        assert_eq!(error.to_string(), "Unsupported provider: azure");
    }

    #[tokio::test]
    async fn test_factory_unsupported_provider() {
        let result = TranscriptionFactory::create_provider("unsupported").await;
        assert!(result.is_err());

        if let Err(TranscriptionError::UnsupportedProvider(provider)) = result {
            assert_eq!(provider, "unsupported");
        } else {
            panic!("Expected UnsupportedProvider error");
        }
    }

    #[tokio::test]
    async fn test_factory_openai_provider_missing_key() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Save current state and set up test environment
            let original_key = std::env::var("OPENAI_API_KEY").ok();
            std::env::remove_var("OPENAI_API_KEY");

            let result = TranscriptionFactory::create_provider("openai").await;

            // Restore original state
            if let Some(key) = original_key {
                std::env::set_var("OPENAI_API_KEY", key);
            } else {
                std::env::remove_var("OPENAI_API_KEY");
            }

            assert!(result.is_err());

            if let Err(TranscriptionError::ConfigurationError(msg)) = result {
                assert!(msg.contains("OpenAI API key not found"));
            } else {
                panic!("Expected ConfigurationError for missing API key");
            }
        }
    }

    #[tokio::test]
    async fn test_factory_openai_provider_creation() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Save current state and set up test environment
            let original_key = std::env::var("OPENAI_API_KEY").ok();
            std::env::set_var("OPENAI_API_KEY", "test-key");

            let result = TranscriptionFactory::create_provider("openai").await;
            assert!(result.is_ok());

            let provider = result.unwrap();

            // Test that the provider implements the trait
            let empty_audio = vec![];
            let result = provider.transcribe_with_language(empty_audio, None).await;
            // We expect this to fail with network/auth error, but it should compile and run
            assert!(result.is_err());

            // Restore original state
            if let Some(key) = original_key {
                std::env::set_var("OPENAI_API_KEY", key);
            } else {
                std::env::remove_var("OPENAI_API_KEY");
            }
        }
    }

    #[tokio::test]
    async fn test_factory_google_provider_missing_credentials() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Save current state and set up test environment
            let original_credentials = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok();
            std::env::remove_var("GOOGLE_APPLICATION_CREDENTIALS");

            let result = TranscriptionFactory::create_provider("google").await;

            // Restore original state
            if let Some(credentials) = original_credentials {
                std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", credentials);
            } else {
                std::env::remove_var("GOOGLE_APPLICATION_CREDENTIALS");
            }

            assert!(result.is_err());

            if let Err(TranscriptionError::ConfigurationError(msg)) = result {
                assert!(msg.contains("Google application credentials not found"));
            } else {
                panic!("Expected ConfigurationError for missing credentials");
            }
        }
    }

    #[tokio::test]
    async fn test_provider_switching_integration() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Save current state and set up test environment
            let original_key = std::env::var("OPENAI_API_KEY").ok();
            std::env::set_var("OPENAI_API_KEY", "test-key");

            // Test case sensitivity
            let result = TranscriptionFactory::create_provider("OpenAI").await;
            assert!(result.is_ok());

            let result = TranscriptionFactory::create_provider("OPENAI").await;
            assert!(result.is_ok());

            // Test that unsupported providers are handled correctly
            let result = TranscriptionFactory::create_provider("unsupported_provider").await;
            assert!(result.is_err());

            if let Err(TranscriptionError::UnsupportedProvider(provider)) = result {
                assert_eq!(provider, "unsupported_provider");
            } else {
                panic!("Expected UnsupportedProvider error");
            }

            // Restore original state
            if let Some(key) = original_key {
                std::env::set_var("OPENAI_API_KEY", key);
            } else {
                std::env::remove_var("OPENAI_API_KEY");
            }
        }
    }

    #[tokio::test]
    async fn test_backward_compatibility_with_existing_config() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Save current state and set up test environment
            let original_key = std::env::var("OPENAI_API_KEY").ok();
            let original_provider = std::env::var("TRANSCRIPTION_PROVIDER").ok();

            // This test ensures that existing .env configurations continue to work
            std::env::set_var("OPENAI_API_KEY", "test-key");
            std::env::remove_var("TRANSCRIPTION_PROVIDER"); // Default should be openai

            let config = crate::config::load_config();
            assert_eq!(config.transcription_provider, "openai");

            let provider =
                TranscriptionFactory::create_provider(&config.transcription_provider).await;
            assert!(provider.is_ok());

            // Restore original state
            if let Some(key) = original_key {
                std::env::set_var("OPENAI_API_KEY", key);
            } else {
                std::env::remove_var("OPENAI_API_KEY");
            }
            if let Some(provider) = original_provider {
                std::env::set_var("TRANSCRIPTION_PROVIDER", provider);
            } else {
                std::env::remove_var("TRANSCRIPTION_PROVIDER");
            }
        }
    }

    #[tokio::test]
    async fn test_factory_local_provider_missing_model_path() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Save current state and set up test environment
            let original_model_path = std::env::var("LOCAL_MODEL_PATH").ok();
            std::env::remove_var("LOCAL_MODEL_PATH");

            let result = TranscriptionFactory::create_provider("local").await;

            // Restore original state
            if let Some(path) = original_model_path {
                std::env::set_var("LOCAL_MODEL_PATH", path);
            } else {
                std::env::remove_var("LOCAL_MODEL_PATH");
            }

            assert!(result.is_err());

            if let Err(TranscriptionError::ConfigurationError(msg)) = result {
                assert!(msg.contains("Local model path not found"));
            } else {
                panic!("Expected ConfigurationError for missing model path");
            }
        }
    }
}
