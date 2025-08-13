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

mod audio;
mod beep;
mod command;
mod config;
mod socket;
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
#[command(name = "hotline")]
#[command(about = "HotLine - A minimalist speech-to-text (STT) tool.")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Run the HotLine daemon
    Daemon {
        /// Path to environment file
        #[arg(long)]
        envfile: Option<PathBuf>,
    },
    /// Send a command to the daemon
    Sendcmd,
    /// Validate and display configuration
    Config,
    /// Start transcription with a profile
    StartTranscription {
        /// Profile name to use
        profile: String,
    },
    /// Stop current transcription
    StopTranscription,
}

fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("hotline")
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

    match args.command {
        Commands::Daemon { envfile } => run_daemon(envfile).await,
        Commands::Sendcmd => run_sendcmd().await,
        Commands::Config => run_config().await,
        Commands::StartTranscription { profile } => run_start_transcription(profile).await,
        Commands::StopTranscription => run_stop_transcription().await,
    }
}

async fn run_config() -> Result<()> {
    // Load configuration with proper precedence
    let config = Config::load_with_precedence()?;

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("Configuration validation error: {}", e);
        return Err(e);
    }

    // Print configuration in human-readable format
    println!("HotLine Configuration");
    println!("=====================");
    println!();
    println!("OpenAI Settings:");
    println!(
        "  API Key: {}",
        if config.openai_api_key.is_some() {
            "***SET***"
        } else {
            "NOT SET"
        }
    );
    println!(
        "  Base URL: {}",
        config.openai_base_url.as_deref().unwrap_or("default")
    );
    println!("  Realtime Model: {}", config.realtime_model);
    println!();
    println!("Audio Settings:");
    println!(
        "  Buffer Duration: {} seconds",
        config.audio_buffer_duration_seconds
    );
    println!("  Sample Rate: {} Hz", config.audio_sample_rate);
    println!("  Channels: {}", config.audio_channels);
    println!("  Audio Feedback: {}", config.enable_audio_feedback);
    println!("  Beep Volume: {}", config.beep_volume);
    println!();
    println!("Whisper Settings:");
    println!("  Model: {}", config.whisper_model);
    println!("  Language: {}", config.whisper_language);
    println!("  Timeout: {} seconds", config.whisper_timeout_seconds);
    println!("  Max Retries: {}", config.whisper_max_retries);
    println!();
    println!("Other Settings:");
    println!("  Log Level: {}", config.rust_log);
    println!();
    println!("Configuration loaded from:");
    let toml_path = Config::get_toml_config_path();
    if toml_path.exists() {
        println!("  - TOML file: {}", toml_path.display());
    }
    println!("  - Environment variables");
    println!("  - Default values");

    Ok(())
}

async fn run_start_transcription(profile_name: String) -> Result<()> {
    // Load configuration to get the profile
    let config = Config::load_with_precedence()?;

    // Get the profile
    let profile = config
        .get_profile(&profile_name)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found in configuration", profile_name))?;

    // Create StartTranscription command from profile
    let command = socket::Command::StartTranscription(socket::StartTranscriptionArgs {
        model: profile.model.clone(),
        language: profile.language.clone(),
        prompt: profile.prompt.clone(),
        vad_config: profile.vad_config.clone(),
        command: profile.command.clone(),
    });

    // Send command to daemon
    match socket::send_command(&command).await {
        Ok(socket::Response::Success { message }) => {
            eprintln!("Success: {}", message);
            Ok(())
        }
        Ok(socket::Response::Error { message }) => {
            eprintln!("Error: {}", message);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to send command: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run_stop_transcription() -> Result<()> {
    // Send StopTranscription command to daemon
    let command = socket::Command::StopTranscription;

    match socket::send_command(&command).await {
        Ok(socket::Response::Success { message }) => {
            eprintln!("Success: {}", message);
            Ok(())
        }
        Ok(socket::Response::Error { message }) => {
            eprintln!("Error: {}", message);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to send command: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run_sendcmd() -> Result<()> {
    use tokio::io::AsyncReadExt;

    // Read JSON command from stdin
    let mut stdin = tokio::io::stdin();
    let mut input = String::new();
    stdin.read_to_string(&mut input).await?;

    // Parse command
    let command: socket::Command =
        serde_json::from_str(&input).map_err(|e| anyhow::anyhow!("Invalid JSON command: {}", e))?;

    // Send command to daemon
    match socket::send_command(&command).await {
        Ok(socket::Response::Success { message }) => {
            eprintln!("Success: {}", message);
            Ok(())
        }
        Ok(socket::Response::Error { message }) => {
            eprintln!("Error: {}", message);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to send command: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(not(test))]
fn handle_command(
    cmd: socket::Command,
    state: RealtimeState,
    transcriber: Arc<RealtimeTranscriber>,
    beep_player: BeepPlayer,
    config: Config,
    _pipe_to: Option<Vec<String>>, // Deprecated, kept for compatibility
) -> Result<socket::Response> {
    use std::sync::atomic::Ordering;

    match cmd {
        socket::Command::StartTranscription(args) => {
            if state.is_recording.load(Ordering::Relaxed) {
                return Ok(socket::Response::Error {
                    message: "Already streaming audio".to_string(),
                });
            }

            // Use provided language or fall back to config
            let language = args.language.or_else(|| {
                if config.whisper_language == "auto" {
                    None
                } else {
                    Some(config.whisper_language.clone())
                }
            });

            // Extract command execution settings from args
            let command_exec = args.command.clone();

            // Start transcription session asynchronously
            let state_clone = state.clone();
            let beep_player_clone = beep_player.clone();
            tokio::spawn(async move {
                // Play start beep
                let _ = beep_player_clone.play_async(BeepType::RecordingStart).await;

                match transcriber.start_session(language).await {
                    Ok((audio_tx, mut transcript_rx, ws_task)) => {
                        *state_clone.audio_sender.lock().await = Some(audio_tx);
                        *state_clone.ws_task.lock().await = Some(ws_task);
                        state_clone.is_recording.store(true, Ordering::Relaxed);

                        // Handle transcriptions
                        let state_inner = state_clone.clone();
                        let beep_player_inner = beep_player_clone.clone();
                        tokio::spawn(async move {
                            while let Some(result) = transcript_rx.recv().await {
                                match result {
                                    Ok(transcript) => {
                                        if !transcript.trim().is_empty() {
                                            eprintln!(
                                                "Real-time transcription: \"{}\"",
                                                transcript
                                            );

                                            // Execute command if specified
                                            match &command_exec {
                                                Some(socket::CommandExecution::SpawnForEachTranscription { command }) => {
                                                    if let Err(e) = command::execute_with_input(command, &transcript).await {
                                                        eprintln!("Failed to execute command: {}", e);
                                                    }
                                                }
                                                None => {
                                                    // Just print to stdout if no command specified
                                                    println!("{}", transcript);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Transcription error: {}", e);
                                        let _ = beep_player_inner.play_async(BeepType::Error).await;
                                        state_inner.is_recording.store(false, Ordering::Relaxed);
                                        if let Some(task) = state_inner.ws_task.lock().await.take()
                                        {
                                            task.abort();
                                        }
                                        *state_inner.audio_sender.lock().await = None;
                                        break;
                                    }
                                }
                            }
                        });

                        eprintln!("Audio streaming started");
                    }
                    Err(e) => {
                        eprintln!("Failed to start WebSocket session: {}", e);
                        let _ = beep_player_clone.play_async(BeepType::Error).await;
                    }
                }
            });

            Ok(socket::Response::Success {
                message: "Starting transcription".to_string(),
            })
        }
        socket::Command::StopTranscription => {
            if !state.is_recording.load(Ordering::Relaxed) {
                return Ok(socket::Response::Error {
                    message: "Not currently streaming".to_string(),
                });
            }

            // Stop transcription
            state.is_recording.store(false, Ordering::Relaxed);

            let state_clone = state.clone();
            let beep_player_clone = beep_player.clone();
            tokio::spawn(async move {
                // Play stop beep
                let _ = beep_player_clone.play_async(BeepType::RecordingStop).await;

                // Close audio sender
                *state_clone.audio_sender.lock().await = None;

                // Wait for final transcriptions
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                // Cancel WebSocket task
                if let Some(task) = state_clone.ws_task.lock().await.take() {
                    task.abort();
                }

                eprintln!("Audio streaming stopped");
            });

            Ok(socket::Response::Success {
                message: "Stopping transcription".to_string(),
            })
        }
    }
}

async fn run_daemon(envfile: Option<PathBuf>) -> Result<()> {
    // Determine the config file path
    let envfile = envfile.unwrap_or_else(get_default_config_path);

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

    eprintln!("HotLine - Real-time WebSocket Transcription");
    eprintln!("Using OpenAI Realtime API for instant transcription");
    eprintln!("Socket path: {}", socket::get_socket_path().display());
    eprintln!("Commands via UNIX socket:");
    eprintln!("  StartTranscription: Start streaming audio to OpenAI");
    eprintln!("  StopTranscription: Stop streaming (recording continues)");

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
        let transcriber = Arc::new(RealtimeTranscriber::with_model(
            api_key,
            config.realtime_model.clone(),
        ));

        // Create UNIX socket listener
        let listener = socket::create_socket_listener().await?;

        // Set up cleanup on exit
        let shutdown = Arc::new(AtomicBool::new(false));

        // Handle Ctrl+C for graceful shutdown
        let shutdown_ctrl_c = shutdown.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for Ctrl+C");
            shutdown_ctrl_c.store(true, Ordering::Relaxed);
        });

        // Spawn socket handler task
        let state_socket = state.clone();
        let transcriber_socket = transcriber.clone();
        let beep_player_socket = beep_player.clone();
        let config_socket = config.clone();
        let pipe_to_socket = None::<Vec<String>>;
        let shutdown_socket = shutdown.clone();

        let socket_task = tokio::spawn(async move {
            while !shutdown_socket.load(Ordering::Relaxed) {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let state_handler = state_socket.clone();
                        let transcriber_handler = transcriber_socket.clone();
                        let beep_player_handler = beep_player_socket.clone();
                        let config_handler = config_socket.clone();
                        let pipe_to_handler = pipe_to_socket.clone();

                        tokio::spawn(async move {
                            let _ = socket::handle_client(stream, |cmd| {
                                handle_command(
                                    cmd,
                                    state_handler.clone(),
                                    transcriber_handler.clone(),
                                    beep_player_handler.clone(),
                                    config_handler.clone(),
                                    pipe_to_handler.clone(),
                                )
                            })
                            .await;
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept socket connection: {}", e);
                    }
                }
            }
        });

        // Main loop
        loop {
            if shutdown.load(Ordering::Relaxed) {
                eprintln!("Shutting down daemon...");
                break;
            }

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

            // Small delay to prevent busy loop
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Cleanup on shutdown
        socket_task.abort();
        let _ = socket_task.await;

        // Stop recording
        if let Err(e) = recorder.stop_recording() {
            eprintln!("Failed to stop recording: {}", e);
        }

        // Cancel WebSocket task if streaming
        if let Some(task) = state.ws_task.lock().await.take() {
            task.abort();
        }

        // Clean up socket file
        socket::cleanup_socket();
    }

    // During tests, just return early without signal handling
    #[cfg(test)]
    {
        eprintln!("Test mode: Socket handling disabled");
    }

    eprintln!("Exiting HotLine");
    Ok(())
}
