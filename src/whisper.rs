use anyhow::Result;
use reqwest;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WhisperError {
    #[error("Authentication failed - invalid API key")]
    AuthenticationFailed,
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("File too large: {0} bytes (max 25MB)")]
    FileTooLarge(usize),
    #[error("API response error: {0}")]
    ApiError(String),
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub struct WhisperClient {
    api_key: String,
    client: reqwest::Client,
    max_retries: u32,
    model: String,
    base_url: String,
}

impl WhisperClient {
    pub fn new_with_options(
        api_key: String,
        timeout_seconds: Option<u64>,
        max_retries: Option<u32>,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<Self, WhisperError> {
        let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(WhisperError::NetworkError)?;

        Ok(WhisperClient {
            api_key,
            client,
            max_retries: max_retries.unwrap_or(3),
            model: model.unwrap_or_else(|| "whisper-1".to_string()),
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })
    }

    pub async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, WhisperError> {
        // Check file size (25MB limit for OpenAI Whisper API)
        const MAX_FILE_SIZE: usize = 25 * 1024 * 1024;
        if audio_data.len() > MAX_FILE_SIZE {
            return Err(WhisperError::FileTooLarge(audio_data.len()));
        }

        let mut retries = 0;
        loop {
            match self.transcribe_attempt(&audio_data, language.as_deref()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    retries += 1;
                    if retries > self.max_retries {
                        return Err(e);
                    }
                    
                    // Don't retry on authentication errors
                    if matches!(e, WhisperError::AuthenticationFailed) {
                        return Err(e);
                    }
                    
                    // Exponential backoff
                    let delay = Duration::from_millis(1000 * (1 << (retries - 1)).min(8));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn transcribe_attempt(
        &self,
        audio_data: &[u8],
        language: Option<&str>,
    ) -> Result<String, WhisperError> {
        let url = format!("{}/audio/transcriptions", self.base_url);
        
        // Create multipart form
        let audio_part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(WhisperError::NetworkError)?;

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
            .map_err(WhisperError::NetworkError)?;

        let status = response.status();
        let response_text = response.text().await.map_err(WhisperError::NetworkError)?;

        match status {
            reqwest::StatusCode::OK => {
                let json: Value = serde_json::from_str(&response_text)?;
                let text = json
                    .get("text")
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| WhisperError::ApiError("No text field in response".to_string()))?;
                Ok(text.to_string())
            }
            reqwest::StatusCode::UNAUTHORIZED => Err(WhisperError::AuthenticationFailed),
            _ => Err(WhisperError::ApiError(format!(
                "HTTP {}: {}",
                status, response_text
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_client_creation() {
        let client = WhisperClient::new_with_options(
            "test-key".to_string(),
            Some(30),
            Some(3),
            Some("whisper-1".to_string()),
            None,
        );
        assert!(client.is_ok());
    }

    #[test]
    fn test_file_size_validation() {
        let client = WhisperClient::new_with_options(
            "test-key".to_string(),
            None,
            None,
            None,
            None,
        ).unwrap();

        // Test file too large
        let large_data = vec![0u8; 26 * 1024 * 1024]; // 26MB
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(client.transcribe_with_language(large_data, None));
        
        assert!(matches!(result, Err(WhisperError::FileTooLarge(_))));
    }
}