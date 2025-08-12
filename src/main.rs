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
use std::sync::atomic::AtomicBool;
#[cfg(not(test))]
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[cfg(not(test))]
use futures::stream::StreamExt;
#[cfg(not(test))]
use signal_hook::consts::{SIGTERM, SIGUSR1, SIGUSR2};
#[cfg(not(test))]
use signal_hook_tokio::Signals;

mod audio;

mod beep;
mod command;
mod config;
mod transcription;

#[cfg(test)]
mod test_utils;
#[cfg(not(test))]
use audio::AudioRecorder;
#[cfg(not(test))]
use beep::{BeepConfig, BeepPlayer, BeepType};
use config::Config;
#[cfg(not(test))]
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
#[allow(dead_code)]
struct RealtimeState {
    is_recording: Arc<AtomicBool>,
    audio_sender: Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>,
    ws_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl RealtimeState {
    fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_sender: Arc::new(Mutex::new(None)),
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
    eprintln!("  SIGUSR1: Start streaming audio to OpenAI");
    eprintln!("  SIGUSR2: Stop streaming (recording continues)");
    eprintln!("  SIGTERM: Shutdown");

    eprintln!("Starting continuous recording...");

    // Main event loop
    #[cfg(not(test))]
    {
        // Initialize beep player
        let beep_config = BeepConfig {
            enabled: config.enable_audio_feedback,
            volume: config.beep_volume,
        };
        let beep_player = BeepPlayer::new(beep_config)?;

        // Initialize audio recorder
        let mut recorder = AudioRecorder::new()?;

        // Start recording immediately (continuous recording)
        recorder.start_recording()?;
        eprintln!("Continuous recording started. Microphone is active.");

        // Initialize state
        let state = RealtimeState::new();

        // Create real-time transcriber with configured model
        let api_key = config.openai_api_key.clone().unwrap();
        let transcriber = RealtimeTranscriber::with_model(api_key, config.realtime_model.clone());

        // Track the last processed sample index
        let last_processed_index = Arc::new(Mutex::new(0usize));
        let mut signals = Signals::new([SIGUSR1, SIGUSR2, SIGTERM])?;

        loop {
            // Always process audio events (continuous recording)
            if let Err(e) = recorder.process_audio_events() {
                eprintln!("Error processing audio events: {}", e);
            }

            // Get current audio data and stream it if we're in streaming mode
            if state.is_recording.load(Ordering::Relaxed) {
                if let Ok(current_audio) = recorder.get_audio_data() {
                    let mut last_idx = last_processed_index.lock().await;

                    // Check if we have new samples since last processed
                    if current_audio.len() > *last_idx {
                        let new_samples = &current_audio[*last_idx..];

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

                            // Update the last processed index
                            *last_idx = current_audio.len();
                        }
                    }
                }
            }

            // Check for signals
            match tokio::time::timeout(tokio::time::Duration::from_millis(50), signals.next()).await
            {
                Ok(Some(signal)) => {
                    match signal {
                        SIGUSR1 => {
                            if !state.is_recording.load(Ordering::Relaxed) {
                                eprintln!("Received SIGUSR1: Starting audio streaming to OpenAI");

                                // Set the current audio position as starting point
                                if let Ok(current_audio) = recorder.get_audio_data() {
                                    *last_processed_index.lock().await = current_audio.len();
                                }

                                // Play start beep (recording continues in background)
                                let _ = beep_player.play_async(BeepType::RecordingStart).await;

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

                                        // Enable streaming mode (recording is already active)
                                        state.is_recording.store(true, Ordering::Relaxed);

                                        // Start task to handle transcriptions
                                        let pipe_cmd = args.pipe_to.clone();
                                        tokio::spawn(async move {
                                            while let Some(transcript) = transcript_rx.recv().await
                                            {
                                                if !transcript.trim().is_empty() {
                                                    eprintln!(
                                                        "Real-time transcription: \"{}\"",
                                                        transcript
                                                    );

                                                    // Execute pipe command if provided
                                                    if let Some(cmd) = &pipe_cmd {
                                                        match command::execute_with_input(
                                                            cmd,
                                                            &transcript,
                                                        )
                                                        .await
                                                        {
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

                                        eprintln!("Audio streaming started - speak now...");
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to start WebSocket session: {}", e);
                                        let _ = beep_player.play_async(BeepType::Error).await;
                                    }
                                }
                            } else {
                                eprintln!("Already streaming, ignoring SIGUSR1");
                            }
                        }
                        SIGUSR2 => {
                            if state.is_recording.load(Ordering::Relaxed) {
                                eprintln!("Received SIGUSR2: Stopping audio streaming");

                                // Stop streaming mode (recording continues)
                                state.is_recording.store(false, Ordering::Relaxed);

                                // Get any final audio data for streaming
                                if let Ok(final_audio) = recorder.get_audio_data() {
                                    let last_idx = last_processed_index.lock().await;
                                    if final_audio.len() > *last_idx {
                                        let remaining_samples = &final_audio[*last_idx..];
                                        if !remaining_samples.is_empty() {
                                            // Convert final samples to PCM16
                                            let mut pcm16_data =
                                                Vec::with_capacity(remaining_samples.len() * 2);
                                            for sample in remaining_samples {
                                                let sample_i16 =
                                                    (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                                pcm16_data
                                                    .extend_from_slice(&sample_i16.to_le_bytes());
                                            }

                                            // Send final audio
                                            if let Some(sender) =
                                                state.audio_sender.lock().await.as_ref()
                                            {
                                                let _ = sender.send(pcm16_data).await;
                                            }
                                        }
                                    }
                                }

                                // Play stop beep (recording continues in background)
                                let _ = beep_player.play_async(BeepType::RecordingStop).await;

                                // Give time for audio to be sent
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                                // Close audio sender to signal end of stream
                                *state.audio_sender.lock().await = None;

                                // Wait a moment for final transcriptions
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                                // Cancel WebSocket task
                                if let Some(task) = state.ws_task.lock().await.take() {
                                    task.abort();
                                }

                                // Clear buffer
                                if let Err(e) = recorder.clear_buffer() {
                                    eprintln!("Failed to clear buffer: {}", e);
                                }

                                eprintln!(
                                    "Audio streaming stopped (recording continues in background)"
                                );
                            } else {
                                eprintln!("Not streaming, ignoring SIGUSR2");
                            }
                        }
                        SIGTERM => {
                            eprintln!("Received SIGTERM: Shutting down gracefully");

                            // Stop the actual recording (we were recording continuously)
                            if let Err(e) = recorder.stop_recording() {
                                eprintln!("Failed to stop recording: {}", e);
                            }

                            // Cancel WebSocket task if streaming
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
