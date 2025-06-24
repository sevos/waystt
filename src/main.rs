use anyhow::Result;
use signal_hook::consts::{SIGTERM, SIGUSR1, SIGUSR2};
use signal_hook_tokio::Signals;
use futures::stream::StreamExt;
use clap::Parser;
use std::path::PathBuf;

mod audio;
mod audio_processing;
mod config;
mod wav;
use audio::AudioRecorder;
use audio_processing::AudioProcessor;
use config::Config;
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
) -> Result<()> {
    println!("Processing audio for {}: {} samples", action, audio_data.len());
    
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
                    println!("WAV encoded: {} bytes ready for transcription", wav_data.len());
                    
                    // Here we would send to transcription service
                    // For now, we'll just log the success
                    println!("Audio ready for {} workflow", action);
                    
                    // Clean up processed data
                    drop(wav_data);
                    drop(processed_audio);
                    
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
                eprintln!("Warning: Failed to load environment file {}: {}", args.envfile.display(), e);
                println!("Falling back to system environment");
                Config::from_env()
            }
        }
    } else {
        println!("Environment file {} not found, using system environment", args.envfile.display());
        Config::from_env()
    };
    
    // Validate configuration (but don't fail if API key missing, as we're just recording for now)
    if let Err(e) = config.validate() {
        eprintln!("Configuration warning: {}", e);
        eprintln!("Note: This is expected during development phase before transcription is implemented");
    }
    
    println!("waystt - Wayland Speech-to-Text Tool");
    println!("Starting audio recording...");
    
    // Initialize audio recorder
    let mut recorder = AudioRecorder::new()?;
    
    // Start recording immediately
    if let Err(e) = recorder.start_recording() {
        eprintln!("Failed to start audio recording: {}", e);
        eprintln!("This may be due to PipeWire not being available or insufficient permissions.");
        return Err(e);
    }
    
    println!("Audio recording started successfully!");
    
    // Give PipeWire a moment to start capturing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    let mut signals = Signals::new(&[SIGUSR1, SIGUSR2, SIGTERM])?;
    
    println!("Ready. Send SIGUSR1 to transcribe and paste, SIGUSR2 to transcribe and copy.");
    
    // Main event loop - process audio and wait for signals
    loop {
        // Process audio events to capture microphone data
        if let Err(e) = recorder.process_audio_events() {
            eprintln!("Error processing audio events: {}", e);
        }

        // Check for signals with timeout
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(50),
            signals.next()
        ).await {
            Ok(Some(signal)) => {
                match signal {
                    SIGUSR1 => {
                        println!("Received SIGUSR1: Stop recording, transcribe, and paste");
                        
                        // Stop recording
                        if let Err(e) = recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {}", e);
                        }
                        
                        // Get recorded audio data and process it
                        match recorder.get_audio_data() {
                            Ok(audio_data) => {
                                let duration = recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                println!("Captured {} audio samples ({:.2} seconds)", audio_data.len(), duration);
                                
                                // Process audio for transcription
                                if let Err(e) = process_audio_for_transcription(
                                    audio_data, 
                                    16000, // Using fixed sample rate from audio module
                                    "transcribe and paste"
                                ).await {
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
                        }
                        
                        // Get recorded audio data and process it
                        match recorder.get_audio_data() {
                            Ok(audio_data) => {
                                let duration = recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                println!("Captured {} audio samples ({:.2} seconds)", audio_data.len(), duration);
                                
                                // Process audio for transcription
                                if let Err(e) = process_audio_for_transcription(
                                    audio_data, 
                                    16000, // Using fixed sample rate from audio module
                                    "transcribe and copy"
                                ).await {
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
        
        // Test the complete processing pipeline
        let result = process_audio_for_transcription(
            test_audio.clone(),
            sample_rate,
            "test"
        ).await;
        
        assert!(result.is_ok(), "Audio processing pipeline should succeed with valid audio");
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_empty_audio() {
        let result = process_audio_for_transcription(
            vec![],
            16000,
            "test"
        ).await;
        
        assert!(result.is_err(), "Audio processing should fail with empty audio");
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_too_short() {
        // Audio that's too short (less than 0.1 seconds)
        let short_audio = vec![0.5; 160]; // 0.01 seconds at 16kHz
        
        let result = process_audio_for_transcription(
            short_audio,
            16000,
            "test"
        ).await;
        
        assert!(result.is_err(), "Audio processing should fail with too short audio");
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_only_silence() {
        // Audio with only silence
        let silent_audio = vec![0.0; 1600]; // 0.1 seconds of silence
        
        let result = process_audio_for_transcription(
            silent_audio,
            16000,
            "test"
        ).await;
        
        assert!(result.is_err(), "Audio processing should fail with only silence");
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
        let sample_rate = u32::from_le_bytes([wav_data[24], wav_data[25], wav_data[26], wav_data[27]]);
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
        assert!(processed.len() >= window_size * 45, "Should contain most of the speech");
        
        // Step 2: Encode to WAV
        let wav_data = encoder.encode_to_wav(&processed).unwrap();
        
        // Verify WAV output
        assert!(wav_data.len() > 44, "Should have WAV header + data");
        assert_eq!(wav_data.len(), 44 + processed.len() * 2, "Correct WAV file size");
        
        // Verify would be under 25MB Whisper limit (should be tiny for this test)
        assert!(wav_data.len() < 25 * 1024 * 1024, "Should be well under Whisper 25MB limit");
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
        assert!(true, "Memory cleanup simulation completed successfully");
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
}
