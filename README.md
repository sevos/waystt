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
- **OpenAI API key** (for Whisper transcription)
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
wget https://github.com/sevos/waystt/releases/download/v0.1.1/waystt-linux-x86_64
mkdir -p ~/.local/bin
mv waystt-linux-x86_64 ~/.local/bin/waystt
chmod +x ~/.local/bin/waystt

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$HOME/.local/bin:$PATH"
```

## Quick Start

1. **Setup configuration:**
```bash
# Create config directory and file
mkdir -p ~/.config/waystt
echo "OPENAI_API_KEY=your_api_key_here" > ~/.config/waystt/.env
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

## Keyboard Shortcuts Setup

### Hyprland

Add to your `~/.config/hypr/hyprland.conf`:

```bash
# waystt - Speech to Text (direct typing)
bind = SUPER, R, exec, pgrep -x waystt >/dev/null && pkill -USR1 waystt || (waystt --pipe-to ydotool type --file - &)

# waystt - Speech to Text (clipboard copy)  
bind = SUPER SHIFT, R, exec, pgrep -x waystt >/dev/null && pkill -USR1 waystt || (waystt --pipe-to wl-copy &)
```

These keybindings will:
- **Super+R**: Start waystt with output piped to ydotool for direct typing, or send SIGUSR1 if already running
- **Super+Shift+R**: Start waystt with output piped to clipboard, or send SIGUSR1 if already running

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

### Niri

Add to your `~/.config/niri/config.kdl`:

```kdl
binds {
    // waystt - Speech to Text (direct typing)
    Mod+R { spawn "sh" "-c" "pgrep -x waystt >/dev/null && pkill -USR1 waystt || (waystt --pipe-to ydotool type --file - &)"; }
    
    // waystt - Speech to Text (clipboard copy)
    Mod+Shift+R { spawn "sh" "-c" "pgrep -x waystt >/dev/null && pkill -USR1 waystt || (waystt --pipe-to wl-copy &)"; }
}
```

## Configuration

Configuration is read from `~/.config/waystt/.env` by default. You can override this location using the `--envfile` flag:

```bash
waystt --envfile /path/to/custom/.env
```

waystt supports two transcription providers: **OpenAI Whisper** (default) and **Google Speech-to-Text**. Choose the one that best fits your needs.

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

```bash
git clone https://github.com/sevos/waystt.git
cd waystt

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