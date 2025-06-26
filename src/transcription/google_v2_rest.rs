use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};

use crate::transcription::{TranscriptionError, TranscriptionProvider};

pub struct GoogleV2RestProvider {
    client: Client,
    project_id: String,
    language_code: String,
    model: String,
    alternative_languages: Vec<String>,
    credentials_path: String,
}

#[derive(Serialize)]
struct RecognizeRequest {
    config: RecognitionConfig,
    content: String, // base64-encoded audio data directly
}

#[derive(Serialize)]
struct RecognitionConfig {
    #[serde(rename = "autoDecodingConfig")]
    auto_decoding_config: AutoDecodingConfig,
    model: String,
    #[serde(rename = "languageCodes")]
    language_codes: Vec<String>,
    features: RecognitionFeatures,
}

#[derive(Serialize)]
struct AutoDecodingConfig {}

#[derive(Serialize)]
struct RecognitionFeatures {
    #[serde(rename = "enableAutomaticPunctuation")]
    enable_automatic_punctuation: bool,
    #[serde(rename = "enableWordTimeOffsets")]
    enable_word_time_offsets: bool,
    #[serde(rename = "enableWordConfidence")]
    enable_word_confidence: bool,
}

// Remove AudioContent struct since we're using content directly in RecognizeRequest

#[derive(Deserialize)]
struct RecognizeResponse {
    results: Vec<SpeechRecognitionResult>,
}

#[derive(Deserialize)]
struct SpeechRecognitionResult {
    alternatives: Vec<SpeechRecognitionAlternative>,
}

#[derive(Deserialize)]
struct SpeechRecognitionAlternative {
    transcript: String,
}

impl GoogleV2RestProvider {
    pub async fn new(
        credentials_path: String,
        language_code: String,
        model: String,
        alternative_languages: Vec<String>,
    ) -> Result<Self, TranscriptionError> {
        // Read service account key to get project ID
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

        // Extract project ID
        let project_id = service_account_key.project_id.clone().ok_or_else(|| {
            TranscriptionError::ConfigurationError(
                "No project_id in service account key".to_string(),
            )
        })?;

        // Create HTTP client
        let client = Client::new();

        Ok(Self {
            client,
            project_id,
            language_code,
            model,
            alternative_languages,
            credentials_path,
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

    async fn get_access_token(&self) -> Result<String, TranscriptionError> {
        // Read service account key
        let service_account_key = tokio::fs::read_to_string(&self.credentials_path)
            .await
            .map_err(|e| {
                TranscriptionError::ConfigurationError(format!(
                    "Failed to read service account key: {}",
                    e
                ))
            })?;

        let service_account_key: ServiceAccountKey = serde_json::from_str(&service_account_key)
            .map_err(|e| {
                TranscriptionError::ConfigurationError(format!(
                    "Failed to parse service account key: {}",
                    e
                ))
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

        Ok(token.token().unwrap_or("").to_string())
    }
}

#[async_trait]
impl TranscriptionProvider for GoogleV2RestProvider {
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

        // Get access token
        let access_token = self.get_access_token().await?;

        // Encode audio as base64
        let audio_base64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &audio_data);

        // Build language codes
        let language_codes = self.build_language_codes(language);

        // Build request payload
        let request_payload = RecognizeRequest {
            config: RecognitionConfig {
                auto_decoding_config: AutoDecodingConfig {},
                model: self.model.clone(),
                language_codes,
                features: RecognitionFeatures {
                    enable_automatic_punctuation: false,
                    enable_word_time_offsets: false,
                    enable_word_confidence: false,
                },
            },
            content: audio_base64,
        };

        // Build API URL
        let url = format!(
            "https://speech.googleapis.com/v2/projects/{}/locations/global/recognizers/_:recognize",
            self.project_id
        );

        // Make HTTP request
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&request_payload)
            .send()
            .await
            .map_err(|e| TranscriptionError::NetworkError(format!("HTTP request failed: {}", e)))?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(TranscriptionError::ApiError(format!(
                "Google Speech API error: {} - {}",
                status, error_text
            )));
        }

        // Parse response
        let recognize_response: RecognizeResponse = response.json().await.map_err(|e| {
            TranscriptionError::ApiError(format!("Failed to parse response: {}", e))
        })?;

        // Extract transcription from response
        if let Some(result) = recognize_response.results.first() {
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
    ) -> TestableGoogleV2RestProvider {
        TestableGoogleV2RestProvider {
            language_code,
            alternative_languages,
        }
    }

    // Testable version that doesn't require a client
    struct TestableGoogleV2RestProvider {
        language_code: String,
        alternative_languages: Vec<String>,
    }

    impl TestableGoogleV2RestProvider {
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
