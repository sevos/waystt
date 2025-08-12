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
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[cfg(not(test))]
use futures::stream::StreamExt;
#[cfg(not(test))]
use signal_hook::consts::{SIGTERM, SIGUSR1, SIGUSR2};
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
use beep::{BeepConfig, BeepPlayer, BeepType};
use config::Config;
use transcription::streaming::StreamingTranscriber;
use wav::WavEncoder;

#[derive(Parser)]
#[command(name = "waystt")]
#[command(about = "Wayland Speech-to-Text Tool - Real-time streaming transcription")]
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
}

fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("waystt")
        .join(".env")
}

/// State for managing recording and streaming transcription
struct StreamingState {
    is_recording: Arc<AtomicBool>,
    audio_sender: Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>,
    transcription_task: Arc<Mutex<Option<tokio::task::JoinHandle<Result<String, transcription::TranscriptionError>>>>>,
    wav_header_sent: Arc<AtomicBool>,
}

impl StreamingState {
    fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_sender: Arc::new(Mutex::new(None)),
            transcription_task: Arc::new(Mutex::new(None)),
            wav_header_sent: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Determine the config file path
    let envfile = args.envfile.unwrap_or_else(get_default_config_path);

    // Load configuration
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

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("Configuration error: {}", e);
        return Err(e);
    }

    eprintln!("waystt - Real-time Streaming Transcription");
    eprintln!("Commands:");
    eprintln!("  SIGUSR1: Start recording and streaming to OpenAI");
    eprintln!("  SIGUSR2: Stop recording and get transcription result");
    eprintln!("  SIGTERM: Shutdown");

    // Initialize beep player
    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;

    // Initialize audio recorder
    let mut recorder = AudioRecorder::new()?;

    // Initialize state
    let state = StreamingState::new();

    // Create streaming transcriber
    let api_key = config.openai_api_key.clone().unwrap();
    let base_url = config.openai_base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let model = config.whisper_model.clone();
    let transcriber = StreamingTranscriber::new(api_key, base_url, model);

    eprintln!("Ready. Send SIGUSR1 to start recording and streaming.");

    // Buffer for accumulating audio samples before encoding
    let audio_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let sample_rate = 16000u32;

    // Main event loop
    #[cfg(not(test))]
    {
        let mut signals = Signals::new([SIGUSR1, SIGUSR2, SIGTERM])?;

        loop {
            // Process audio events if recording
            if state.is_recording.load(Ordering::Relaxed) {
                if let Err(e) = recorder.process_audio_events() {
                    eprintln!("Error processing audio events: {}", e);
                }

                // Get current audio data and stream it
                if let Ok(current_audio) = recorder.get_audio_data() {
                    let mut buffer = audio_buffer.lock().await;
                    
                    // Check if we have new samples
                    if current_audio.len() > buffer.len() {
                        let new_samples = &current_audio[buffer.len()..];
                        buffer.extend_from_slice(new_samples);
                        
                        // Stream new audio chunks periodically (e.g., every 0.5 seconds worth of audio)
                        let chunk_size = (sample_rate as f32 * 0.5) as usize;
                        if new_samples.len() >= chunk_size {
                            // Encode the new samples as WAV
                            let encoder = WavEncoder::new(sample_rate, 1);
                            
                            // If this is the first chunk, include WAV header
                            let wav_data = if !state.wav_header_sent.load(Ordering::Relaxed) {
                                match encoder.encode_to_wav(new_samples) {
                                    Ok(data) => {
                                        state.wav_header_sent.store(true, Ordering::Relaxed);
                                        data
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to encode WAV: {}", e);
                                        continue;
                                    }
                                }
                            } else {
                                // For subsequent chunks, just send raw PCM data as WAV expects it
                                let mut wav_chunk = Vec::new();
                                for sample in new_samples {
                                    let sample_i16 = (sample * 32767.0) as i16;
                                    wav_chunk.extend_from_slice(&sample_i16.to_le_bytes());
                                }
                                wav_chunk
                            };
                            
                            // Send to streaming transcriber
                            if let Some(sender) = state.audio_sender.lock().await.as_ref() {
                                if sender.send(wav_data).await.is_err() {
                                    eprintln!("Failed to send audio chunk to transcriber");
                                }
                            }
                        }
                    }
                }
            }

            // Check for signals
            match tokio::time::timeout(tokio::time::Duration::from_millis(50), signals.next()).await {
                Ok(Some(signal)) => {
                    match signal {
                        SIGUSR1 => {
                            if !state.is_recording.load(Ordering::Relaxed) {
                                eprintln!("Received SIGUSR1: Starting recording and streaming");
                                
                                // Clear buffers
                                audio_buffer.lock().await.clear();
                                if let Err(e) = recorder.clear_buffer() {
                                    eprintln!("Failed to clear recorder buffer: {}", e);
                                }
                                state.wav_header_sent.store(false, Ordering::Relaxed);
                                
                                // Start streaming transcription
                                let language = if config.whisper_language == "auto" {
                                    None
                                } else {
                                    Some(config.whisper_language.clone())
                                };
                                
                                match transcriber.start_streaming(language).await {
                                    Ok((sender, task)) => {
                                        *state.audio_sender.lock().await = Some(sender);
                                        *state.transcription_task.lock().await = Some(task);
                                        
                                        // Start recording
                                        if let Err(e) = recorder.start_recording() {
                                            eprintln!("Failed to start recording: {}", e);
                                            let _ = beep_player.play_async(BeepType::Error).await;
                                        } else {
                                            state.is_recording.store(true, Ordering::Relaxed);
                                            
                                            // Play start beep
                                            let _ = beep_player.play_async(BeepType::RecordingStart).await;
                                            
                                            eprintln!("Recording and streaming started - speak now...");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to start streaming: {}", e);
                                        let _ = beep_player.play_async(BeepType::Error).await;
                                    }
                                }
                            } else {
                                eprintln!("Already recording, ignoring SIGUSR1");
                            }
                        }
                        SIGUSR2 => {
                            if state.is_recording.load(Ordering::Relaxed) {
                                eprintln!("Received SIGUSR2: Stopping recording and getting result");
                                
                                // Stop recording
                                state.is_recording.store(false, Ordering::Relaxed);
                                if let Err(e) = recorder.stop_recording() {
                                    eprintln!("Failed to stop recording: {}", e);
                                }
                                
                                // Play stop beep
                                let _ = beep_player.play_async(BeepType::RecordingStop).await;
                                
                                // Send any remaining audio
                                if let Ok(final_audio) = recorder.get_audio_data() {
                                    let buffer = audio_buffer.lock().await;
                                    if final_audio.len() > buffer.len() {
                                        let remaining = &final_audio[buffer.len()..];
                                        if !remaining.is_empty() {
                                            let encoder = WavEncoder::new(sample_rate, 1);
                                            let wav_data = if !state.wav_header_sent.load(Ordering::Relaxed) {
                                                encoder.encode_to_wav(remaining).unwrap_or_default()
                                            } else {
                                                let mut wav_chunk = Vec::new();
                                                for sample in remaining {
                                                    let sample_i16 = (sample * 32767.0) as i16;
                                                    wav_chunk.extend_from_slice(&sample_i16.to_le_bytes());
                                                }
                                                wav_chunk
                                            };
                                            
                                            if let Some(sender) = state.audio_sender.lock().await.as_ref() {
                                                let _ = sender.send(wav_data).await;
                                            }
                                        }
                                    }
                                }
                                
                                // Close the audio sender to signal end of stream
                                *state.audio_sender.lock().await = None;
                                
                                // Wait for transcription result
                                if let Some(task) = state.transcription_task.lock().await.take() {
                                    eprintln!("Waiting for transcription result...");
                                    match task.await {
                                        Ok(Ok(text)) => {
                                            if !text.trim().is_empty() {
                                                eprintln!("Transcription: \"{}\"", text);
                                                
                                                // Execute pipe command if provided
                                                if let Some(cmd) = &args.pipe_to {
                                                    match command::execute_with_input(cmd, &text).await {
                                                        Ok(exit_code) => {
                                                            eprintln!("Command executed with exit code: {}", exit_code);
                                                        }
                                                        Err(e) => {
                                                            eprintln!("Failed to execute pipe command: {}", e);
                                                            let _ = beep_player.play_async(BeepType::Error).await;
                                                        }
                                                    }
                                                } else {
                                                    // Output to stdout
                                                    println!("{}", text);
                                                }
                                                
                                                let _ = beep_player.play_async(BeepType::Success).await;
                                            } else {
                                                eprintln!("Received empty transcription");
                                                let _ = beep_player.play_async(BeepType::Success).await;
                                            }
                                        }
                                        Ok(Err(e)) => {
                                            eprintln!("Transcription failed: {}", e);
                                            let _ = beep_player.play_async(BeepType::Error).await;
                                        }
                                        Err(e) => {
                                            eprintln!("Transcription task failed: {}", e);
                                            let _ = beep_player.play_async(BeepType::Error).await;
                                        }
                                    }
                                }
                                
                                // Clear buffer
                                if let Err(e) = recorder.clear_buffer() {
                                    eprintln!("Failed to clear buffer: {}", e);
                                }
                            } else {
                                eprintln!("Not recording, ignoring SIGUSR2");
                            }
                        }
                        SIGTERM => {
                            eprintln!("Received SIGTERM: Shutting down gracefully");
                            
                            // Stop recording if active
                            if state.is_recording.load(Ordering::Relaxed) {
                                if let Err(e) = recorder.stop_recording() {
                                    eprintln!("Failed to stop recording: {}", e);
                                }
                            }
                            
                            // Cancel any pending transcription
                            if let Some(task) = state.transcription_task.lock().await.take() {
                                task.abort();
                            }
                            
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
                    // Timeout occurred, continue
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