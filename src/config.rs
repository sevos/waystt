use anyhow::Result;
use std::path::Path;

/// Configuration for waystt loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    pub openai_api_key: Option<String>,
    pub audio_buffer_duration_seconds: usize,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub whisper_model: String,
    pub whisper_language: String,
    pub whisper_timeout_seconds: u64,
    pub whisper_max_retries: u32,
    pub rust_log: String,
    pub enable_audio_feedback: bool,
    pub beep_volume: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            audio_buffer_duration_seconds: 300, // 5 minutes
            audio_sample_rate: 16000,           // Optimized for Whisper
            audio_channels: 1,                  // Mono
            whisper_model: "whisper-1".to_string(),
            whisper_language: "auto".to_string(),
            whisper_timeout_seconds: 60,
            whisper_max_retries: 3,
            rust_log: "info".to_string(),
            enable_audio_feedback: true,
            beep_volume: 0.1,
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

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.openai_api_key.is_none() {
            return Err(anyhow::anyhow!(
                "OPENAI_API_KEY is required for transcription. Please set it in your .env file."
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
    use std::env;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    // Mutex to ensure tests that modify environment variables run sequentially
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // Helper function to clear all waystt environment variables
    fn clear_env_vars() {
        env::remove_var("OPENAI_API_KEY");
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
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.openai_api_key, None);
        assert_eq!(config.audio_buffer_duration_seconds, 300);
        assert_eq!(config.audio_sample_rate, 16000);
        assert_eq!(config.audio_channels, 1);
        assert_eq!(config.whisper_model, "whisper-1");
        assert_eq!(config.whisper_language, "auto");
        assert_eq!(config.rust_log, "info");
        assert!(config.enable_audio_feedback);
        assert_eq!(config.beep_volume, 0.1);
    }

    #[test]
    fn test_config_from_env_defaults() {
        let _lock = ENV_MUTEX.lock().unwrap();

        // Clear all environment variables first
        clear_env_vars();

        let config = Config::from_env();
        assert_eq!(config.openai_api_key, None);
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

    #[test]
    fn test_config_from_env_variables() {
        let _lock = ENV_MUTEX.lock().unwrap();

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

        let config = Config::from_env();
        assert_eq!(config.openai_api_key, Some("test-api-key".to_string()));
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

    #[test]
    fn test_config_from_env_invalid_numbers() {
        let _lock = ENV_MUTEX.lock().unwrap();

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

    #[test]
    fn test_load_env_file() {
        let _lock = ENV_MUTEX.lock().unwrap();

        clear_env_vars();

        // Create a temporary .env file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "OPENAI_API_KEY=file-api-key").unwrap();
        writeln!(temp_file, "AUDIO_BUFFER_DURATION_SECONDS=120").unwrap();
        writeln!(temp_file, "WHISPER_MODEL=whisper-base").unwrap();
        writeln!(temp_file, "RUST_LOG=warn").unwrap();

        // Load config from file
        let config = Config::load_env_file(temp_file.path()).unwrap();

        assert_eq!(config.openai_api_key, Some("file-api-key".to_string()));
        assert_eq!(config.audio_buffer_duration_seconds, 120);
        assert_eq!(config.whisper_model, "whisper-base");
        assert_eq!(config.rust_log, "warn");

        // Other values should be defaults
        assert_eq!(config.audio_sample_rate, 16000);
        assert_eq!(config.audio_channels, 1);
        assert_eq!(config.whisper_language, "auto");

        clear_env_vars();
    }

    #[test]
    fn test_load_nonexistent_env_file() {
        let result = Config::load_env_file("/nonexistent/path/.env");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_success() {
        let mut config = Config::default();
        config.openai_api_key = Some("test-key".to_string());

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
        let mut config = Config::default();
        config.openai_api_key = Some("test-key".to_string());
        config.audio_buffer_duration_seconds = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AUDIO_BUFFER_DURATION_SECONDS"));
    }

    #[test]
    fn test_config_validation_invalid_sample_rate() {
        let mut config = Config::default();
        config.openai_api_key = Some("test-key".to_string());
        config.audio_sample_rate = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AUDIO_SAMPLE_RATE"));
    }

    #[test]
    fn test_config_validation_invalid_channels() {
        let mut config = Config::default();
        config.openai_api_key = Some("test-key".to_string());
        config.audio_channels = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AUDIO_CHANNELS"));
    }

    #[test]
    fn test_config_validation_invalid_beep_volume() {
        let mut config = Config::default();
        config.openai_api_key = Some("test-key".to_string());

        // Test negative volume
        config.beep_volume = -0.1;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BEEP_VOLUME"));

        // Test volume > 1.0
        config.beep_volume = 1.1;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BEEP_VOLUME"));
    }

    #[test]
    fn test_config_audio_feedback_env_vars() {
        let _lock = ENV_MUTEX.lock().unwrap();
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

    #[test]
    fn test_config_audio_feedback_invalid_env_vars() {
        let _lock = ENV_MUTEX.lock().unwrap();
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
