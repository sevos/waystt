use async_trait::async_trait;
use std::fmt;

pub mod openai;
// Secure Google provider using google-api-proto
pub mod google_v2;

#[derive(Debug)]
pub enum TranscriptionError {
    AuthenticationFailed,
    NetworkError(String),
    FileTooLarge(usize),
    ApiError(String),
    JsonError(String),
    ConfigurationError(String),
    UnsupportedProvider(String),
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranscriptionError::AuthenticationFailed => write!(f, "Authentication failed"),
            TranscriptionError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            TranscriptionError::FileTooLarge(size) => {
                write!(f, "File too large: {} bytes (max 25MB)", size)
            }
            TranscriptionError::ApiError(msg) => write!(f, "API error: {}", msg),
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
                    None,
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

                let client = google_v2::GoogleV2Provider::new(
                    credentials_path,
                    config.google_speech_language_code,
                    config.google_speech_model,
                    config.google_speech_alternative_languages,
                )
                .await?;

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
    use std::sync::Mutex;

    // Use the same mutex as config tests to prevent race conditions
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_transcription_error_display() {
        let error = TranscriptionError::AuthenticationFailed;
        assert_eq!(error.to_string(), "Authentication failed");

        let error = TranscriptionError::NetworkError("Connection timeout".to_string());
        assert_eq!(error.to_string(), "Network error: Connection timeout");

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
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("OPENAI_API_KEY");

        let result = TranscriptionFactory::create_provider("openai").await;
        assert!(result.is_err());

        if let Err(TranscriptionError::ConfigurationError(msg)) = result {
            assert!(msg.contains("OpenAI API key not found"));
        } else {
            panic!("Expected ConfigurationError for missing API key");
        }
    }

    #[tokio::test]
    async fn test_factory_openai_provider_creation() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let result = TranscriptionFactory::create_provider("openai").await;
        assert!(result.is_ok());

        let provider = result.unwrap();

        // Test that the provider implements the trait
        let empty_audio = vec![];
        let result = provider.transcribe_with_language(empty_audio, None).await;
        // We expect this to fail with network/auth error, but it should compile and run
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_factory_google_provider_missing_credentials() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GOOGLE_APPLICATION_CREDENTIALS");

        let result = TranscriptionFactory::create_provider("google").await;
        assert!(result.is_err());

        if let Err(TranscriptionError::ConfigurationError(msg)) = result {
            assert!(msg.contains("Google application credentials not found"));
        } else {
            panic!("Expected ConfigurationError for missing credentials");
        }
    }

    #[tokio::test]
    async fn test_provider_switching_integration() {
        let _lock = ENV_MUTEX.lock().unwrap();
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

        // Cleanup
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[tokio::test]
    async fn test_backward_compatibility_with_existing_config() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // This test ensures that existing .env configurations continue to work
        std::env::set_var("OPENAI_API_KEY", "test-key");
        std::env::remove_var("TRANSCRIPTION_PROVIDER"); // Default should be openai

        let config = crate::config::load_config();
        assert_eq!(config.transcription_provider, "openai");

        let provider = TranscriptionFactory::create_provider(&config.transcription_provider).await;
        assert!(provider.is_ok());

        // Cleanup
        std::env::remove_var("OPENAI_API_KEY");
    }
}
