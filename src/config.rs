#![allow(clippy::float_cmp)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Profile configuration for transcription sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionProfile {
    pub model: Option<String>,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub hooks: Option<crate::socket::Hooks>,
    pub vad_config: Option<crate::socket::VadConfig>,
}

/// Configuration for HotLine loaded from environment variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub audio_buffer_duration_seconds: usize,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub whisper_model: String,
    pub whisper_language: String,
    pub whisper_timeout_seconds: u64,
    pub whisper_max_retries: u32,
    pub realtime_model: String,
    pub rust_log: String,
    pub enable_audio_feedback: bool,
    pub beep_volume: f32,
    #[serde(default)]
    pub profiles: HashMap<String, TranscriptionProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            openai_base_url: None,
            audio_buffer_duration_seconds: 300, // 5 minutes
            audio_sample_rate: 16000,           // Optimized for Whisper
            audio_channels: 1,                  // Mono
            whisper_model: "whisper-1".to_string(),
            whisper_language: "auto".to_string(),
            whisper_timeout_seconds: 60,
            whisper_max_retries: 3,
            realtime_model: "whisper-1".to_string(),
            rust_log: "info".to_string(),
            enable_audio_feedback: true,
            beep_volume: 0.1,
            profiles: HashMap::new(),
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    #[allow(clippy::field_reassign_with_default)]
    pub fn from_env() -> Self {
        let mut config = Config::default();

        // Load OpenAI API key
        config.openai_api_key = std::env::var("OPENAI_API_KEY").ok();

        // Load OpenAI base URL
        config.openai_base_url = std::env::var("OPENAI_BASE_URL").ok();

        // Load audio configuration
        if let Ok(duration) = std::env::var("AUDIO_BUFFER_DURATION_SECONDS") {
            if let Ok(parsed) = duration.parse::<usize>() {
                config.audio_buffer_duration_seconds = parsed;
            }
        }

        if let Ok(sample_rate) = std::env::var("AUDIO_SAMPLE_RATE") {
            if let Ok(parsed) = sample_rate.parse::<u32>() {
                config.audio_sample_rate = parsed;
            }
        }

        if let Ok(channels) = std::env::var("AUDIO_CHANNELS") {
            if let Ok(parsed) = channels.parse::<u16>() {
                config.audio_channels = parsed;
            }
        }

        // Load transcription configuration
        if let Ok(model) = std::env::var("WHISPER_MODEL") {
            config.whisper_model = model;
        }

        if let Ok(language) = std::env::var("WHISPER_LANGUAGE") {
            config.whisper_language = language;
        }

        if let Ok(timeout) = std::env::var("WHISPER_TIMEOUT_SECONDS") {
            if let Ok(parsed) = timeout.parse::<u64>() {
                config.whisper_timeout_seconds = parsed;
            }
        }

        if let Ok(retries) = std::env::var("WHISPER_MAX_RETRIES") {
            if let Ok(parsed) = retries.parse::<u32>() {
                config.whisper_max_retries = parsed;
            }
        }

        // Load Realtime API model
        if let Ok(model) = std::env::var("REALTIME_MODEL") {
            config.realtime_model = model;
        }

        // Load logging configuration
        if let Ok(log_level) = std::env::var("RUST_LOG") {
            config.rust_log = log_level;
        }

        // Load audio feedback configuration
        if let Ok(enabled) = std::env::var("ENABLE_AUDIO_FEEDBACK") {
            config.enable_audio_feedback = enabled.to_lowercase() == "true";
        }

        if let Ok(volume) = std::env::var("BEEP_VOLUME") {
            if let Ok(parsed) = volume.parse::<f32>() {
                config.beep_volume = parsed.clamp(0.0, 1.0);
            }
        }

        config
    }

    /// Load environment file and return config
    pub fn load_env_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        dotenvy::from_path(path)?;
        Ok(Self::from_env())
    }

    /// Load configuration from TOML file
    pub fn from_toml<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&contents)?;

        // Merge with environment variables (env vars take precedence)
        config.merge_with_env();
        Ok(config)
    }

    /// Get the default TOML config path
    pub fn get_toml_config_path() -> std::path::PathBuf {
        if let Ok(config_dir) = std::env::var("XDG_CONFIG_DIR") {
            std::path::PathBuf::from(config_dir)
                .join("hotline")
                .join("hotline.toml")
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| {
                    std::env::var("HOME")
                        .map_or_else(|_| std::path::PathBuf::from("."), std::path::PathBuf::from)
                })
                .join("hotline")
                .join("hotline.toml")
        }
    }

    /// Load configuration with proper precedence:
    /// 1. Command-line arguments (handled by caller)
    /// 2. Environment variables
    /// 3. TOML configuration file
    /// 4. Default values
    pub fn load_with_precedence() -> Result<Self> {
        let toml_path = Self::get_toml_config_path();

        // Start with defaults
        let mut config = if toml_path.exists() {
            // Load from TOML if it exists
            match Self::from_toml(&toml_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load TOML config from {}: {}",
                        toml_path.display(),
                        e
                    );
                    Self::default()
                }
            }
        } else {
            Self::default()
        };

        // Merge with environment variables (they take precedence)
        config.merge_with_env();

        Ok(config)
    }

    /// Merge current config with environment variables (env vars take precedence)
    fn merge_with_env(&mut self) {
        // Load OpenAI API key
        if let Ok(val) = std::env::var("OPENAI_API_KEY") {
            self.openai_api_key = Some(val);
        }

        // Load OpenAI base URL
        if let Ok(val) = std::env::var("OPENAI_BASE_URL") {
            self.openai_base_url = Some(val);
        }

        // Load audio configuration
        if let Ok(duration) = std::env::var("AUDIO_BUFFER_DURATION_SECONDS") {
            if let Ok(parsed) = duration.parse::<usize>() {
                self.audio_buffer_duration_seconds = parsed;
            }
        }

        if let Ok(sample_rate) = std::env::var("AUDIO_SAMPLE_RATE") {
            if let Ok(parsed) = sample_rate.parse::<u32>() {
                self.audio_sample_rate = parsed;
            }
        }

        if let Ok(channels) = std::env::var("AUDIO_CHANNELS") {
            if let Ok(parsed) = channels.parse::<u16>() {
                self.audio_channels = parsed;
            }
        }

        // Load transcription configuration
        if let Ok(model) = std::env::var("WHISPER_MODEL") {
            self.whisper_model = model;
        }

        if let Ok(language) = std::env::var("WHISPER_LANGUAGE") {
            self.whisper_language = language;
        }

        if let Ok(timeout) = std::env::var("WHISPER_TIMEOUT_SECONDS") {
            if let Ok(parsed) = timeout.parse::<u64>() {
                self.whisper_timeout_seconds = parsed;
            }
        }

        if let Ok(retries) = std::env::var("WHISPER_MAX_RETRIES") {
            if let Ok(parsed) = retries.parse::<u32>() {
                self.whisper_max_retries = parsed;
            }
        }

        // Load Realtime API model
        if let Ok(model) = std::env::var("REALTIME_MODEL") {
            self.realtime_model = model;
        }

        // Load logging configuration
        if let Ok(log_level) = std::env::var("RUST_LOG") {
            self.rust_log = log_level;
        }

        // Load audio feedback configuration
        if let Ok(enabled) = std::env::var("ENABLE_AUDIO_FEEDBACK") {
            self.enable_audio_feedback = enabled.to_lowercase() == "true";
        }

        if let Ok(volume) = std::env::var("BEEP_VOLUME") {
            if let Ok(parsed) = volume.parse::<f32>() {
                self.beep_volume = parsed.clamp(0.0, 1.0);
            }
        }
    }

    /// Get a transcription profile by name
    pub fn get_profile(&self, name: &str) -> Option<&TranscriptionProfile> {
        self.profiles.get(name)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // OpenAI API key is required
        if self.openai_api_key.is_none() {
            return Err(anyhow::anyhow!(
                "OPENAI_API_KEY is required. Please set it in your .env file."
            ));
        }

        if self.audio_buffer_duration_seconds == 0 {
            return Err(anyhow::anyhow!(
                "AUDIO_BUFFER_DURATION_SECONDS must be greater than 0"
            ));
        }

        if self.audio_sample_rate == 0 {
            return Err(anyhow::anyhow!("AUDIO_SAMPLE_RATE must be greater than 0"));
        }

        if self.audio_channels == 0 {
            return Err(anyhow::anyhow!("AUDIO_CHANNELS must be greater than 0"));
        }

        if self.beep_volume < 0.0 || self.beep_volume > 1.0 {
            return Err(anyhow::anyhow!(
                "BEEP_VOLUME must be between 0.0 and 1.0, got: {}",
                self.beep_volume
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_MUTEX;
    use std::env;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Helper function to clear all hotline environment variables
    fn clear_env_vars() {
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("OPENAI_BASE_URL");
        env::remove_var("TRANSCRIPTION_PROVIDER");
        env::remove_var("AUDIO_BUFFER_DURATION_SECONDS");
        env::remove_var("AUDIO_SAMPLE_RATE");
        env::remove_var("AUDIO_CHANNELS");
        env::remove_var("WHISPER_MODEL");
        env::remove_var("WHISPER_LANGUAGE");
        env::remove_var("WHISPER_TIMEOUT_SECONDS");
        env::remove_var("WHISPER_MAX_RETRIES");
        env::remove_var("RUST_LOG");
        env::remove_var("ENABLE_AUDIO_FEEDBACK");
        env::remove_var("BEEP_VOLUME");
        env::remove_var("GOOGLE_APPLICATION_CREDENTIALS");
        env::remove_var("GOOGLE_SPEECH_LANGUAGE_CODE");
        env::remove_var("GOOGLE_SPEECH_MODEL");
        env::remove_var("GOOGLE_SPEECH_ALTERNATIVE_LANGUAGES");
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.openai_api_key, None);
        assert_eq!(config.openai_base_url, None);
        assert_eq!(config.audio_buffer_duration_seconds, 300);
        assert_eq!(config.audio_sample_rate, 16000);
        assert_eq!(config.audio_channels, 1);
        assert_eq!(config.whisper_model, "whisper-1");
        assert_eq!(config.whisper_language, "auto");
        assert_eq!(config.whisper_timeout_seconds, 60);
        assert_eq!(config.whisper_max_retries, 3);
        assert_eq!(config.rust_log, "info");
        assert!(config.enable_audio_feedback);
        assert_eq!(config.beep_volume, 0.1);
    }

    #[tokio::test]
    async fn test_config_from_env_defaults() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Clear all environment variables first
            clear_env_vars();

            let config = Config::from_env();
            assert_eq!(config.openai_api_key, None);
            assert_eq!(config.openai_base_url, None);
            assert_eq!(config.audio_buffer_duration_seconds, 300);
            assert_eq!(config.audio_sample_rate, 16000);
            assert_eq!(config.audio_channels, 1);
            assert_eq!(config.whisper_model, "whisper-1");
            assert_eq!(config.whisper_language, "auto");
            assert_eq!(config.whisper_timeout_seconds, 60);
            assert_eq!(config.whisper_max_retries, 3);
            assert_eq!(config.rust_log, "info");

            // Clean up after test
            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_config_from_env_variables() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Clear environment variables first to ensure clean state
            clear_env_vars();

            // Set environment variables
            env::set_var("OPENAI_API_KEY", "test-api-key");
            env::set_var("AUDIO_BUFFER_DURATION_SECONDS", "600");
            env::set_var("AUDIO_SAMPLE_RATE", "44100");
            env::set_var("AUDIO_CHANNELS", "2");
            env::set_var("WHISPER_MODEL", "whisper-large");
            env::set_var("WHISPER_LANGUAGE", "en");
            env::set_var("WHISPER_TIMEOUT_SECONDS", "120");
            env::set_var("WHISPER_MAX_RETRIES", "5");
            env::set_var("RUST_LOG", "debug");
            env::set_var("OPENAI_BASE_URL", "http://localhost:8080");

            let config = Config::from_env();
            assert_eq!(config.openai_api_key, Some("test-api-key".to_string()));
            assert_eq!(
                config.openai_base_url,
                Some("http://localhost:8080".to_string())
            );
            assert_eq!(config.audio_buffer_duration_seconds, 600);
            assert_eq!(config.audio_sample_rate, 44100);
            assert_eq!(config.audio_channels, 2);
            assert_eq!(config.whisper_model, "whisper-large");
            assert_eq!(config.whisper_language, "en");
            assert_eq!(config.whisper_timeout_seconds, 120);
            assert_eq!(config.whisper_max_retries, 5);
            assert_eq!(config.rust_log, "debug");

            // Clean up after test
            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_config_from_env_invalid_numbers() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            // Clear at the start
            clear_env_vars();

            // Set invalid numeric values
            env::set_var("AUDIO_BUFFER_DURATION_SECONDS", "invalid");
            env::set_var("AUDIO_SAMPLE_RATE", "not-a-number");
            env::set_var("AUDIO_CHANNELS", "bad");
            env::set_var("WHISPER_TIMEOUT_SECONDS", "invalid");
            env::set_var("WHISPER_MAX_RETRIES", "bad");

            let config = Config::from_env();

            // Should fallback to defaults for invalid values
            assert_eq!(config.audio_buffer_duration_seconds, 300);
            assert_eq!(config.audio_sample_rate, 16000);
            assert_eq!(config.audio_channels, 1);
            assert_eq!(config.whisper_timeout_seconds, 60);
            assert_eq!(config.whisper_max_retries, 3);

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_load_env_file() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;

            clear_env_vars();

            // Create a temporary .env file
            let mut temp_file = NamedTempFile::new().unwrap();
            writeln!(temp_file, "OPENAI_API_KEY=file-api-key").unwrap();
            writeln!(temp_file, "AUDIO_BUFFER_DURATION_SECONDS=120").unwrap();
            writeln!(temp_file, "WHISPER_MODEL=whisper-base").unwrap();
            writeln!(temp_file, "RUST_LOG=warn").unwrap();
            writeln!(temp_file, "OPENAI_BASE_URL=http://localhost:8080").unwrap();

            // Load config from file
            let config = Config::load_env_file(temp_file.path()).unwrap();

            assert_eq!(config.openai_api_key, Some("file-api-key".to_string()));
            assert_eq!(
                config.openai_base_url,
                Some("http://localhost:8080".to_string())
            );
            assert_eq!(config.audio_buffer_duration_seconds, 120);
            assert_eq!(config.whisper_model, "whisper-base");
            assert_eq!(config.rust_log, "warn");

            // Other values should be defaults
            assert_eq!(config.audio_sample_rate, 16000);
            assert_eq!(config.audio_channels, 1);
            assert_eq!(config.whisper_language, "auto");

            clear_env_vars();
        }
    }

    #[test]
    fn test_load_nonexistent_env_file() {
        let result = Config::load_env_file("/nonexistent/path/.env");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_success() {
        let config = Config {
            openai_api_key: Some("test-key".to_string()),
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_missing_api_key() {
        let config = Config::default(); // No API key

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("OPENAI_API_KEY is required"));
    }

    #[test]
    fn test_config_validation_invalid_duration() {
        let config = Config {
            openai_api_key: Some("test-key".to_string()),
            audio_buffer_duration_seconds: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AUDIO_BUFFER_DURATION_SECONDS"));
    }

    #[test]
    fn test_config_validation_invalid_sample_rate() {
        let config = Config {
            openai_api_key: Some("test-key".to_string()),
            audio_sample_rate: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AUDIO_SAMPLE_RATE"));
    }

    #[test]
    fn test_config_validation_invalid_channels() {
        let config = Config {
            openai_api_key: Some("test-key".to_string()),
            audio_channels: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AUDIO_CHANNELS"));
    }

    #[test]
    fn test_config_validation_invalid_beep_volume() {
        // Test negative volume
        let config = Config {
            openai_api_key: Some("test-key".to_string()),
            beep_volume: -0.1,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BEEP_VOLUME"));

        // Test volume > 1.0
        let config2 = Config {
            openai_api_key: Some("test-key".to_string()),
            beep_volume: 1.1,
            ..Default::default()
        };
        let result = config2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BEEP_VOLUME"));
    }

    #[tokio::test]
    async fn test_config_audio_feedback_env_vars() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            // Test enabled audio feedback
            env::set_var("ENABLE_AUDIO_FEEDBACK", "true");
            env::set_var("BEEP_VOLUME", "0.5");

            let config = Config::from_env();
            assert!(config.enable_audio_feedback);
            assert_eq!(config.beep_volume, 0.5);

            clear_env_vars();

            // Test disabled audio feedback
            env::set_var("ENABLE_AUDIO_FEEDBACK", "false");
            env::set_var("BEEP_VOLUME", "0.8");

            let config = Config::from_env();
            assert!(!config.enable_audio_feedback);
            assert_eq!(config.beep_volume, 0.8);

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_config_audio_feedback_invalid_env_vars() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            // Test invalid volume values
            env::set_var("BEEP_VOLUME", "invalid");
            let config = Config::from_env();
            assert_eq!(config.beep_volume, 0.1); // Should use default

            // Test volume clamping
            env::set_var("BEEP_VOLUME", "2.0");
            let config = Config::from_env();
            assert_eq!(config.beep_volume, 1.0); // Should be clamped to 1.0

            env::set_var("BEEP_VOLUME", "-0.5");
            let config = Config::from_env();
            assert_eq!(config.beep_volume, 0.0); // Should be clamped to 0.0

            clear_env_vars();
        }
    }
}
