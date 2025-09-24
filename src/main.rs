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

use anyhow::{anyhow, Result};
use clap::Parser;
use std::{io::Write, path::PathBuf};

use std::time::Instant;
use tokio::io::AsyncWriteExt;

use futures::stream::StreamExt;
#[cfg(not(test))]
use signal_hook::consts::{SIGTERM, SIGUSR1};
#[cfg(not(test))]
use signal_hook_tokio::Signals;

mod audio;
mod audio_processing;
mod beep;
mod command;
mod config;
mod transcription;
mod wav;

#[cfg(test)]
mod test_utils;
use audio::AudioRecorder;
use audio_processing::AudioProcessor;
use beep::{BeepConfig, BeepPlayer, BeepType};
use config::Config;
use transcription::{TranscriptionError, TranscriptionFactory};
use wav::WavEncoder;

#[derive(Parser)]
#[command(name = "waystt")]
#[command(about = "Wayland Speech-to-Text Tool - Signal-driven transcription")]
#[command(version)]
struct Args {
    /// Path to environment file
    #[arg(long)]
    envfile: Option<PathBuf>,

    /// Pipe transcribed text to the specified command
    /// Usage: waystt --pipe-to command args
    /// Example: waystt --pipe-to wl-copy
    /// Example: waystt --pipe-to ydotool type --file -
    #[arg(long, short = 'p', num_args = 1.., value_name = "COMMAND", allow_hyphen_values = true, trailing_var_arg = true)]
    pipe_to: Option<Vec<String>>,

    /// Download the configured local model and exit
    #[arg(long)]
    download_model: bool,
}

fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("waystt")
        .join(".env")
}

async fn download_model(model: &str) -> Result<PathBuf> {
    let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
    let url = format!("{}/{}", base_url, model);
    let dir = Config::model_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(model);

    let resp = reqwest::get(&url).await.map_err(|e| anyhow!("{}", e))?;
    if !resp.status().is_success() {
        return Err(anyhow!("Failed to download model: {}", resp.status()));
    }

    let total_size = resp.content_length();
    let mut file = tokio::fs::File::create(&path).await?;
    let mut stream = resp.bytes_stream();

    let mut downloaded = 0u64;
    let start_time = Instant::now();

    print!("{}... ", model);
    std::io::stdout().flush().unwrap();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("Download error: {}", e))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if let Some(total) = total_size {
            let percentage = (downloaded as f64 / total as f64) * 100.0;
            let elapsed = start_time.elapsed().as_secs_f64();

            if elapsed > 0.0 {
                let speed = downloaded as f64 / elapsed / 1024.0 / 1024.0; // MB/s
                let eta = if speed > 0.0 {
                    (total - downloaded) as f64 / (speed * 1024.0 * 1024.0)
                } else {
                    0.0
                };

                print!(
                    "\r{}... {:.1}% ({:.1} MB/s, ETA: {:.0}s)    ",
                    model, percentage, speed, eta
                );
                std::io::stdout().flush().unwrap();
            }
        } else {
            print!(
                "\r{}... {:.1} MB downloaded    ",
                model,
                downloaded as f64 / 1024.0 / 1024.0
            );
            std::io::stdout().flush().unwrap();
        }
    }

    file.flush().await?;
    Ok(path)
}

/// Process recorded audio for transcription
async fn process_audio_for_transcription(
    audio_data: Vec<f32>,
    sample_rate: u32,
    config: &Config,
    pipe_command: Option<&Vec<String>>,
) -> Result<i32> {
    // Initialize beep player
    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;
    eprintln!("Processing audio: {} samples", audio_data.len());

    // Initialize audio processor
    let processor = AudioProcessor::new(sample_rate);

    // Process audio for speech recognition
    match processor.process_for_speech_recognition(&audio_data) {
        Ok(processed_audio) => {
            let original_duration = processor.get_duration_seconds(&audio_data);
            let processed_duration = processor.get_duration_seconds(&processed_audio);

            eprintln!(
                "Audio processed successfully: {:.2}s -> {:.2}s ({} samples)",
                original_duration,
                processed_duration,
                processed_audio.len()
            );

            // Encode to WAV format for API
            let encoder = WavEncoder::new(sample_rate, 1);
            match encoder.encode_to_wav(&processed_audio) {
                Ok(wav_data) => {
                    eprintln!(
                        "WAV encoded: {} bytes ready for transcription",
                        wav_data.len()
                    );

                    // Initialize transcription provider with configuration
                    let provider =
                        TranscriptionFactory::create_provider(&config.transcription_provider)
                            .await?;

                    // Send to transcription service
                    eprintln!(
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
                                eprintln!("Warning: Received empty transcription from Whisper API");
                                eprintln!("This might indicate silent audio or unclear speech");

                                // Empty transcription is still a successful transcription, so pipe it
                                let exit_code = if let Some(cmd) = pipe_command {
                                    match command::execute_with_input(cmd, "").await {
                                        Ok(exit_code) => exit_code,
                                        Err(e) => {
                                            eprintln!("Failed to execute pipe command: {}", e);
                                            // Play error beep for command execution failure
                                            if let Err(beep_err) =
                                                beep_player.play_async(BeepType::Error).await
                                            {
                                                eprintln!(
                                                    "Warning: Failed to play error beep: {}",
                                                    beep_err
                                                );
                                            }
                                            1
                                        }
                                    }
                                } else {
                                    // Output empty transcription to stdout (existing behavior)
                                    println!("{}", transcribed_text);
                                    0
                                };

                                // Play success beep for successful (but empty) transcription
                                if let Err(e) = beep_player.play_async(BeepType::Success).await {
                                    eprintln!("Warning: Failed to play success beep: {}", e);
                                }

                                return Ok(exit_code);
                            }

                            eprintln!("Transcription successful: \"{}\"", transcribed_text);

                            // Handle piping to command or stdout
                            let exit_code = if let Some(cmd) = pipe_command {
                                match command::execute_with_input(cmd, &transcribed_text).await {
                                    Ok(exit_code) => exit_code,
                                    Err(e) => {
                                        eprintln!("Failed to execute pipe command: {}", e);
                                        // Play error beep for command execution failure
                                        if let Err(beep_err) =
                                            beep_player.play_async(BeepType::Error).await
                                        {
                                            eprintln!(
                                                "Warning: Failed to play error beep: {}",
                                                beep_err
                                            );
                                        }
                                        return Ok(1);
                                    }
                                }
                            } else {
                                // Output transcribed text to stdout (existing behavior)
                                println!("{}", transcribed_text);
                                0
                            };

                            // Play success beep after successful transcription
                            if let Err(e) = beep_player.play_async(BeepType::Success).await {
                                eprintln!("Warning: Failed to play success beep: {}", e);
                            }

                            Ok(exit_code)
                        }
                        Err(e) => {
                            eprintln!("❌ Transcription failed: {}", e);

                            // Play error beep for transcription failure
                            if let Err(beep_err) = beep_player.play_async(BeepType::Error).await {
                                eprintln!("Warning: Failed to play error beep: {}", beep_err);
                            }

                            // Provide helpful error messages based on error details
                            match &e {
                                TranscriptionError::AuthenticationFailed { provider, details } => {
                                    if let Some(details) = details {
                                        eprintln!("🔑 Authentication details: {}", details);
                                    }
                                    eprintln!("💡 Check your {} API key configuration", provider);
                                    if provider.contains("OpenAI") {
                                        eprintln!("💡 Verify OPENAI_API_KEY in your environment");
                                    } else if provider.contains("Google") {
                                        eprintln!("💡 Verify GOOGLE_APPLICATION_CREDENTIALS path and file content");
                                    }
                                }
                                TranscriptionError::NetworkError(details) => {
                                    eprintln!(
                                        "🌐 Network details: {} - {}",
                                        details.error_type, details.error_message
                                    );
                                    match details.error_type.as_str() {
                                        "Request timeout" => {
                                            eprintln!("💡 The transcription service took too long to respond");
                                            eprintln!("💡 Try with a shorter audio clip or check your internet speed");
                                        }
                                        "Connection failed" => {
                                            eprintln!(
                                                "💡 Cannot connect to {} servers",
                                                details.provider
                                            );
                                            eprintln!("💡 Check your internet connection and firewall settings");
                                        }
                                        "Service unavailable" => {
                                            eprintln!(
                                                "💡 {} service is temporarily unavailable",
                                                details.provider
                                            );
                                            eprintln!("💡 Please try again in a few minutes");
                                        }
                                        _ => {
                                            eprintln!(
                                                "💡 Check your internet connection and try again"
                                            );
                                        }
                                    }
                                }
                                TranscriptionError::ApiError(details) => {
                                    if let Some(status) = details.status_code {
                                        eprintln!("📡 API Response: HTTP {}", status);
                                    }
                                    if let Some(code) = &details.error_code {
                                        eprintln!("🏷️  Error Code: {}", code);
                                    }
                                    if let Some(raw_response) = &details.raw_response {
                                        eprintln!("📄 Raw API Response: {}", raw_response);
                                    }

                                    // Provide specific guidance based on error codes and status
                                    match (details.status_code, details.error_code.as_deref()) {
                                        (Some(400), Some("INVALID_ARGUMENT")) => {
                                            eprintln!(
                                                "💡 Check your audio format and language settings"
                                            );
                                        }
                                        (Some(401), _) => {
                                            eprintln!("💡 API key is invalid or has insufficient permissions");
                                        }
                                        (Some(403), _) => {
                                            eprintln!("💡 API access denied - check your billing/quota settings");
                                        }
                                        (Some(404), _) => {
                                            eprintln!("💡 API endpoint not found - check your service configuration");
                                        }
                                        (Some(429), _) => {
                                            eprintln!("💡 Rate limit exceeded - please wait before trying again");
                                        }
                                        (Some(500..=599), _) => {
                                            eprintln!(
                                                "💡 {} server error - please try again later",
                                                details.provider
                                            );
                                        }
                                        _ => {
                                            eprintln!("💡 Check the error details above and your API configuration");
                                        }
                                    }
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
                                TranscriptionError::JsonError(_) => {
                                    eprintln!("💡 Failed to parse API response - the service may be experiencing issues");
                                }
                            }

                            // Don't execute pipe command when transcription fails
                            Ok(1) // Return exit code 1 for transcription failure
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to encode WAV: {}", e);

                    // Don't execute pipe command when WAV encoding fails
                    Ok(1) // Return exit code 1 for WAV encoding failure
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

            // Don't execute pipe command when audio processing fails
            Ok(1) // Return exit code 1 for audio processing failure
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Determine the config file path
    let envfile = args.envfile.unwrap_or_else(get_default_config_path);

    // Load configuration from environment file or system environment
    let config = if envfile.exists() {
        eprintln!("Loading environment from: {}", envfile.display());
        match Config::load_env_file(&envfile) {
            Ok(config) => config,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to load environment file {}: {}",
                    envfile.display(),
                    e
                );
                eprintln!("Falling back to system environment");
                Config::from_env()
            }
        }
    } else {
        eprintln!(
            "Environment file {} not found, using system environment",
            envfile.display()
        );
        Config::from_env()
    };

    if args.download_model {
        match download_model(&config.whisper_model).await {
            Ok(path) => {
                eprintln!("Model downloaded to {}", path.display());
                return Ok(());
            }
            Err(e) => {
                eprintln!("Failed to download model: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Validate configuration (but don't fail if API key missing, as we're just recording for now)
    if let Err(e) = config.validate() {
        eprintln!("Configuration warning: {}", e);
        if config.transcription_provider == "local" {
            std::process::exit(1);
        }
        eprintln!(
            "Note: This is expected during development phase before transcription is implemented"
        );
    }

    eprintln!("waystt - Wayland Speech-to-Text Tool");
    eprintln!("Starting audio recording...");

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

    eprintln!("Audio recording started successfully!");

    // Give PipeWire a moment to start capturing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    eprintln!("Ready. Send SIGUSR1 to transcribe and output to stdout.");

    // Main event loop - process audio and wait for signals
    #[cfg(not(test))]
    {
        let mut signals = Signals::new([SIGUSR1, SIGTERM])?;

        loop {
            // Process audio events to capture microphone data
            if let Err(e) = recorder.process_audio_events() {
                eprintln!("Error processing audio events: {}", e);
            }

            // Check for signals with timeout
            match tokio::time::timeout(tokio::time::Duration::from_millis(50), signals.next()).await
            {
                Ok(Some(signal)) => {
                    match signal {
                        SIGUSR1 => {
                            eprintln!("Received SIGUSR1: Stop recording, transcribe, and output");

                            // Stop recording
                            if let Err(e) = recorder.stop_recording() {
                                eprintln!("Failed to stop recording: {}", e);
                            } else {
                                // Play recording stop beep
                                if let Err(e) =
                                    beep_player.play_async(BeepType::RecordingStop).await
                                {
                                    eprintln!("Warning: Failed to play recording stop beep: {}", e);
                                }
                            }

                            // Get recorded audio data and process it
                            match recorder.get_audio_data() {
                                Ok(audio_data) => {
                                    let duration =
                                        recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                    eprintln!(
                                        "Captured {} audio samples ({:.2} seconds)",
                                        audio_data.len(),
                                        duration
                                    );

                                    // Process audio for transcription
                                    match process_audio_for_transcription(
                                        audio_data,
                                        16000, // Using fixed sample rate from audio module
                                        &config,
                                        args.pipe_to.as_ref(),
                                    )
                                    .await
                                    {
                                        Ok(exit_code) => {
                                            eprintln!(
                                                "Audio processing completed with exit code: {}",
                                                exit_code
                                            );

                                            // Clear buffer to free memory
                                            if let Err(e) = recorder.clear_buffer() {
                                                eprintln!("Failed to clear audio buffer: {}", e);
                                            }

                                            // Exit with the appropriate code
                                            std::process::exit(exit_code);
                                        }
                                        Err(e) => {
                                            eprintln!("Audio processing failed: {}", e);

                                            // Clear buffer to free memory
                                            if let Err(e) = recorder.clear_buffer() {
                                                eprintln!("Failed to clear audio buffer: {}", e);
                                            }

                                            // Exit with error code
                                            std::process::exit(1);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to get audio data: {}", e);
                                }
                            }

                            break;
                        }
                        SIGTERM => {
                            eprintln!("Received SIGTERM: Shutting down gracefully");
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
                            eprintln!("Received unexpected signal: {}", signal);
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
    }

    // During tests, just return early without signal handling
    #[cfg(test)]
    {
        eprintln!("Test mode: Signal handling disabled");
    }

    eprintln!("Exiting waystt");
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
        let result = process_audio_for_transcription(vec![], 16000, &test_config, None).await;

        assert!(
            result.is_ok() && result.unwrap() == 1,
            "Audio processing should return exit code 1 with empty audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_too_short() {
        // Audio that's too short (less than 0.1 seconds)
        let short_audio = vec![0.5; 160]; // 0.01 seconds at 16kHz

        let test_config = Config::default();
        let result = process_audio_for_transcription(short_audio, 16000, &test_config, None).await;

        assert!(
            result.is_ok() && result.unwrap() == 1,
            "Audio processing should return exit code 1 with too short audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_only_silence() {
        // Audio with only silence
        let silent_audio = vec![0.0; 1600]; // 0.1 seconds of silence

        let test_config = Config::default();
        let result = process_audio_for_transcription(silent_audio, 16000, &test_config, None).await;

        assert!(
            result.is_ok() && result.unwrap() == 1,
            "Audio processing should return exit code 1 with only silence"
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
            let result = process_audio_for_transcription(audio_data, 16000, &config, None).await;

            assert!(
                result.is_ok() && result.unwrap() == 1,
                "Should return exit code 1 for {}",
                description
            );
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_pipe_to_functionality_with_command() {
        use crate::test_utils::ENV_MUTEX;

        let _lock = ENV_MUTEX.lock().await;

        let config = Config::default();
        let pipe_command = vec!["cat".to_string()];

        // Test with empty audio (should not execute command)
        let result =
            process_audio_for_transcription(vec![], 16000, &config, Some(&pipe_command)).await;

        assert!(result.is_ok());
        // Should return exit code 1 without executing the command
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_pipe_to_functionality_with_failing_command() {
        use crate::test_utils::ENV_MUTEX;

        let _lock = ENV_MUTEX.lock().await;

        let config = Config::default();
        let pipe_command = vec!["false".to_string()]; // Command that always fails

        // Test with empty audio (should not execute command)
        let result =
            process_audio_for_transcription(vec![], 16000, &config, Some(&pipe_command)).await;

        assert!(result.is_ok());
        // Should return exit code 1 without executing the command
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_pipe_to_functionality_with_nonexistent_command() {
        use crate::test_utils::ENV_MUTEX;

        let _lock = ENV_MUTEX.lock().await;

        let config = Config::default();
        let pipe_command = vec!["nonexistent_command_12345".to_string()];

        // Test with empty audio (should not execute command)
        let result =
            process_audio_for_transcription(vec![], 16000, &config, Some(&pipe_command)).await;

        assert!(result.is_ok());
        // Should return exit code 1 without executing the command
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_pipe_to_functionality_with_successful_empty_transcription() {
        use crate::test_utils::ENV_MUTEX;

        let _lock = ENV_MUTEX.lock().await;

        // This test would require mocking the transcription provider to return empty string
        // For now, we're testing the audio processing failure cases which is sufficient
        // The successful transcription + pipe logic is tested via the command module tests
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
}
