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
#[derive(Clone)]
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

        // Create channel for audio samples
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();

        // Initialize audio recorder
        let mut recorder = match AudioRecorder::new() {
            Ok(recorder) => recorder,
            Err(e) => {
                eprintln!("Failed to initialize audio recorder: {}", e);
                let _ = beep_player.play_async(BeepType::Error).await;
                return Err(e);
            }
        };
        recorder.set_audio_sender(audio_tx);

        // Start recording immediately
        if let Err(e) = recorder.start_recording() {
            eprintln!("Failed to start recording: {}", e);
            let _ = beep_player.play_async(BeepType::Error).await;
            return Err(e);
        }

        // Play "line ready" sound
        let _ = beep_player.play_async(BeepType::LineReady).await;
        eprintln!("Continuous recording started. Microphone is active. System ready.");

        // Initialize state
        let state = RealtimeState::new();

        // Create real-time transcriber with configured model
        let api_key = config.openai_api_key.clone().unwrap();
        let transcriber = RealtimeTranscriber::with_model(api_key, config.realtime_model.clone());

        let mut signals = Signals::new([SIGUSR1, SIGUSR2, SIGTERM])?;
        loop {
            // Always process audio events (continuous recording)
            if let Err(e) = recorder.process_audio_events() {
                eprintln!("Error processing audio events: {}", e);
            }

            // Process audio from the queue if we're streaming
            if state.is_recording.load(Ordering::Relaxed) {
                // Try to receive audio samples without blocking
                while let Ok(samples) = audio_rx.try_recv() {
                    // Convert to PCM16 and send immediately
                    if !samples.is_empty() {
                        let mut pcm16_data = Vec::with_capacity(samples.len() * 2);
                        for sample in samples {
                            let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                            pcm16_data.extend_from_slice(&sample_i16.to_le_bytes());
                        }

                        // Send to real-time transcriber
                        if let Some(sender) = state.audio_sender.lock().await.as_ref() {
                            if sender.send(pcm16_data).await.is_err() {
                                eprintln!("Failed to send audio to transcriber");
                            }
                        }
                    }
                }
            } else {
                // If not streaming, drain the queue to prevent memory buildup
                while audio_rx.try_recv().is_ok() {
                    // Just discard the samples
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

                                // Drain any accumulated audio before starting
                                while audio_rx.try_recv().is_ok() {}

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

                                        // Clone handles for the spawned task
                                        let state_clone = state.clone();
                                        let beep_player_clone = beep_player.clone();

                                        // Start task to handle transcriptions and errors
                                        let pipe_cmd = args.pipe_to.clone();
                                        tokio::spawn(async move {
                                            while let Some(result) = transcript_rx.recv().await {
                                                match result {
                                                    Ok(transcript) => {
                                                        if !transcript.trim().is_empty() {
                                                            eprintln!("Real-time transcription: \"{}\"", transcript);
                                                            if let Some(cmd) = &pipe_cmd {
                                                                if let Err(e) = command::execute_with_input(cmd, &transcript).await {
                                                                    eprintln!("Failed to execute pipe command: {}", e);
                                                                }
                                                            } else {
                                                                println!("{}", transcript);
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        eprintln!("Transcription error: {}", e);
                                                        let _ = beep_player_clone.play_async(BeepType::Error).await;

                                                        // Terminate the session
                                                        state_clone.is_recording.store(false, Ordering::Relaxed);
                                                        if let Some(task) = state_clone.ws_task.lock().await.take() {
                                                            task.abort();
                                                        }
                                                        *state_clone.audio_sender.lock().await = None;
                                                        eprintln!("Transcription session terminated due to error.");
                                                        break; // Exit the loop
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

                                // Process any remaining audio in the queue
                                while let Ok(samples) = audio_rx.try_recv() {
                                    if !samples.is_empty() {
                                        let mut pcm16_data = Vec::with_capacity(samples.len() * 2);
                                        for sample in samples {
                                            let sample_i16 =
                                                (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                            pcm16_data.extend_from_slice(&sample_i16.to_le_bytes());
                                        }

                                        if let Some(sender) =
                                            state.audio_sender.lock().await.as_ref()
                                        {
                                            let _ = sender.send(pcm16_data).await;
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
