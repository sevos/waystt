use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(clippy::enum_variant_names)] // Transcription suffix is intentional for clarity
pub enum Command {
    StartTranscription(StartTranscriptionArgs),
    StopTranscription,
    ToggleTranscription(StartTranscriptionArgs), // Same args as Start for profile support
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StartTranscriptionArgs {
    // Transcription parameters
    pub model: Option<String>,
    pub language: Option<String>,
    pub prompt: Option<String>,

    // VAD parameters
    pub vad_config: Option<VadConfig>,

    // Lifecycle hooks
    pub hooks: Option<Hooks>,
}

/// Lifecycle hooks for executing commands at different stages of transcription
///
/// TOML format:
/// ```toml
/// [profiles.example.hooks.on_transcription_start]
/// type = "spawn"
/// command = ["notify-send", "Recording started"]
///
/// [profiles.example.hooks.on_transcription_receive]
/// type = "spawn_with_stdin"
/// command = ["wl-copy"]
///
/// [profiles.example.hooks.on_transcription_stop]
/// type = "spawn"
/// command = ["notify-send", "Recording stopped"]
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Hooks {
    /// Hook executed when transcription starts
    pub on_transcription_start: Option<CommandExecution>,
    /// Hook executed when transcription text is received (text piped to stdin)
    pub on_transcription_receive: Option<CommandExecution>,
    /// Hook executed when transcription stops
    pub on_transcription_stop: Option<CommandExecution>,
}

/// Command execution configuration using tagged enum for extensibility.
/// Each variant can have different fields specific to its execution strategy.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandExecution {
    /// Spawn command without stdin (for start/stop hooks)
    Spawn { command: Vec<String> },
    /// Spawn command with text piped to stdin (for receive hook)
    SpawnWithStdin { command: Vec<String> },
    // Future command types can be added here with different fields
    // e.g., SpawnOnce { command: Vec<String>, timeout: u64 },
    // e.g., WriteToFile { path: String, append: bool },
    // e.g., HttpPost { url: String, headers: HashMap<String, String> },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum VadConfig {
    ServerVad(ServerVadConfig),
    SemanticVad(SemanticVadConfig),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerVadConfig {
    pub threshold: Option<f32>,
    pub prefix_padding_ms: Option<u32>,
    pub silence_duration_ms: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SemanticVadConfig {
    pub eagerness: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Success { message: String },
    Error { message: String },
}

use anyhow::Result;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

/// Get the socket path, using XDG_RUNTIME_DIR if available, otherwise XDG_CONFIG_DIR
pub fn get_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("hotline.sock")
    } else if let Ok(config_dir) = std::env::var("XDG_CONFIG_DIR") {
        PathBuf::from(config_dir)
            .join("hotline")
            .join("hotline.sock")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| {
                std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from)
            })
            .join("hotline")
            .join("hotline.sock")
    }
}

/// Create a UNIX socket listener
pub async fn create_socket_listener() -> Result<UnixListener> {
    let socket_path = get_socket_path();

    // Ensure parent directory exists
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove existing socket if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    eprintln!("UNIX socket listening at: {}", socket_path.display());

    Ok(listener)
}

/// Clean up the socket file
pub fn cleanup_socket() {
    let socket_path = get_socket_path();
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
        eprintln!("Cleaned up socket file: {}", socket_path.display());
    }
}

/// Send a command to the daemon via UNIX socket
pub async fn send_command(command: &Command) -> Result<Response> {
    let socket_path = get_socket_path();

    if !socket_path.exists() {
        return Ok(Response::Error {
            message: format!(
                "Daemon not running. Socket not found at: {}",
                socket_path.display()
            ),
        });
    }

    let mut stream = UnixStream::connect(&socket_path).await?;

    // Serialize command to JSON
    let json = serde_json::to_string(command)?;

    // Send command with newline delimiter
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;

    // Parse response
    let response: Response = serde_json::from_str(&response_line)?;

    Ok(response)
}

/// Handle a client connection
pub async fn handle_client(
    mut stream: UnixStream,
    command_handler: impl Fn(Command) -> Result<Response>,
) -> Result<()> {
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();

    // Read command from client
    reader.read_line(&mut line).await?;

    // Parse command
    let response = match serde_json::from_str::<Command>(&line) {
        Ok(command) => {
            eprintln!("Received command: {:?}", command);
            command_handler(command).unwrap_or_else(|e| Response::Error {
                message: format!("Command execution failed: {}", e),
            })
        }
        Err(e) => Response::Error {
            message: format!("Invalid command format: {}", e),
        },
    };

    // Send response
    let response_json = serde_json::to_string(&response)?;
    stream.write_all(response_json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    Ok(())
}
