# waystt - Wayland Speech-to-Text Tool

A signal-driven speech-to-text application for Wayland environments that records audio and transcribes it using OpenAI's Whisper API.

## Features

- **Continuous Audio Recording**: Background recording with signal-triggered transcription
- **OpenAI Whisper Integration**: High-quality speech recognition using Whisper API
- **Dual Output Modes**: 
  - Direct text typing via ydotool (SIGUSR1)
  - Clipboard copy for manual pasting (SIGUSR2)
- **Musical Audio Feedback**: Pleasant beep patterns for user notifications
  - Recording start: "Ding dong" (C4 → E4)
  - Recording stop: "Dong ding" (E4 → C4) 
  - Success: "Ding ding" (E4 → E4)
  - Error: Warbling tone
- **Wayland Native**: Optimized for Wayland compositors (Hyprland, Niri, etc.)
- **Persistent Clipboard**: Background daemon for clipboard persistence

## Dependencies

### System Dependencies

**Required for all systems:**
- **Audio System**: PipeWire (for audio recording)
- **Text Input**: ydotool (for direct text typing via SIGUSR1)
- **Clipboard**: wtype (for clipboard operations via SIGUSR2) 
- **Environment**: Wayland display server

**Arch Linux:**
```bash
sudo pacman -S pipewire pipewire-pulse pipewire-alsa ydotool wtype
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install pipewire pipewire-pulse pipewire-alsa ydotool wtype
```

**Fedora:**
```bash
sudo dnf install pipewire pipewire-pulseaudio pipewire-alsa ydotool wtype
```

**Post-installation setup for ydotool:**
```bash
# Add user to input group for ydotool permissions
sudo usermod -a -G input $USER
# Log out and back in for group changes to take effect
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

### Download Binary (Recommended)

1. Download the latest release for x86_64 Linux from [GitHub Releases](https://github.com/sevos/waystt/releases)

2. Install to your local bin directory:
```bash
# Download and install
wget https://github.com/sevos/waystt/releases/download/v0.1.0/waystt-linux-x86_64
mkdir -p ~/.local/bin
mv waystt-linux-x86_64 ~/.local/bin/waystt
chmod +x ~/.local/bin/waystt
```

3. Ensure `~/.local/bin` is in your PATH:
```bash
# Add to your shell profile (~/.bashrc, ~/.zshrc, etc.)
export PATH="$HOME/.local/bin:$PATH"
# Then reload your shell or run:
source ~/.bashrc  # or ~/.zshrc
```

### Building from Source

1. Clone the repository:
```bash
git clone https://github.com/sevos/waystt.git
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
# If installed to ~/.local/bin
nohup waystt > /tmp/waystt.log 2>&1 & disown

# If built from source
nohup ./target/release/waystt > /tmp/waystt.log 2>&1 & disown
```

### Signal-Based Transcription

Once running, waystt continuously records audio and waits for signals:

- **SIGUSR1**: Stop recording, transcribe audio, and type text directly (using ydotool)
- **SIGUSR2**: Stop recording, transcribe audio, and copy text to clipboard

Send signals using:
```bash
# For direct text typing
pkill --signal SIGUSR1 waystt

# For clipboard copy
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
# waystt - Speech to Text (direct typing)
bind = SUPER, R, exec, pgrep -x waystt >/dev/null && pkill -USR1 waystt || ~/.local/bin/waystt &

# waystt - Speech to Text (clipboard copy)  
bind = SUPER SHIFT, R, exec, pgrep -x waystt >/dev/null && pkill -USR2 waystt || ~/.local/bin/waystt &
```

These keybindings will:
- **Super+R**: Start waystt if not running, or send SIGUSR1 to transcribe and type directly
- **Super+Shift+R**: Start waystt if not running, or send SIGUSR2 to transcribe and copy to clipboard

### Niri

Add to your `~/.config/niri/config.kdl`:

```kdl
binds {
    // waystt - Speech to Text (direct typing)
    Mod+R { spawn "sh" "-c" "pgrep -x waystt >/dev/null && pkill -USR1 waystt || ~/.local/bin/waystt &"; }
    
    // waystt - Speech to Text (clipboard copy)
    Mod+Shift+R { spawn "sh" "-c" "pgrep -x waystt >/dev/null && pkill -USR2 waystt || ~/.local/bin/waystt &"; }
}
```

## Configuration

The application reads configuration from `.env` file or environment variables:

```bash
# Required
OPENAI_API_KEY=your_api_key_here

# Audio Feedback (optional)
ENABLE_AUDIO_FEEDBACK=true
BEEP_VOLUME=0.1

# Audio Configuration (optional)
AUDIO_BUFFER_DURATION_SECONDS=300
AUDIO_SAMPLE_RATE=16000
AUDIO_CHANNELS=1

# Whisper Configuration (optional)
WHISPER_MODEL=whisper-1
WHISPER_LANGUAGE=auto
WHISPER_TIMEOUT_SECONDS=60
WHISPER_MAX_RETRIES=3

# Logging (optional)
RUST_LOG=info
```

### Configuration Options

**Required:**
- `OPENAI_API_KEY`: Your OpenAI API key

**Audio Feedback:**
- `ENABLE_AUDIO_FEEDBACK`: Enable/disable musical beep notifications (default: true)
- `BEEP_VOLUME`: Volume level for beeps, 0.0-1.0 (default: 0.1)

**Audio Recording:**
- `AUDIO_BUFFER_DURATION_SECONDS`: Maximum recording duration (default: 300)
- `AUDIO_SAMPLE_RATE`: Recording sample rate (default: 16000)
- `AUDIO_CHANNELS`: Number of audio channels (default: 1)

**Whisper API:**
- `WHISPER_MODEL`: OpenAI Whisper model to use (default: whisper-1)
- `WHISPER_LANGUAGE`: Language for transcription (default: auto)
- `WHISPER_TIMEOUT_SECONDS`: API timeout in seconds (default: 60)
- `WHISPER_MAX_RETRIES`: Number of retry attempts (default: 3)

**Logging:**
- `RUST_LOG`: Log level (default: info)

## Workflow

1. **Start Recording**: Launch waystt - it immediately begins recording audio with a "ding dong" sound
2. **Speak**: Talk into your microphone while recording indicator shows
3. **Trigger Transcription**: Press your configured keybind or send signal:
   - **Super+R** (SIGUSR1): Direct text typing - transcribed text appears immediately where cursor is
   - **Super+Shift+R** (SIGUSR2): Clipboard copy - transcribed text copied for manual pasting
4. **Audio Feedback**: Listen for completion sounds:
   - "Dong ding" when recording stops
   - "Ding ding" when transcription succeeds and text is typed/copied
5. **Result**: Text is either typed directly or available via Ctrl+V

## Troubleshooting

### Audio Issues

If audio recording fails:
- Ensure PipeWire is running: `systemctl --user status pipewire`
- Check microphone permissions
- Verify microphone is not muted

### Text Input Issues

If direct text typing (SIGUSR1) fails:
- Ensure ydotool is installed and user is in input group
- Check ydotool permissions: `sudo usermod -a -G input $USER` (requires re-login)
- Verify ydotool daemon is running: `systemctl --user status ydotool`

### Clipboard Issues

If clipboard operations (SIGUSR2) fail:
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

This project is licensed under the GNU General Public License v3.0 or later (GPL-3.0-or-later).

**Source Code Availability**: As required by the GPL v3 license, the complete source code for this software is available at https://github.com/sevos/waystt. Anyone who distributes this software must also provide access to the corresponding source code.

**License Summary**:
- ✅ Commercial use allowed
- ✅ Modification allowed
- ✅ Distribution allowed
- ✅ Private use allowed
- ❗ **Copyleft**: Derivative works must also be GPL v3
- ❗ **Source disclosure**: Must provide source code when distributing
- ❗ **Same license**: Derivatives must use GPL v3 or compatible license

For the full license text, see the [LICENSE](LICENSE) file.