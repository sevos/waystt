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
use transcription::realtime::RealtimeTranscriber;

#[derive(Parser)]
#[command(name = "waystt")]
#[command(about = "Wayland Speech-to-Text Tool - Real-time WebSocket transcription")]
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

/// State for managing recording and real-time transcription
struct RealtimeState {
    is_recording: Arc<AtomicBool>,
    audio_sender: Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>,
    transcript_receiver: Arc<Mutex<Option<mpsc::Receiver<String>>>>,
    ws_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl RealtimeState {
    fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_sender: Arc::new(Mutex::new(None)),
            transcript_receiver: Arc::new(Mutex::new(None)),
            ws_task: Arc::new(Mutex::new(None)),
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

    eprintln!("waystt - Real-time WebSocket Transcription");
    eprintln!("Using OpenAI Realtime API for instant transcription");
    eprintln!("Commands:");
    eprintln!("  SIGUSR1: Start recording and real-time transcription");
    eprintln!("  SIGUSR2: Stop recording and finalize transcription");
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
    let state = RealtimeState::new();

    // Create real-time transcriber
    let api_key = config.openai_api_key.clone().unwrap();
    let transcriber = RealtimeTranscriber::new(api_key);

    // Buffer for accumulating audio samples
    let audio_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let sample_rate = 16000u32;

    eprintln!("Ready. Send SIGUSR1 to start real-time transcription.");

    // Task to handle transcription results
    let pipe_command = args.pipe_to.clone();
    let beep_player_clone = beep_player.clone();
    let transcript_handler = tokio::spawn(async move {
        // This will be started when we begin recording
    });

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
                        
                        // Convert to PCM16 and send immediately
                        if !new_samples.is_empty() {
                            let mut pcm16_data = Vec::with_capacity(new_samples.len() * 2);
                            for sample in new_samples {
                                let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                pcm16_data.extend_from_slice(&sample_i16.to_le_bytes());
                            }
                            
                            // Send to real-time transcriber
                            if let Some(sender) = state.audio_sender.lock().await.as_ref() {
                                if sender.send(pcm16_data).await.is_err() {
                                    eprintln!("Failed to send audio to transcriber");
                                }
                            }
                            
                            buffer.extend_from_slice(new_samples);
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
                                eprintln!("Received SIGUSR1: Starting real-time transcription");
                                
                                // Clear buffers
                                audio_buffer.lock().await.clear();
                                if let Err(e) = recorder.clear_buffer() {
                                    eprintln!("Failed to clear recorder buffer: {}", e);
                                }
                                
                                // Play start beep BEFORE starting recording
                                let _ = beep_player.play_async(BeepType::RecordingStart).await;
                                
                                // Wait for beep to finish
                                tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                                
                                // Start WebSocket session
                                let language = if config.whisper_language == "auto" {
                                    None
                                } else {
                                    Some(config.whisper_language.clone())
                                };
                                
                                match transcriber.start_session(language).await {
                                    Ok((audio_tx, mut transcript_rx, ws_task)) => {
                                        *state.audio_sender.lock().await = Some(audio_tx);
                                        *state.ws_task.lock().await = Some(ws_task);
                                        
                                        // Start recording
                                        if let Err(e) = recorder.start_recording() {
                                            eprintln!("Failed to start recording: {}", e);
                                            let _ = beep_player.play_async(BeepType::Error).await;
                                        } else {
                                            state.is_recording.store(true, Ordering::Relaxed);
                                            
                                            // Start task to handle transcriptions
                                            let pipe_cmd = args.pipe_to.clone();
                                            let beep = beep_player.clone();
                                            tokio::spawn(async move {
                                                while let Some(transcript) = transcript_rx.recv().await {
                                                    if !transcript.trim().is_empty() {
                                                        eprintln!("Real-time transcription: \"{}\"", transcript);
                                                        
                                                        // Execute pipe command if provided
                                                        if let Some(cmd) = &pipe_cmd {
                                                            match command::execute_with_input(cmd, &transcript).await {
                                                                Ok(exit_code) => {
                                                                    eprintln!("Command executed with exit code: {}", exit_code);
                                                                }
                                                                Err(e) => {
                                                                    eprintln!("Failed to execute pipe command: {}", e);
                                                                }
                                                            }
                                                        } else {
                                                            // Output to stdout
                                                            println!("{}", transcript);
                                                        }
                                                    }
                                                }
                                            });
                                            
                                            eprintln!("Real-time transcription started - speak now...");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to start WebSocket session: {}", e);
                                        let _ = beep_player.play_async(BeepType::Error).await;
                                    }
                                }
                            } else {
                                eprintln!("Already recording, ignoring SIGUSR1");
                            }
                        }
                        SIGUSR2 => {
                            if state.is_recording.load(Ordering::Relaxed) {
                                eprintln!("Received SIGUSR2: Stopping recording");
                                
                                // Stop recording
                                state.is_recording.store(false, Ordering::Relaxed);
                                if let Err(e) = recorder.stop_recording() {
                                    eprintln!("Failed to stop recording: {}", e);
                                }
                                
                                // Play stop beep
                                let _ = beep_player.play_async(BeepType::RecordingStop).await;
                                
                                // Close audio sender to signal end of stream
                                *state.audio_sender.lock().await = None;
                                
                                // Wait a moment for final transcriptions
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                
                                // Cancel WebSocket task
                                if let Some(task) = state.ws_task.lock().await.take() {
                                    task.abort();
                                }
                                
                                // Play success beep
                                let _ = beep_player.play_async(BeepType::Success).await;
                                
                                // Clear buffer
                                if let Err(e) = recorder.clear_buffer() {
                                    eprintln!("Failed to clear buffer: {}", e);
                                }
                                
                                eprintln!("Recording stopped");
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
                            
                            // Cancel WebSocket task
                            if let Some(task) = state.ws_task.lock().await.take() {
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