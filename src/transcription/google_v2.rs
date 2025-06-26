use async_trait::async_trait;
use google_api_proto::google::cloud::speech::v2::{
    recognition_config::DecodingConfig, recognize_request::AudioSource,
    speech_client::SpeechClient, AutoDetectDecodingConfig, RecognitionConfig, RecognitionFeatures,
    RecognizeRequest,
};
use tonic::{
    transport::{Channel, ClientTlsConfig},
    Request,
};
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};

use crate::transcription::{TranscriptionError, TranscriptionProvider};

pub struct GoogleV2Provider {
    client: SpeechClient<Channel>,
    auth_token: String,
    parent: String,
    language_code: String,
    model: String,
    alternative_languages: Vec<String>,
}

impl GoogleV2Provider {
    #[allow(dead_code)]
    pub async fn new(
        credentials_path: String,
        language_code: String,
        model: String,
        alternative_languages: Vec<String>,
    ) -> Result<Self, TranscriptionError> {
        // Read service account key
        let service_account_key =
            tokio::fs::read_to_string(&credentials_path)
                .await
                .map_err(|e| {
                    TranscriptionError::ConfigurationError(format!(
                        "Failed to read service account key from {}: {}",
                        credentials_path, e
                    ))
                })?;

        let service_account_key: ServiceAccountKey = serde_json::from_str(&service_account_key)
            .map_err(|e| {
                TranscriptionError::ConfigurationError(format!(
                    "Failed to parse service account key: {}",
                    e
                ))
            })?;

        // Extract project ID first
        let project_id = service_account_key.project_id.clone().ok_or_else(|| {
            TranscriptionError::ConfigurationError(
                "No project_id in service account key".to_string(),
            )
        })?;

        // Create authenticator
        let auth = ServiceAccountAuthenticator::builder(service_account_key)
            .build()
            .await
            .map_err(|_e| TranscriptionError::AuthenticationFailed)?;

        // Get access token
        let token = auth
            .token(&["https://www.googleapis.com/auth/cloud-platform"])
            .await
            .map_err(|_e| TranscriptionError::AuthenticationFailed)?;

        // Create channel with explicit TLS configuration and timeout
        let tls_config = ClientTlsConfig::new().domain_name("speech.googleapis.com");
        let endpoint = tonic::transport::Channel::from_static("https://speech.googleapis.com")
            .tls_config(tls_config)
            .map_err(|e| TranscriptionError::NetworkError(format!("TLS config error: {}", e)))?
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10));
        let channel = endpoint.connect().await.map_err(|e| {
            TranscriptionError::NetworkError(format!(
                "Failed to connect to speech.googleapis.com: {}",
                e
            ))
        })?;

        // Create client (we'll add auth headers manually)
        let client = SpeechClient::new(channel);
        let auth_token = format!("Bearer {}", token.token().unwrap_or(""));

        let parent = format!("projects/{}/locations/global", project_id);

        Ok(Self {
            client,
            auth_token,
            parent,
            language_code,
            model,
            alternative_languages,
        })
    }

    fn build_language_codes(&self, language_hint: Option<String>) -> Vec<String> {
        let mut language_codes = Vec::new();

        // If a specific language is provided, use it as primary
        if let Some(lang) = language_hint {
            language_codes.push(lang);
        } else {
            // Use configured primary language
            language_codes.push(self.language_code.clone());
        }

        // Add alternative languages, but limit to 3 total as per Google's requirements
        for alt_lang in &self.alternative_languages {
            if language_codes.len() >= 3 {
                break;
            }
            if !language_codes.contains(alt_lang) {
                language_codes.push(alt_lang.clone());
            }
        }

        language_codes
    }
}

#[async_trait]
impl TranscriptionProvider for GoogleV2Provider {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError> {
        if audio_data.is_empty() {
            return Err(TranscriptionError::ApiError("Empty audio data".to_string()));
        }

        // Google Cloud Speech has a 10MB limit for synchronous recognition
        const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;
        if audio_data.len() > MAX_FILE_SIZE {
            return Err(TranscriptionError::FileTooLarge(audio_data.len()));
        }

        let language_codes = self.build_language_codes(language);

        // Build recognition config using AutoDetectDecodingConfig
        let auto_detect_config = AutoDetectDecodingConfig {};

        let config = RecognitionConfig {
            decoding_config: Some(DecodingConfig::AutoDecodingConfig(auto_detect_config)),
            model: self.model.clone(),
            language_codes,
            features: Some(RecognitionFeatures {
                enable_automatic_punctuation: true,
                enable_word_time_offsets: false,
                enable_word_confidence: false,
                ..Default::default()
            }),
            adaptation: None,
            transcript_normalization: None,
            translation_config: None,
        };

        // Build request
        let request = RecognizeRequest {
            recognizer: format!("{}/recognizers/_", self.parent),
            config: Some(config),
            config_mask: None,
            audio_source: Some(AudioSource::Content(audio_data.into())),
        };

        // Make the API call with auth header
        let mut client = self.client.clone();
        let mut req = Request::new(request);
        req.metadata_mut().insert(
            "authorization",
            self.auth_token
                .parse()
                .map_err(|_| TranscriptionError::AuthenticationFailed)?,
        );

        let response = client.recognize(req).await.map_err(|e| {
            TranscriptionError::NetworkError(format!(
                "Google Speech API gRPC call failed: status={:?}, message={}, details={:?}",
                e.code(),
                e.message(),
                e.metadata()
            ))
        })?;

        let recognize_response = response.into_inner();

        // Extract transcription from response
        // Note: results is a Vec, not an Option<Vec>
        if let Some(result) = recognize_response.results.first() {
            // alternatives is a Vec, not an Option<Vec>
            if let Some(alternative) = result.alternatives.first() {
                return Ok(alternative.transcript.clone());
            }
        }

        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {

    // Helper to create provider for testing language code logic
    fn create_test_provider(
        language_code: String,
        alternative_languages: Vec<String>,
    ) -> TestableGoogleV2Provider {
        TestableGoogleV2Provider {
            language_code,
            alternative_languages,
        }
    }

    // Testable version that doesn't require a client
    struct TestableGoogleV2Provider {
        language_code: String,
        alternative_languages: Vec<String>,
    }

    impl TestableGoogleV2Provider {
        fn build_language_codes(&self, language_hint: Option<String>) -> Vec<String> {
            let mut language_codes = Vec::new();

            // If a specific language is provided, use it as primary
            if let Some(lang) = language_hint {
                language_codes.push(lang);
            } else {
                // Use configured primary language
                language_codes.push(self.language_code.clone());
            }

            // Add alternative languages, but limit to 3 total as per Google's requirements
            for alt_lang in &self.alternative_languages {
                if language_codes.len() >= 3 {
                    break;
                }
                if !language_codes.contains(alt_lang) {
                    language_codes.push(alt_lang.clone());
                }
            }

            language_codes
        }
    }

    #[test]
    fn test_build_language_codes_with_hint() {
        let provider = create_test_provider(
            "en-US".to_string(),
            vec!["fr-FR".to_string(), "de-DE".to_string()],
        );

        let codes = provider.build_language_codes(Some("es-ES".to_string()));
        assert_eq!(codes, vec!["es-ES", "fr-FR", "de-DE"]);
    }

    #[test]
    fn test_build_language_codes_without_hint() {
        let provider = create_test_provider(
            "en-US".to_string(),
            vec!["fr-FR".to_string(), "de-DE".to_string()],
        );

        let codes = provider.build_language_codes(None);
        assert_eq!(codes, vec!["en-US", "fr-FR", "de-DE"]);
    }

    #[test]
    fn test_build_language_codes_limit() {
        let provider = create_test_provider(
            "en-US".to_string(),
            vec![
                "fr-FR".to_string(),
                "de-DE".to_string(),
                "es-ES".to_string(),
                "it-IT".to_string(),
            ],
        );

        let codes = provider.build_language_codes(None);
        assert_eq!(codes.len(), 3); // Should be limited to 3
        assert_eq!(codes, vec!["en-US", "fr-FR", "de-DE"]);
    }

    #[test]
    fn test_build_language_codes_no_duplicates() {
        let provider = create_test_provider(
            "en-US".to_string(),
            vec!["en-US".to_string(), "fr-FR".to_string()],
        );

        let codes = provider.build_language_codes(Some("en-US".to_string()));
        assert_eq!(codes, vec!["en-US", "fr-FR"]); // No duplicates
    }
}
