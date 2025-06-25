# waystt - Wayland Speech-to-Text Tool

A signal-driven speech-to-text application for Wayland environments that records audio and transcribes it using OpenAI's Whisper API.

## Features

- Continuous audio recording with signal-triggered transcription
- OpenAI Whisper API integration for high-quality speech recognition
- Persistent clipboard integration for Wayland
- Optimized for Wayland compositors (Hyprland, Niri, etc.)

## Dependencies

### System Dependencies

**Arch Linux:**
```bash
sudo pacman -S pipewire pipewire-pulse pipewire-alsa wtype
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install pipewire pipewire-pulse pipewire-alsa wtype
```

**Fedora:**
```bash
sudo dnf install pipewire pipewire-pulseaudio pipewire-alsa wtype
```

### Build Dependencies

- Rust (latest stable)
- Cargo
- PkgConfig
- ALSA development libraries

**Arch Linux:**
```bash
sudo pacman -S rust cargo pkgconf alsa-lib
```

**Ubuntu/Debian:**
```bash
sudo apt install rustc cargo pkg-config libasound2-dev
```

## Installation

### Building from Source

1. Clone the repository:
```bash
git clone <repository-url>
cd waystt
```

2. Create environment configuration:
```bash
cp .env.example .env
```

3. Edit `.env` and add your OpenAI API key:
```bash
OPENAI_API_KEY=your_api_key_here
```

4. Build the application:
```bash
cargo build --release
```

5. The binary will be available at `./target/release/waystt`

## Usage

### Starting the Application

Run waystt in the background:
```bash
nohup ./target/release/waystt > /tmp/waystt.log 2>&1 & disown
```

### Signal-Based Transcription

Once running, waystt continuously records audio and waits for signals:

- **SIGUSR2**: Stop recording, transcribe audio, and copy text to clipboard

Send signals using:
```bash
pkill --signal SIGUSR2 waystt
```

### Checking Logs

Monitor application output:
```bash
tail -f /tmp/waystt.log
```

## Keyboard Shortcuts Setup

### Hyprland

Add to your `~/.config/hypr/hyprland.conf`:

```bash
# waystt - Speech to Text
bind = SUPER, R, exec, pgrep -x waystt >/dev/null && pkill -USR2 waystt || waystt &
```

This keybinding (Super+R) will:
- Start waystt if not running
- Send SIGUSR2 signal to transcribe and copy to clipboard if already running

### Niri

Add to your `~/.config/niri/config.kdl`:

```kdl
binds {
    // waystt - Speech to Text
    Mod+R { spawn "sh" "-c" "pgrep -x waystt >/dev/null && pkill -USR2 waystt || waystt &"; }
}
```

## Configuration

The application reads configuration from `.env` file or environment variables:

```bash
# Required
OPENAI_API_KEY=your_api_key_here

# Optional (with defaults)
WHISPER_MODEL=whisper-1
WHISPER_LANGUAGE=auto
WHISPER_TIMEOUT_SECONDS=30
WHISPER_MAX_RETRIES=3
```

### Configuration Options

- `OPENAI_API_KEY`: Your OpenAI API key (required)
- `WHISPER_MODEL`: OpenAI Whisper model to use (default: whisper-1)
- `WHISPER_LANGUAGE`: Language for transcription (default: auto)
- `WHISPER_TIMEOUT_SECONDS`: API timeout in seconds (default: 30)
- `WHISPER_MAX_RETRIES`: Number of retry attempts (default: 3)

## Workflow

1. **Start Recording**: Launch waystt - it immediately begins recording audio
2. **Speak**: Talk into your microphone
3. **Trigger Transcription**: Press your configured keybind (Super+R) or send SIGUSR2
4. **Get Result**: Transcribed text is copied to clipboard
5. **Paste**: Use Ctrl+V to paste the transcribed text anywhere

## Troubleshooting

### Audio Issues

If audio recording fails:
- Ensure PipeWire is running: `systemctl --user status pipewire`
- Check microphone permissions
- Verify microphone is not muted

### Clipboard Issues

If clipboard operations fail:
- Ensure you're running under Wayland: `echo $WAYLAND_DISPLAY`
- Install wtype: Required for clipboard pasting functionality

### API Issues

If transcription fails:
- Verify your OpenAI API key is valid
- Check internet connectivity
- Review logs for specific error messages

## Development

### Running Tests

```bash
cargo test
```

### Running with Debug Output

```bash
RUST_LOG=debug cargo run
```

## License

[Add your license information here]