# ☎️ HotLine

**HotLine** is an open source, minimalist speech-to-text (STT) tool that channels the charm of classic telephony into a modern user experience powered by OpenAI's real-time transcription API.

## 🔊 Philosophy

**HotLine** is built on the idea that speech interfaces should be effortless, responsive, and maybe even a little fun.

- **Zero friction.** No windows, prompts, or configuration menus. You trigger it, you speak, it types.
- **Uncanny familiarity.** We grew up with phones that clicked, hummed, and beeped. HotLine borrows those auditory signals to communicate its state: on, off, listening, or error — all without cluttering your screen.
- **Free as in freedom.** HotLine is fully open source, made to be hacked, extended, and improved.
- **Focused on joy.** Computers should be delightful. HotLine takes a serious tool — speech recognition — and wraps it in a nostalgic, playful skin.

## 🎧 User Experience

When you activate HotLine, it plays a soft **off-hook tone**, just like picking up an old-school phone. That's your cue: HotLine is now ready.

Tap the "start listening" signal? You'll hear a crisp **beep**, like an answering machine — and you're live. Your speech starts flowing into the current text field as keystrokes, automatically and naturally.

Need to stop? The familiar **"beep-beep-beep"** of a dropped call lets you know it's over.

And if something goes sideways — say, no API connection — you'll hear the unmistakable **three-tone SIT error signal** you've heard a thousand times before.

No visual distractions. Just you, your voice, and the comforting sounds of analog feedback.

## Requirements

- **OpenAI API key**
- **System packages**:
  ```bash
  # Arch Linux
  sudo pacman -S pipewire

  # Ubuntu/Debian
  sudo apt install pipewire-pulse

  # Fedora
  sudo dnf install pipewire-pulseaudio
  ```

## Installation

1.  **Download the latest binary** from the [GitHub Releases](https://github.com/sevos/hotline/releases) page.
2.  **Install the binary**:
    ```bash
    # Download the binary
    wget https://github.com/sevos/hotline/releases/latest/download/hotline-linux-x86_64

    # Make it executable
    chmod +x hotline-linux-x86_64

    # Move it to a directory in your PATH
    mkdir -p ~/.local/bin
    mv hotline-linux-x86_64 ~/.local/bin/hotline

    # Add ~/.local/bin to your PATH if it's not already (add to ~/.bashrc or ~/.zshrc)
    export PATH="$HOME/.local/bin:$PATH"
    ```

## Configuration

HotLine supports multiple configuration methods with the following precedence (highest to lowest):
1. Command-line arguments
2. Environment variables
3. TOML configuration file
4. Default values

### TOML Configuration (Recommended)

Create a configuration file at `~/.config/hotline/hotline.toml`:

```toml
# OpenAI API configuration
openai_api_key = "your-api-key-here"

# Real-time transcription model
# Options: "whisper-1", "gpt-4o-transcribe", "gpt-4o-mini-transcribe"
realtime_model = "whisper-1"

# Audio configuration
audio_buffer_duration_seconds = 300  # 5 minutes
audio_sample_rate = 16000
audio_channels = 1

# Whisper settings
whisper_language = "auto"  # or "en", "es", "fr", etc.

# Audio feedback
enable_audio_feedback = true
beep_volume = 0.1  # Range: 0.0 to 1.0

# Transcription profiles for different use cases
[profiles.default]
model = "whisper-1"
language = "en"

[profiles.coding]
model = "gpt-4o-mini-transcribe"
language = "en"
prompt = "The user is a programmer, so expect technical terms."

[profiles.coding-spanish]
model = "gpt-4o-mini-transcribe"
language = "es"
prompt = "El usuario es un programador escribiendo código. Espera términos técnicos de programación, nombres de funciones, variables en inglés mezclados con español."
vad_config = { SemanticVad = { eagerness = "medium" } }

[profiles.meeting]
model = "whisper-1"
language = "auto"
prompt = "This is a business meeting with multiple speakers."
```

### Environment Variables

Alternatively, create `~/.config/hotline/.env`:

```ini
# Required: Your OpenAI API Key
OPENAI_API_KEY=your_api_key_here

# Optional: Specify the transcription model
# Options: whisper-1, gpt-4o-transcribe, gpt-4o-mini-transcribe
REALTIME_MODEL=whisper-1

# Optional: Set the language for transcription (default: auto-detect)
# Use ISO 639-1 codes, e.g., "en", "es", "fr"
WHISPER_LANGUAGE=en

# Optional: Disable audio feedback beeps (default: true)
ENABLE_AUDIO_FEEDBACK=false

# Optional: Adjust beep volume (default: 0.1, range: 0.0 to 1.0)
BEEP_VOLUME=0.05
```

### Viewing Configuration

To validate and view your current configuration:

```bash
hotline config
```

## Usage

HotLine now uses a modern architecture with subcommands and UNIX socket communication.

### Starting the Daemon

Start the HotLine daemon (background service):

```bash
# Run in foreground to see logs
hotline daemon

# Or run in background
hotline daemon &

# Use a custom environment file
hotline daemon --envfile /path/to/.env
```

### Controlling Transcription

HotLine provides two ways to control transcription:

#### User-Friendly Commands (Recommended)

```bash
# Start transcription with a profile
hotline start-transcription default
hotline start-transcription coding
hotline start-transcription coding-spanish

# Stop current transcription
hotline stop-transcription
```

#### Advanced JSON Commands

For advanced users and scripting, you can send raw JSON commands:

```bash
# Start transcription with custom settings
echo '{
  "StartTranscription": {
    "model": "gpt-4o-transcribe",
    "language": "en",
    "prompt": "Technical documentation",
    "command": {
      "type": "spawn_for_each",
      "command": ["wl-copy"]
    }
  }
}' | hotline sendcmd

# Stop transcription
echo '{"StopTranscription": null}' | hotline sendcmd
```

### Command Execution

Commands are configured using a tagged format that supports extensibility for future command types.

Currently supported:
- **spawn_for_each**: Spawns command for each transcription, piping text to stdin
  - Copying text to clipboard: `["wl-copy"]`
  - Typing text directly: `["ydotool", "type", "--file", "-"]`
  - Saving to a file: `["tee", "-a", "/tmp/transcript.txt"]`

The tagged format allows future command types with different fields (e.g., `spawn_once`, `write_to_file`, `http_post`, etc.)

Configure commands in your profile or pass them in the JSON command.

## Voice Activity Detection (VAD)

HotLine supports two VAD modes:

### Server VAD (Default)
Traditional threshold-based voice detection:
```json
{
  "vad_config": {
    "ServerVad": {
      "threshold": 0.5,
      "prefix_padding_ms": 300,
      "silence_duration_ms": 500
    }
  }
}
```

### Semantic VAD
AI-powered context-aware voice detection:
```json
{
  "vad_config": {
    "SemanticVad": {
      "eagerness": "medium"  // Options: "low", "medium", "high"
    }
  }
}
```

## Keybinding Examples

The most effective way to use HotLine is to bind commands to hotkeys.

### Hyprland

Add to `~/.config/hypr/hyprland.conf`:

```ini
# Start/stop transcription with Super+R
bind = SUPER, R, exec, hotline start-transcription default
bind = SUPER SHIFT, R, exec, hotline stop-transcription

# Push-to-talk style with submap
bind = SUPER, T, submap, speech
submap = speech
binde=, SUPER_L, exec, hotline start-transcription coding
bindr=, SUPER_L, exec, hotline stop-transcription
bind=, escape, submap, reset
submap = reset
```

### Niri

Add to `~/.config/niri/config.kdl`:

```kdl
binds {
    // Toggle transcription
    Mod+R {
        spawn "sh" "-c" "if pgrep -f 'hotline daemon' >/dev/null; then hotline stop-transcription || hotline start-transcription default; else echo 'HotLine daemon not running'; fi";
    }
    
    // Start with specific profile
    Mod+Shift+R {
        spawn "hotline" "start-transcription" "coding";
    }
}
```

### Using `ydotool` for Direct Typing

To type transcribed text directly into any application:

1.  **Install `ydotool`**:
    ```bash
    # Arch Linux
    sudo pacman -S ydotool
    ```

2.  **Setup permissions**:
    ```bash
    sudo usermod -a -G input $USER
    ```

3.  **Start the `ydotool` daemon**:
    ```bash
    systemctl --user enable --now ydotool.service
    ```

4.  **Configure in your profile**:
    ```toml
    [profiles.typing]
    model = "whisper-1"
    language = "en"
    
    [profiles.typing.command]
    type = "spawn_for_each"
    command = ["ydotool", "type", "--file", "-"]
    ```

## Architecture

HotLine uses a client-server architecture:

- **Daemon**: Long-running background service that handles audio recording and transcription
- **Client Commands**: Send commands to the daemon via UNIX socket
- **Socket Path**: `$XDG_RUNTIME_DIR/hotline.sock` (or `~/.config/hotline/hotline.sock`)

## Troubleshooting

### No transcription
- Ensure the daemon is running: `pgrep -f "hotline daemon"`
- Verify your `OPENAI_API_KEY` is correct and has credits
- Check daemon logs by running in foreground: `hotline daemon`
- Verify socket exists: `ls -la $XDG_RUNTIME_DIR/hotline.sock`

### Audio issues
- Ensure PipeWire is running: `systemctl --user status pipewire`
- Check microphone is not muted: `pavucontrol` or `alsamixer`
- Test with different audio sample rates in config

### Socket errors
- Check socket permissions: `ls -la $XDG_RUNTIME_DIR/hotline.sock`
- Ensure only one daemon instance is running
- Try removing stale socket: `rm $XDG_RUNTIME_DIR/hotline.sock`

### Configuration issues
- Validate configuration: `hotline config`
- Check file permissions on config files
- Ensure TOML syntax is correct

## Development

### Building from Source

```bash
# Clone the repository
git clone https://github.com/sevos/hotline
cd hotline

# Build release version
cargo build --release

# Binary will be at target/release/hotline
```

### Testing

```bash
# Run all tests (with audio feedback disabled)
BEEP_VOLUME=0.0 cargo test

# Run specific test
BEEP_VOLUME=0.0 cargo test test_name

# Check code formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --all-targets -- -D warnings
```

### Project Structure

```
hotline/
├── src/
│   ├── main.rs           # CLI and subcommands
│   ├── socket.rs         # UNIX socket communication
│   ├── config.rs         # Configuration management
│   ├── audio.rs          # Audio recording (PipeWire/CPAL)
│   ├── beep.rs           # Audio feedback system
│   ├── transcription/
│   │   ├── mod.rs        # Transcription provider abstraction
│   │   └── realtime.rs   # OpenAI Real-time API client
│   └── command.rs        # Command execution utilities
├── hotline.toml.example  # Example configuration
└── README.md
```

## Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and linting
5. Submit a pull request

## License

Licensed under GPL v3.0 or later. See [LICENSE](LICENSE) for details.

---
*Source code: https://github.com/sevos/hotline*