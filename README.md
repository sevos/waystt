# waystt - Wayland Speech-to-Text Tool

Press a keybind, speak, and get instant text output. A speech-to-text tool that transcribes audio using OpenAI Whisper and outputs to stdout.

## Features

- **Signal-driven**: Press keybind → speak → get text (no GUI needed)
- **UNIX philosophy**: Outputs transcribed text to stdout for piping to other tools
- **On-demand operation**: Starts when called, processes audio, then exits
- **Audio feedback**: Beeps confirm recording start/stop and success
- **Wayland native**: Works with modern Linux desktops (Hyprland, Niri, etc.)

## Requirements

- **Wayland desktop** (Hyprland, Niri, GNOME, KDE, etc.)
- **OpenAI API key** (for Whisper transcription) OR **Local Whisper model** OR **Google Cloud credentials**
- **System packages**:

```bash
# Arch Linux
sudo pacman -S pipewire

# Ubuntu/Debian  
sudo apt install pipewire-pulse

# Fedora
sudo dnf install pipewire-pulseaudio
```

**Optional (for direct typing keybindings):**
```bash
# Arch Linux
sudo pacman -S ydotool

# Ubuntu/Debian  
sudo apt install ydotool

# Fedora
sudo dnf install ydotool

# Setup ydotool permissions and service:
sudo usermod -a -G input $USER

# Enable and start ydotool daemon service
sudo systemctl enable --now ydotool.service

# Set socket environment variable (add to ~/.bashrc or ~/.zshrc)
echo 'export YDOTOOL_SOCKET=/tmp/.ydotool_socket' >> ~/.bashrc

# Log out and back in (or source ~/.bashrc)
```

## Installation

### From AUR (Arch Linux)

```bash
# Using your preferred AUR helper
yay -S waystt-bin
# or
paru -S waystt-bin
```

### Download Binary

1. Download from [GitHub Releases](https://github.com/sevos/waystt/releases)
2. Install:

```bash
wget https://github.com/sevos/waystt/releases/latest/download/waystt-linux-x86_64
mkdir -p ~/.local/bin
mv waystt-linux-x86_64 ~/.local/bin/waystt
chmod +x ~/.local/bin/waystt

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$HOME/.local/bin:$PATH"
```

## Quick Start

1. **Setup configuration (choose one provider):**

   **Option A: OpenAI Whisper (default)**
   ```bash
   # Create config directory and file
   mkdir -p ~/.config/waystt
   echo "OPENAI_API_KEY=your_api_key_here" > ~/.config/waystt/.env
   ```
   
   **Option B: Local Whisper (offline)**
   ```bash
   # Download a model and configure
   mkdir -p ~/.local/share/waystt/models ~/.config/waystt
   cd ~/.local/share/waystt/models
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
   
   # Create config
   cat > ~/.config/waystt/.env << EOF
   TRANSCRIPTION_PROVIDER=local
   LOCAL_MODEL_PATH=$HOME/.local/share/waystt/models/ggml-base.bin
   EOF
   ```
   
   **Option C: Google Speech-to-Text**
   ```bash
   # After setting up Google Cloud (see Configuration section)
   mkdir -p ~/.config/waystt
   cat > ~/.config/waystt/.env << EOF
   TRANSCRIPTION_PROVIDER=google
   GOOGLE_APPLICATION_CREDENTIALS=/path/to/your/service-account-key.json
   EOF
   ```

2. **Test the application:**
```bash
# Run waystt and pipe output to see it working
waystt | tee /tmp/waystt-output.txt
```

3. **Use with signals:**
```bash
# Transcribe and output to stdout
pkill --signal SIGUSR1 waystt
```

## Quick Reference

### Common Commands

```bash
# Start waystt and save output to file
waystt > output.txt

# Start waystt and copy output to clipboard
waystt --pipe-to wl-copy

# Start waystt and type output directly
waystt --pipe-to ydotool type --file -

# Trigger transcription (if waystt is running)
pkill --signal SIGUSR1 waystt
```

### Keybinding Pattern

Most keybindings follow this pattern:
```bash
pgrep -x waystt >/dev/null && pkill --signal SIGUSR1 waystt || (waystt [OPTIONS] &)
```

This means: "If waystt is running, send signal to transcribe. Otherwise, start waystt with specified options."

## Keyboard Shortcuts Setup

### Hyprland

Add to your `~/.config/hypr/hyprland.conf`:

```bash
# waystt - Speech to Text (direct typing)
bind = SUPER, R, exec, pgrep -x waystt >/dev/null && pkill --signal SIGUSR1 waystt || (waystt --pipe-to ydotool type --file - &)

# waystt - Speech to Text (clipboard copy)  
bind = SUPER SHIFT, R, exec, pgrep -x waystt >/dev/null && pkill --signal SIGUSR1 waystt || (waystt --pipe-to wl-copy &)
```

### Niri

Add to your `~/.config/niri/config.kdl`:

```kdl
binds {
    // waystt - Speech to Text (direct typing)
    Mod+R { spawn "sh" "-c" "pgrep -x waystt >/dev/null && pkill --signal SIGUSR1 waystt || (waystt --pipe-to ydotool type --file - &)"; }
    
    // waystt - Speech to Text (clipboard copy)
    Mod+Shift+R { spawn "sh" "-c" "pgrep -x waystt >/dev/null && pkill --signal SIGUSR1 waystt || (waystt --pipe-to wl-copy &)"; }
}
```

**Keybinding Functions:**
- **Super+R** (Hyprland) / **Mod+R** (Niri): Direct typing via ydotool
- **Super+Shift+R** (Hyprland) / **Mod+Shift+R** (Niri): Copy to clipboard

## Usage Examples

waystt starts on-demand, records audio, transcribes it, outputs to stdout, then exits:

### Basic Usage (stdout)

```bash
# Terminal 1: Start waystt with output to file
waystt > transcription.txt

# Terminal 2: Trigger transcription (or use keyboard shortcut)
pkill --signal SIGUSR1 waystt
```

### Using --pipe-to Option

The `--pipe-to` option allows you to pipe transcribed text directly to another command:

```bash
# Copy transcription to clipboard
waystt --pipe-to wl-copy
pkill --signal SIGUSR1 waystt

# Type transcription directly into focused window
waystt --pipe-to ydotool type --file -
pkill --signal SIGUSR1 waystt

# Process transcription with sed and copy to clipboard
waystt --pipe-to sh -c "sed 's/hello/hi/g' | wl-copy"
pkill --signal SIGUSR1 waystt

# Save to file with timestamp
waystt --pipe-to sh -c "echo \"$(date): $(cat)\" >> speech-log.txt"
pkill --signal SIGUSR1 waystt
```


## Configuration

Configuration is read from `~/.config/waystt/.env` by default. You can override this location using the `--envfile` flag:

```bash
waystt --envfile /path/to/custom/.env
```

waystt supports three transcription providers: **OpenAI Whisper** (default), **Local Whisper**, and **Google Speech-to-Text**. Choose the one that best fits your needs.

### OpenAI Whisper (Default)

OpenAI Whisper offers excellent accuracy and supports automatic language detection.

**Required:** Create `~/.config/waystt/.env` with your OpenAI API key:

```bash
OPENAI_API_KEY=your_api_key_here
```

**Optional OpenAI settings:**
```bash
# Whisper model (whisper-1 is default, most cost-effective)
WHISPER_MODEL=whisper-1

# Force specific language (default: auto-detect)
WHISPER_LANGUAGE=en

# API timeout in seconds
WHISPER_TIMEOUT_SECONDS=60

# Max retry attempts
WHISPER_MAX_RETRIES=3
```

### Local Whisper (Offline)

Local Whisper runs entirely offline using local Whisper models, providing privacy and independence from external APIs.

**Benefits:**
- **Complete privacy** - No data sent to external services
- **No API costs** - Free to use once model is downloaded
- **Works offline** - No internet connection required
- **Fast processing** - Depends on your hardware

**Setup Steps:**

1. **Download a Whisper model:**
   
   Models are available from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp/tree/main):

   ```bash
   # Create models directory
   mkdir -p ~/.local/share/waystt/models
   cd ~/.local/share/waystt/models
   
   # Download a model (choose one based on your needs):
   
   # Tiny model (39 MB) - Fastest, basic accuracy
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin
   
   # Base model (142 MB) - Good balance of speed and accuracy  
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
   
   # Small model (244 MB) - Better accuracy, slower than base
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin
   
   # Medium model (769 MB) - Even better accuracy
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin
   
   # Large model (1550 MB) - Best accuracy, slowest processing
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin
   ```

2. **Configure waystt for local processing:**

   ```bash
   # Switch to local provider
   TRANSCRIPTION_PROVIDER=local
   
   # Path to your downloaded model file
   LOCAL_MODEL_PATH=/home/username/.local/share/waystt/models/ggml-base.bin
   
   # Language setting (optional, default: auto-detect)
   WHISPER_LANGUAGE=en
   ```

**Model Selection Guide:**
- **ggml-tiny.bin** - Ultra-fast, suitable for simple commands and phrases
- **ggml-base.bin** - **Recommended for most users** - Good balance of speed and accuracy
- **ggml-small.bin** - Better accuracy for longer sentences
- **ggml-medium.bin** - High accuracy for complex speech
- **ggml-large-v3.bin** - Best accuracy for challenging audio conditions

**Hardware Requirements:**
- **RAM**: 1-4 GB depending on model size
- **CPU**: Modern multi-core processor recommended for faster processing
- **Storage**: 39 MB to 1.5 GB for model files

### Google Speech-to-Text

Google Speech-to-Text provides fast, accurate transcription with support for many languages and dialects.

**Setup Steps:**

1. **Enable Google Cloud Speech-to-Text API:**
   - Go to [Google Cloud Console](https://console.cloud.google.com/)
   - Create a new project or select existing one
   - Enable the "Cloud Speech-to-Text API"
   - Create a service account and download the JSON key file

2. **Configure waystt for Google:**

```bash
# Switch to Google provider
TRANSCRIPTION_PROVIDER=google

# Path to your service account JSON file
GOOGLE_APPLICATION_CREDENTIALS=/path/to/your/service-account-key.json

# Primary language (default: en-US)
GOOGLE_SPEECH_LANGUAGE_CODE=en-US

# Model selection (latest_long for longer audio, latest_short for shorter)
GOOGLE_SPEECH_MODEL=latest_long

# Optional: Alternative languages for auto-detection (comma-separated)
GOOGLE_SPEECH_ALTERNATIVE_LANGUAGES=es-ES,fr-FR,de-DE
```

**Popular Google language codes:**
- `en-US` - English (United States)
- `en-GB` - English (United Kingdom)
- `es-ES` - Spanish (Spain)
- `fr-FR` - French (France)
- `de-DE` - German (Germany)
- `ja-JP` - Japanese
- `zh-CN` - Chinese (Simplified)

### General Settings

**Audio and system settings (apply to both providers):**
```bash
# Disable audio beeps
ENABLE_AUDIO_FEEDBACK=false

# Adjust beep volume (0.0 to 1.0)
BEEP_VOLUME=0.1

# Debug logging
RUST_LOG=debug
```


## Troubleshooting

### Audio Issues

If audio recording fails:
- Ensure PipeWire is running: `systemctl --user status pipewire`
- Check microphone permissions
- Verify microphone is not muted


### API Issues

**OpenAI Provider:**
- Verify your OpenAI API key is valid and has sufficient credits
- Check internet connectivity
- Review logs for specific error messages

**Google Provider:**
- Verify your service account JSON file path is correct
- Ensure the Speech-to-Text API is enabled in your Google Cloud project
- Check that your service account has the necessary permissions
- Verify your Google Cloud project has billing enabled
- Review logs for specific error messages

**Local Provider:**
- Verify your model file path is correct: `ls -la /path/to/your/model.bin`
- Ensure the model file is downloaded completely (check file size)
- Check available RAM - larger models require more memory
- Review logs for specific error messages
- Try a smaller model if you encounter memory issues

## Development

### Running Tests

```bash
cargo test
```

### Running with Debug Output

```bash
# Using default config location (~/.config/waystt/.env)
RUST_LOG=debug cargo run

# Or using project-local .env file for development
RUST_LOG=debug cargo run -- --envfile .env
```

## Building from Source

### Using Nix (Recommended)

If you have Nix installed, the project includes a development shell with all dependencies:

```bash
git clone https://github.com/sevos/waystt.git
cd waystt

# Enter development environment with all dependencies
nix-shell

# Create config directory and copy example configuration
mkdir -p ~/.config/waystt
cp .env.example ~/.config/waystt/.env
# Edit ~/.config/waystt/.env with your API key

# Build and test
cargo build --release
cargo test

# Install to local bin
mkdir -p ~/.local/bin
cp ./target/release/waystt ~/.local/bin/
```

### Traditional Build

```bash
git clone https://github.com/sevos/waystt.git
cd waystt

# Install build dependencies (example for Ubuntu/Debian)
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libasound2-dev cmake clang

# Create config directory and copy example configuration
mkdir -p ~/.config/waystt
cp .env.example ~/.config/waystt/.env
# Edit ~/.config/waystt/.env with your API key

# Build the project
cargo build --release

# Install to local bin
mkdir -p ~/.local/bin
cp ./target/release/waystt ~/.local/bin/
```

## License

Licensed under GPL v3.0 or later. Source code: https://github.com/sevos/waystt

See [LICENSE](LICENSE) for full terms.
