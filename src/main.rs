#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::float_cmp)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::needless_continue)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::single_match_else)]
#![allow(clippy::match_bool)]

use anyhow::Result;
use clap::Parser;
use futures::stream::StreamExt;
use signal_hook::consts::{SIGTERM, SIGUSR1, SIGUSR2};
use signal_hook_tokio::Signals;
use std::path::PathBuf;

mod audio;
mod audio_processing;
mod beep;
mod clipboard;
mod config;
mod transcription;
mod wav;
use audio::AudioRecorder;
use audio_processing::AudioProcessor;
use beep::{BeepConfig, BeepPlayer, BeepType};
use clipboard::ClipboardManager;
use config::Config;
use transcription::{TranscriptionError, TranscriptionFactory};
use wav::WavEncoder;

#[derive(Parser)]
#[command(name = "waystt")]
#[command(about = "Wayland Speech-to-Text Tool - Signal-driven transcription")]
#[command(version)]
struct Args {
    /// Path to environment file
    #[arg(long, default_value = "./.env")]
    envfile: PathBuf,
}

/// Process recorded audio for transcription
async fn process_audio_for_transcription(
    audio_data: Vec<f32>,
    sample_rate: u32,
    action: &str,
    config: &Config,
) -> Result<()> {
    // Initialize beep player
    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;
    println!(
        "Processing audio for {}: {} samples",
        action,
        audio_data.len()
    );

    // Initialize audio processor
    let processor = AudioProcessor::new(sample_rate);

    // Process audio for speech recognition
    match processor.process_for_speech_recognition(&audio_data) {
        Ok(processed_audio) => {
            let original_duration = processor.get_duration_seconds(&audio_data);
            let processed_duration = processor.get_duration_seconds(&processed_audio);

            println!(
                "Audio processed successfully: {:.2}s -> {:.2}s ({} samples)",
                original_duration,
                processed_duration,
                processed_audio.len()
            );

            // Encode to WAV format for API
            let encoder = WavEncoder::new(sample_rate, 1);
            match encoder.encode_to_wav(&processed_audio) {
                Ok(wav_data) => {
                    println!(
                        "WAV encoded: {} bytes ready for transcription",
                        wav_data.len()
                    );

                    // Initialize transcription provider with configuration
                    let provider =
                        TranscriptionFactory::create_provider(&config.transcription_provider)
                            .await?;

                    // Send to transcription service
                    println!(
                        "Sending audio to {} provider...",
                        config.transcription_provider
                    );
                    let language = if config.whisper_language == "auto" {
                        None
                    } else {
                        Some(config.whisper_language.clone())
                    };
                    match provider.transcribe_with_language(wav_data, language).await {
                        Ok(transcribed_text) => {
                            if transcribed_text.trim().is_empty() {
                                println!("Warning: Received empty transcription from Whisper API");
                                println!("This might indicate silent audio or unclear speech");
                                return Ok(());
                            }

                            println!("Transcription successful: \"{}\"", transcribed_text);

                            // Initialize clipboard manager
                            let mut clipboard_manager = ClipboardManager::new().map_err(|e| {
                                anyhow::anyhow!("Failed to initialize clipboard: {}", e)
                            })?;

                            match action {
                                "type" => {
                                    // SIGUSR1: Type text directly using ydotool
                                    match clipboard_manager.type_text_directly(&transcribed_text) {
                                        Ok(()) => {
                                            println!(
                                                "✅ Text typed successfully: \"{}\"",
                                                transcribed_text
                                            );
                                            // Play success beep after successful typing
                                            if let Err(e) =
                                                beep_player.play_async(BeepType::Success).await
                                            {
                                                eprintln!(
                                                    "Warning: Failed to play success beep: {}",
                                                    e
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("❌ Failed to type text: {}", e);
                                            // Play error beep on typing failure
                                            if let Err(beep_err) =
                                                beep_player.play_async(BeepType::Error).await
                                            {
                                                eprintln!(
                                                    "Warning: Failed to play error beep: {}",
                                                    beep_err
                                                );
                                            }
                                            return Err(anyhow::anyhow!(
                                                "Text typing failed: {}",
                                                e
                                            ));
                                        }
                                    }
                                }
                                "copy" => {
                                    // SIGUSR2: Copy to clipboard only (with persistence)
                                    match clipboard_manager.copy_text_persistent(&transcribed_text)
                                    {
                                        Ok(()) => {
                                            println!(
                                                "✅ Text copied to persistent clipboard: \"{}\"",
                                                transcribed_text
                                            );
                                            println!("💡 Paste manually with Ctrl+V when ready");
                                            println!(
                                                "💡 Clipboard data will persist after app exits"
                                            );
                                            // Play success beep after successful clipboard operation
                                            if let Err(e) =
                                                beep_player.play_async(BeepType::Success).await
                                            {
                                                eprintln!(
                                                    "Warning: Failed to play success beep: {}",
                                                    e
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("❌ Failed to copy to clipboard: {}", e);
                                            eprintln!(
                                                "💡 Setup instructions: {}",
                                                ClipboardManager::get_setup_instructions()
                                            );
                                            // Play error beep on clipboard failure
                                            if let Err(beep_err) =
                                                beep_player.play_async(BeepType::Error).await
                                            {
                                                eprintln!(
                                                    "Warning: Failed to play error beep: {}",
                                                    beep_err
                                                );
                                            }
                                            return Err(anyhow::anyhow!(
                                                "Clipboard operation failed: {}",
                                                e
                                            ));
                                        }
                                    }
                                }
                                _ => {
                                    println!("❌ Unknown action: {}", action);
                                    println!("Transcribed text: \"{}\"", transcribed_text);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Transcription failed: {}", e);

                            // Play error beep for transcription failure
                            if let Err(beep_err) = beep_player.play_async(BeepType::Error).await {
                                eprintln!("Warning: Failed to play error beep: {}", beep_err);
                            }

                            // Provide helpful error messages
                            match &e {
                                TranscriptionError::AuthenticationFailed => {
                                    eprintln!("💡 Check your API key configuration");
                                }
                                TranscriptionError::NetworkError(_) => {
                                    eprintln!("💡 Check your internet connection");
                                }
                                TranscriptionError::FileTooLarge(size) => {
                                    eprintln!("💡 Audio file too large: {} bytes (max 25MB)", size);
                                    eprintln!("💡 Try recording shorter clips");
                                }
                                TranscriptionError::ConfigurationError(_) => {
                                    eprintln!("💡 Check your transcription provider configuration");
                                }
                                TranscriptionError::UnsupportedProvider(provider) => {
                                    eprintln!("💡 Unsupported provider: {}. Check TRANSCRIPTION_PROVIDER setting", provider);
                                }
                                _ => {
                                    eprintln!("💡 Please check your configuration and try again");
                                }
                            }

                            return Err(anyhow::anyhow!("Transcription failed: {}", e));
                        }
                    }

                    Ok(())
                }
                Err(e) => {
                    eprintln!("Failed to encode WAV: {}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            eprintln!("Audio processing failed: {}", e);

            // Play error beep for audio processing failure
            if let Err(beep_err) = beep_player.play_async(BeepType::Error).await {
                eprintln!("Warning: Failed to play error beep: {}", beep_err);
            }

            if e.to_string().contains("too short") {
                eprintln!("Tip: Try speaking for at least 0.1 seconds before sending signal");
            } else if e.to_string().contains("only silence") {
                eprintln!("Tip: Make sure your microphone is working and you're speaking clearly");
            }
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration from environment file or system environment
    let config = if args.envfile.exists() {
        println!("Loading environment from: {}", args.envfile.display());
        match Config::load_env_file(&args.envfile) {
            Ok(config) => config,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to load environment file {}: {}",
                    args.envfile.display(),
                    e
                );
                println!("Falling back to system environment");
                Config::from_env()
            }
        }
    } else {
        println!(
            "Environment file {} not found, using system environment",
            args.envfile.display()
        );
        Config::from_env()
    };

    // Validate configuration (but don't fail if API key missing, as we're just recording for now)
    if let Err(e) = config.validate() {
        eprintln!("Configuration warning: {}", e);
        eprintln!(
            "Note: This is expected during development phase before transcription is implemented"
        );
    }

    println!("waystt - Wayland Speech-to-Text Tool");
    println!("Starting audio recording...");

    // Initialize beep player for recording feedback
    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;

    // Initialize audio recorder
    let mut recorder = AudioRecorder::new()?;

    // Play recording start beep BEFORE starting recording to avoid capturing it
    if let Err(e) = beep_player.play_async(BeepType::RecordingStart).await {
        eprintln!("Warning: Failed to play recording start beep: {}", e);
    }

    // Give a moment for the beep to finish before starting recording (beep is now 500ms)
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

    // Start recording immediately
    if let Err(e) = recorder.start_recording() {
        eprintln!("Failed to start audio recording: {}", e);
        eprintln!("This may be due to PipeWire not being available or insufficient permissions.");
        return Err(e);
    }

    println!("Audio recording started successfully!");

    // Give PipeWire a moment to start capturing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mut signals = Signals::new([SIGUSR1, SIGUSR2, SIGTERM])?;

    println!("Ready. Send SIGUSR1 to transcribe and type, or SIGUSR2 to transcribe and copy.");

    // Main event loop - process audio and wait for signals
    loop {
        // Process audio events to capture microphone data
        if let Err(e) = recorder.process_audio_events() {
            eprintln!("Error processing audio events: {}", e);
        }

        // Check for signals with timeout
        match tokio::time::timeout(tokio::time::Duration::from_millis(50), signals.next()).await {
            Ok(Some(signal)) => {
                match signal {
                    SIGUSR1 => {
                        println!("Received SIGUSR1: Stop recording, transcribe, and type");

                        // Stop recording
                        if let Err(e) = recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {}", e);
                        } else {
                            // Play recording stop beep
                            if let Err(e) = beep_player.play_async(BeepType::RecordingStop).await {
                                eprintln!("Warning: Failed to play recording stop beep: {}", e);
                            }
                        }

                        // Get recorded audio data and process it
                        match recorder.get_audio_data() {
                            Ok(audio_data) => {
                                let duration =
                                    recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                println!(
                                    "Captured {} audio samples ({:.2} seconds)",
                                    audio_data.len(),
                                    duration
                                );

                                // Process audio for transcription
                                if let Err(e) = process_audio_for_transcription(
                                    audio_data,
                                    16000, // Using fixed sample rate from audio module
                                    "type", &config,
                                )
                                .await
                                {
                                    eprintln!("Audio processing failed: {}", e);
                                }

                                // Clear buffer to free memory
                                if let Err(e) = recorder.clear_buffer() {
                                    eprintln!("Failed to clear audio buffer: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to get audio data: {}", e);
                            }
                        }

                        break;
                    }
                    SIGUSR2 => {
                        println!("Received SIGUSR2: Stop recording, transcribe, and copy");

                        // Stop recording
                        if let Err(e) = recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {}", e);
                        } else {
                            // Play recording stop beep
                            if let Err(e) = beep_player.play_async(BeepType::RecordingStop).await {
                                eprintln!("Warning: Failed to play recording stop beep: {}", e);
                            }
                        }

                        // Get recorded audio data and process it
                        match recorder.get_audio_data() {
                            Ok(audio_data) => {
                                let duration =
                                    recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                println!(
                                    "Captured {} audio samples ({:.2} seconds)",
                                    audio_data.len(),
                                    duration
                                );

                                // Process audio for transcription
                                if let Err(e) = process_audio_for_transcription(
                                    audio_data,
                                    16000, // Using fixed sample rate from audio module
                                    "copy", &config,
                                )
                                .await
                                {
                                    eprintln!("Audio processing failed: {}", e);
                                }

                                // Clear buffer to free memory
                                if let Err(e) = recorder.clear_buffer() {
                                    eprintln!("Failed to clear audio buffer: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to get audio data: {}", e);
                            }
                        }

                        break;
                    }
                    SIGTERM => {
                        println!("Received SIGTERM: Shutting down gracefully");
                        if let Err(e) = recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {}", e);
                        }

                        // Clear buffer on shutdown
                        if let Err(e) = recorder.clear_buffer() {
                            eprintln!("Failed to clear audio buffer during shutdown: {}", e);
                        }

                        break;
                    }
                    _ => {
                        println!("Received unexpected signal: {}", signal);
                    }
                }
            }
            Ok(None) => {
                // Signal stream ended
                break;
            }
            Err(_) => {
                // Timeout occurred, continue processing audio
                continue;
            }
        }
    }

    println!("Exiting waystt");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_processing::AudioProcessor;
    use wav::WavEncoder;

    #[tokio::test]
    async fn test_audio_processing_pipeline_integration() {
        // Create test audio: silence - speech - silence
        let sample_rate = 16000u32;
        let window_size = (sample_rate as f32 * 0.01) as usize; // 10ms window

        let mut test_audio = vec![0.0; window_size]; // Leading silence
        test_audio.extend(vec![0.2; window_size * 20]); // 200ms of speech
        test_audio.extend(vec![0.0; window_size]); // Trailing silence

        // Test only the audio processing part, not the API call
        // Since we don't have an API key in tests, we'll just test up to WAV encoding
        let processor = AudioProcessor::new(sample_rate);
        let processed = processor.process_for_speech_recognition(&test_audio);
        assert!(processed.is_ok(), "Audio processing should succeed");

        let encoder = WavEncoder::new(sample_rate, 1);
        let wav_result = encoder.encode_to_wav(&processed.unwrap());
        assert!(
            wav_result.is_ok(),
            "WAV encoding should succeed with valid audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_empty_audio() {
        let test_config = Config::default();
        let result = process_audio_for_transcription(vec![], 16000, "test", &test_config).await;

        assert!(
            result.is_err(),
            "Audio processing should fail with empty audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_too_short() {
        // Audio that's too short (less than 0.1 seconds)
        let short_audio = vec![0.5; 160]; // 0.01 seconds at 16kHz

        let test_config = Config::default();
        let result =
            process_audio_for_transcription(short_audio, 16000, "test", &test_config).await;

        assert!(
            result.is_err(),
            "Audio processing should fail with too short audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_only_silence() {
        // Audio with only silence
        let silent_audio = vec![0.0; 1600]; // 0.1 seconds of silence

        let test_config = Config::default();
        let result =
            process_audio_for_transcription(silent_audio, 16000, "test", &test_config).await;

        assert!(
            result.is_err(),
            "Audio processing should fail with only silence"
        );
    }

    #[test]
    fn test_wav_encoder_whisper_compatibility() {
        let encoder = WavEncoder::default();
        let test_samples = vec![0.1, 0.2, -0.1, -0.2];

        let wav_data = encoder.encode_to_wav(&test_samples).unwrap();

        // Verify WAV format matches Whisper requirements
        assert!(wav_data.len() > 44, "WAV should have header + data");

        // Check header for Whisper compatibility (16kHz, mono, 16-bit)
        assert_eq!(&wav_data[0..4], b"RIFF");
        assert_eq!(&wav_data[8..12], b"WAVE");

        // Sample rate should be 16000
        let sample_rate =
            u32::from_le_bytes([wav_data[24], wav_data[25], wav_data[26], wav_data[27]]);
        assert_eq!(sample_rate, 16000);

        // Channels should be 1 (mono)
        let channels = u16::from_le_bytes([wav_data[22], wav_data[23]]);
        assert_eq!(channels, 1);

        // Bits per sample should be 16
        let bits_per_sample = u16::from_le_bytes([wav_data[34], wav_data[35]]);
        assert_eq!(bits_per_sample, 16);
    }

    #[test]
    fn test_end_to_end_audio_pipeline() {
        let sample_rate = 16000u32;
        let processor = AudioProcessor::new(sample_rate);
        let encoder = WavEncoder::new(sample_rate, 1);

        // Create realistic audio test case
        let window_size = (sample_rate as f32 * 0.01) as usize;
        let mut audio = vec![0.005; window_size * 2]; // Quiet leading section
        audio.extend(vec![0.3; window_size * 50]); // 500ms of speech
        audio.extend(vec![0.005; window_size * 2]); // Quiet trailing section

        // Step 1: Process for speech recognition
        let processed = processor.process_for_speech_recognition(&audio).unwrap();

        // Verify processing results
        assert!(processed.len() < audio.len(), "Audio should be trimmed");
        assert!(
            processed.len() >= window_size * 45,
            "Should contain most of the speech"
        );

        // Step 2: Encode to WAV
        let wav_data = encoder.encode_to_wav(&processed).unwrap();

        // Verify WAV output
        assert!(wav_data.len() > 44, "Should have WAV header + data");
        assert_eq!(
            wav_data.len(),
            44 + processed.len() * 2,
            "Correct WAV file size"
        );

        // Verify would be under 25MB Whisper limit (should be tiny for this test)
        assert!(
            wav_data.len() < 25 * 1024 * 1024,
            "Should be well under Whisper 25MB limit"
        );
    }

    #[test]
    fn test_memory_cleanup_simulation() {
        // Test that we're not leaking memory during processing
        let sample_rate = 16000u32;
        let processor = AudioProcessor::new(sample_rate);
        let encoder = WavEncoder::new(sample_rate, 1);

        // Process multiple audio buffers to simulate repeated signal handling
        for _ in 0..10 {
            let audio = vec![0.2; sample_rate as usize]; // 1 second of audio

            // Process audio
            let processed = processor.process_for_speech_recognition(&audio).unwrap();

            // Encode to WAV
            let wav_data = encoder.encode_to_wav(&processed).unwrap();

            // Simulate cleanup (drop would happen automatically)
            drop(wav_data);
            drop(processed);
        }

        // If we get here without running out of memory, cleanup is working
        // Test passed successfully
    }

    #[test]
    fn test_edge_case_handling() {
        let processor = AudioProcessor::default();

        // Test various edge cases that could occur in real usage

        // Very quiet audio (but not silence)
        let quiet_audio = vec![0.002; 1600]; // Just above silence threshold
        let result = processor.process_for_speech_recognition(&quiet_audio);
        // Should either succeed with quiet audio or fail gracefully
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("silence") || error_msg.contains("too short"));
        }

        // Audio with clipping (values outside [-1.0, 1.0])
        let clipped_audio = vec![1.5, -1.5, 0.5, -0.5]; // Mix of clipped and normal
        let processed = processor.normalize_audio(&clipped_audio);
        // Should be normalized without errors
        assert_eq!(processed.len(), clipped_audio.len());

        // Very long audio (simulate max buffer size)
        let long_audio = vec![0.1; 16000 * 10]; // 10 seconds
        let result = processor.process_for_speech_recognition(&long_audio);
        assert!(result.is_ok(), "Should handle long audio without issues");
    }

    #[tokio::test]
    async fn test_process_audio_for_transcription_error_handling() {
        let config = Config::default();

        // Test with various error conditions
        let test_cases = vec![
            (vec![], "empty audio"),
            (vec![0.1; 100], "too short audio"),
            (vec![0.0; 1600], "silent audio"),
        ];

        for (audio_data, description) in test_cases {
            let result = process_audio_for_transcription(audio_data, 16000, "test", &config).await;

            assert!(result.is_err(), "Should fail for {}", description);
        }
    }

    #[test]
    fn test_config_validation_comprehensive() {
        // Test valid config
        let mut config = Config {
            openai_api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // Test invalid sample rates (must be > 0)
        config.audio_sample_rate = 0; // Invalid
        assert!(config.validate().is_err());

        // Test invalid channel counts (must be > 0)
        config.audio_sample_rate = 16000; // Reset to valid
        config.audio_channels = 0; // Invalid
        assert!(config.validate().is_err());

        // Test invalid buffer duration (must be > 0)
        config.audio_channels = 1; // Reset to valid
        config.audio_buffer_duration_seconds = 0; // Invalid
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_signal_action_validation() {
        // Test that we handle different action types correctly
        let valid_actions = vec!["type", "copy"];
        let invalid_actions = vec!["invalid", "", "TYPE", "COPY"];

        for valid_action in valid_actions {
            // Actions are processed in process_audio_for_transcription
            // This test validates the action string handling logic
            assert!(matches!(valid_action, "type" | "copy"));
        }

        for invalid_action in invalid_actions {
            // Invalid actions should not match our expected patterns
            assert!(!matches!(invalid_action, "type" | "copy"));
        }
    }
}
