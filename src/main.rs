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
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
#[cfg(not(test))]
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

mod audio;
mod beep;
mod command;
mod command_executor;
mod config;
mod mpris;
mod socket;
mod sound_manager;
mod transcription;

#[cfg(test)]
mod test_utils;
#[cfg(not(test))]
use audio::AudioRecorder;
#[cfg(not(test))]
use beep::BeepConfig;
#[cfg(not(test))]
use command_executor::CommandExecutor;
use config::Config;
#[cfg(not(test))]
use sound_manager::SoundManager;
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
    /// Toggle transcription (start if stopped, stop if running)
    ToggleTranscription {
        /// Profile name to use (when starting)
        profile: String,
    },
    /// Run the HotLine MPRIS controller
    Mpris {
        /// Profile name to use for toggling
        profile: String,
    },
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
    ws_shutdown: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    current_hooks: Arc<Mutex<Option<socket::Hooks>>>,
}

impl RealtimeState {
    fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_sender: Arc::new(Mutex::new(None)),
            ws_task: Arc::new(Mutex::new(None)),
            ws_shutdown: Arc::new(Mutex::new(None)),
            current_hooks: Arc::new(Mutex::new(None)),
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
        Commands::ToggleTranscription { profile } => run_toggle_transcription(profile).await,
        Commands::Mpris { profile } => mpris::run_mpris_server(profile).await,
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
        hooks: profile.hooks.clone(),
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

async fn run_toggle_transcription(profile_name: String) -> Result<()> {
    // Load configuration to get profiles
    let config = Config::load_with_precedence()?;

    // Get the specified profile
    let profile = config
        .get_profile(&profile_name)
        .ok_or_else(|| anyhow!("Profile '{}' not found in configuration", profile_name))?;

    // Create ToggleTranscription command from profile
    let command = socket::Command::ToggleTranscription(socket::StartTranscriptionArgs {
        model: profile.model.clone(),
        language: profile.language.clone(),
        prompt: profile.prompt.clone(),
        vad_config: profile.vad_config.clone(),
        hooks: profile.hooks.clone(),
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
    sound_manager: SoundManager,
    command_executor: CommandExecutor,
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

            // Extract hooks from args
            let hooks = args.hooks.clone();

            // Store hooks in state for use in stop command (will be done in the async block)
            let state_for_hooks = state.clone();

            // Start transcription session asynchronously
            let state_clone = state.clone();
            let sound_manager_clone = sound_manager.clone();
            let command_executor_clone = command_executor.clone();
            tokio::spawn(async move {
                // Store hooks in state for use in stop command
                *state_for_hooks.current_hooks.lock().await = hooks.clone();

                // Start playing RecordingStart sound indefinitely
                let sound_handle = match sound_manager_clone.play_recording_start().await {
                    Ok(handle) => handle,
                    Err(e) => {
                        eprintln!("Failed to play recording start sound: {}", e);
                        sound_manager_clone.play_error();
                        return;
                    }
                };

                // Try to establish WebSocket connection
                match transcriber.start_session(language).await {
                    Ok((audio_tx, mut transcript_rx, ws_task, shutdown_tx)) => {
                        // WebSocket connection successful - stop the RecordingStart sound
                        sound_handle.stop().await;

                        // Now execute on_transcription_start hook after sound has stopped
                        if let Some(ref hooks) = hooks {
                            if let Some(ref start_hook) = hooks.on_transcription_start {
                                match start_hook {
                                    socket::CommandExecution::Spawn { command }
                                    | socket::CommandExecution::SpawnWithStdin { command } => {
                                        command_executor_clone.execute_hook(
                                            "on_transcription_start",
                                            command,
                                            String::new(),
                                        );
                                    }
                                }
                            }
                        }

                        // Now we can start sending audio frames to the API
                        *state_clone.audio_sender.lock().await = Some(audio_tx);
                        *state_clone.ws_task.lock().await = Some(ws_task);
                        *state_clone.ws_shutdown.lock().await = Some(shutdown_tx);
                        state_clone.is_recording.store(true, Ordering::Relaxed);

                        // Handle transcriptions
                        let state_inner = state_clone.clone();
                        let sound_manager_inner = sound_manager_clone.clone();
                        let command_executor_inner = command_executor_clone.clone();
                        tokio::spawn(async move {
                            while let Some(result) = transcript_rx.recv().await {
                                match result {
                                    Ok(transcript) => {
                                        if !transcript.trim().is_empty() {
                                            eprintln!(
                                                "Real-time transcription: \"{}\"",
                                                transcript
                                            );

                                            // Execute on_transcription_receive hook or print to stdout
                                            if let Some(ref hooks) = hooks {
                                                if let Some(ref receive_hook) =
                                                    hooks.on_transcription_receive
                                                {
                                                    match receive_hook {
                                                        socket::CommandExecution::SpawnWithStdin { command }
                                                        | socket::CommandExecution::Spawn { command } => {
                                                            command_executor_inner.execute_hook(
                                                                "on_transcription_receive",
                                                                command,
                                                                transcript.clone()
                                                            );
                                                        }
                                                    }
                                                } else {
                                                    // No receive hook, print to stdout
                                                    println!("{}", transcript);
                                                }
                                            } else {
                                                // No hooks at all, print to stdout
                                                println!("{}", transcript);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Transcription error: {}", e);
                                        sound_manager_inner.play_error();
                                        state_inner.is_recording.store(false, Ordering::Relaxed);

                                        // Send shutdown signal to cleanly close WebSocket
                                        if let Some(shutdown_tx) =
                                            state_inner.ws_shutdown.lock().await.take()
                                        {
                                            let _ = shutdown_tx.send(()).await;
                                        }

                                        if let Some(task) = state_inner.ws_task.lock().await.take()
                                        {
                                            let _ = tokio::time::timeout(
                                                tokio::time::Duration::from_secs(1),
                                                task,
                                            )
                                            .await;
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
                        // WebSocket connection failed - stop the RecordingStart sound
                        sound_handle.stop().await;

                        eprintln!("Failed to start WebSocket session: {}", e);
                        sound_manager_clone.play_error();
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
            let sound_manager_clone = sound_manager.clone();
            let command_executor_clone = command_executor.clone();
            tokio::spawn(async move {
                // First, execute on_transcription_stop hook BEFORE playing stop sound
                let hooks = state_clone.current_hooks.lock().await.clone();
                if let Some(hooks) = hooks {
                    if let Some(stop_hook) = hooks.on_transcription_stop {
                        match stop_hook {
                            socket::CommandExecution::Spawn { command }
                            | socket::CommandExecution::SpawnWithStdin { command } => {
                                command_executor_clone.execute_hook(
                                    "on_transcription_stop",
                                    &command,
                                    String::new(),
                                );
                            }
                        }
                    }
                }

                // Now play stop beep and wait for it to complete
                if let Err(e) = sound_manager_clone.play_recording_stop().await {
                    eprintln!("Failed to play recording stop sound: {}", e);
                }

                // Only close audio resources AFTER the stop sound has finished
                *state_clone.audio_sender.lock().await = None;

                // Send shutdown signal to cleanly close WebSocket
                if let Some(shutdown_tx) = state_clone.ws_shutdown.lock().await.take() {
                    let _ = shutdown_tx.send(()).await;
                }

                // Wait for WebSocket task to complete cleanly
                if let Some(task) = state_clone.ws_task.lock().await.take() {
                    // Give it a moment to close cleanly, then abort if needed
                    let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), task).await;
                }

                // Clear hooks from state
                *state_clone.current_hooks.lock().await = None;

                eprintln!("Audio streaming stopped");
            });

            Ok(socket::Response::Success {
                message: "Stopping transcription".to_string(),
            })
        }
        socket::Command::ToggleTranscription(args) => {
            // Check if currently recording
            if state.is_recording.load(Ordering::Relaxed) {
                // Currently recording, so stop it
                state.is_recording.store(false, Ordering::Relaxed);

                let state_clone = state.clone();
                let sound_manager_clone = sound_manager.clone();
                let command_executor_clone = command_executor.clone();
                tokio::spawn(async move {
                    // First, execute on_transcription_stop hook BEFORE playing stop sound
                    let hooks = state_clone.current_hooks.lock().await.clone();
                    if let Some(hooks) = hooks {
                        if let Some(stop_hook) = hooks.on_transcription_stop {
                            match stop_hook {
                                socket::CommandExecution::Spawn { command }
                                | socket::CommandExecution::SpawnWithStdin { command } => {
                                    command_executor_clone.execute_hook(
                                        "on_transcription_stop",
                                        &command,
                                        String::new(),
                                    );
                                }
                            }
                        }
                    }

                    // Now play stop beep and wait for it to complete
                    if let Err(e) = sound_manager_clone.play_recording_stop().await {
                        eprintln!("Failed to play recording stop sound: {}", e);
                    }

                    // Only close audio resources AFTER the stop sound has finished
                    *state_clone.audio_sender.lock().await = None;

                    // Send shutdown signal to cleanly close WebSocket
                    if let Some(shutdown_tx) = state_clone.ws_shutdown.lock().await.take() {
                        let _ = shutdown_tx.send(()).await;
                    }

                    // Wait for WebSocket task to complete cleanly
                    if let Some(task) = state_clone.ws_task.lock().await.take() {
                        // Give it a moment to close cleanly, then abort if needed
                        let _ =
                            tokio::time::timeout(tokio::time::Duration::from_secs(2), task).await;
                    }

                    // Clear hooks from state
                    *state_clone.current_hooks.lock().await = None;

                    eprintln!("Audio streaming stopped (toggled)");
                });

                Ok(socket::Response::Success {
                    message: "Toggled: Stopping transcription".to_string(),
                })
            } else {
                // Not recording, so start it
                // This is the same logic as StartTranscription

                // Use provided language or fall back to config
                let language = args.language.or_else(|| {
                    if config.whisper_language == "auto" {
                        None
                    } else {
                        Some(config.whisper_language.clone())
                    }
                });

                // Extract hooks from args
                let hooks = args.hooks.clone();

                // Store hooks in state for use in stop command (will be done in the async block)
                let state_for_hooks = state.clone();

                // Start transcription session asynchronously
                let state_clone = state.clone();
                let sound_manager_clone = sound_manager.clone();
                let command_executor_clone = command_executor.clone();
                tokio::spawn(async move {
                    // Store hooks in state for use in stop command
                    *state_for_hooks.current_hooks.lock().await = hooks.clone();

                    // Start playing RecordingStart sound indefinitely
                    let sound_handle = match sound_manager_clone.play_recording_start().await {
                        Ok(handle) => handle,
                        Err(e) => {
                            eprintln!("Failed to play recording start sound: {}", e);
                            sound_manager_clone.play_error();
                            return;
                        }
                    };

                    // Try to establish WebSocket connection
                    match transcriber.start_session(language).await {
                        Ok((audio_tx, mut transcript_rx, ws_task, shutdown_tx)) => {
                            // WebSocket connection successful - stop the RecordingStart sound
                            sound_handle.stop().await;

                            // Now execute on_transcription_start hook after sound has stopped
                            if let Some(ref hooks) = hooks {
                                if let Some(ref start_hook) = hooks.on_transcription_start {
                                    match start_hook {
                                        socket::CommandExecution::Spawn { command }
                                        | socket::CommandExecution::SpawnWithStdin { command } => {
                                            command_executor_clone.execute_hook(
                                                "on_transcription_start",
                                                command,
                                                String::new(),
                                            );
                                        }
                                    }
                                }
                            }

                            // Now we can start sending audio frames to the API
                            *state_clone.audio_sender.lock().await = Some(audio_tx);
                            *state_clone.ws_task.lock().await = Some(ws_task);
                            *state_clone.ws_shutdown.lock().await = Some(shutdown_tx);
                            state_clone.is_recording.store(true, Ordering::Relaxed);

                            // Handle transcriptions
                            let state_inner = state_clone.clone();
                            let sound_manager_inner = sound_manager_clone.clone();
                            let command_executor_inner = command_executor_clone.clone();
                            tokio::spawn(async move {
                                while let Some(result) = transcript_rx.recv().await {
                                    match result {
                                        Ok(transcript) => {
                                            if !transcript.trim().is_empty() {
                                                eprintln!(
                                                    "Real-time transcription: \"{}\"",
                                                    transcript
                                                );

                                                // Execute on_transcription_receive hook or print to stdout
                                                if let Some(ref hooks) = hooks {
                                                    if let Some(ref receive_hook) =
                                                        hooks.on_transcription_receive
                                                    {
                                                        match receive_hook {
                                                            socket::CommandExecution::SpawnWithStdin { command }
                                                            | socket::CommandExecution::Spawn { command } => {
                                                                command_executor_inner.execute_hook(
                                                                    "on_transcription_receive",
                                                                    command,
                                                                    transcript.clone()
                                                                );
                                                            }
                                                        }
                                                    } else {
                                                        // No receive hook, print to stdout
                                                        println!("{}", transcript);
                                                    }
                                                } else {
                                                    // No hooks at all, print to stdout
                                                    println!("{}", transcript);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Transcription error: {}", e);
                                            // Play error beep
                                            sound_manager_inner.play_error();
                                        }
                                    }
                                }

                                // Cleanup when stream ends
                                state_inner.is_recording.store(false, Ordering::Relaxed);
                                *state_inner.audio_sender.lock().await = None;
                                eprintln!("Transcription stream ended");
                            });

                            eprintln!("Audio streaming started (toggled)");
                        }
                        Err(e) => {
                            // WebSocket connection failed - stop the RecordingStart sound
                            sound_handle.stop().await;

                            eprintln!("Failed to start transcription session: {}", e);
                            state_clone.is_recording.store(false, Ordering::Relaxed);
                            // Play error beep
                            sound_manager_clone.play_error();
                        }
                    }
                });

                Ok(socket::Response::Success {
                    message: "Toggled: Starting transcription".to_string(),
                })
            }
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
    eprintln!("  ToggleTranscription: Toggle streaming (start if stopped, stop if running)");

    eprintln!("Starting continuous recording...");

    // Main event loop
    #[cfg(not(test))]
    {
        // Initialize sound manager and command executor
        let beep_config = BeepConfig {
            enabled: config.enable_audio_feedback,
            volume: config.beep_volume,
        };
        let sound_manager = SoundManager::new(beep_config)?;
        let command_executor = CommandExecutor::new();

        // Create channel for audio samples
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();

        // Initialize audio recorder
        let mut recorder = match AudioRecorder::new() {
            Ok(recorder) => recorder,
            Err(e) => {
                eprintln!("Failed to initialize audio recorder: {}", e);
                sound_manager.play_error();
                return Err(e);
            }
        };
        recorder.set_audio_sender(audio_tx);

        // Start recording immediately
        if let Err(e) = recorder.start_recording() {
            eprintln!("Failed to start recording: {}", e);
            sound_manager.play_error();
            return Err(e);
        }

        // Play "line ready" sound (fire and forget, non-blocking)
        sound_manager.play_line_ready();
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
        let sound_manager_socket = sound_manager.clone();
        let command_executor_socket = command_executor.clone();
        let config_socket = config.clone();
        let pipe_to_socket = None::<Vec<String>>;
        let shutdown_socket = shutdown.clone();

        let socket_task = tokio::spawn(async move {
            while !shutdown_socket.load(Ordering::Relaxed) {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let state_handler = state_socket.clone();
                        let transcriber_handler = transcriber_socket.clone();
                        let sound_manager_handler = sound_manager_socket.clone();
                        let command_executor_handler = command_executor_socket.clone();
                        let config_handler = config_socket.clone();
                        let pipe_to_handler = pipe_to_socket.clone();

                        tokio::spawn(async move {
                            let _ = socket::handle_client(stream, |cmd| {
                                handle_command(
                                    cmd,
                                    state_handler.clone(),
                                    transcriber_handler.clone(),
                                    sound_manager_handler.clone(),
                                    command_executor_handler.clone(),
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
        if let Some(shutdown_tx) = state.ws_shutdown.lock().await.take() {
            let _ = shutdown_tx.send(()).await;
        }
        if let Some(task) = state.ws_task.lock().await.take() {
            let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), task).await;
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
