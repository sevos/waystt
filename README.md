# waystt - Wayland Speech-to-Text Tool

Press a keybind, speak, and get instant text output. A background speech-to-text tool that transcribes audio using OpenAI Whisper and either types directly or copies to clipboard.

## Features

- **Signal-driven**: Press keybind → speak → get text (no GUI needed)
- **Dual output modes**: Direct typing or clipboard copy
- **Background operation**: Runs continuously, always ready
- **Audio feedback**: Beeps confirm recording start/stop and success
- **Wayland native**: Works with modern Linux desktops (Hyprland, Niri, etc.)

## Requirements

- **Wayland desktop** (Hyprland, Niri, GNOME, KDE, etc.)
- **OpenAI API key** (for Whisper transcription)
- **System packages**:

```bash
# Arch Linux
sudo pacman -S pipewire ydotool wtype

# Ubuntu/Debian  
sudo apt install pipewire-pulse ydotool wtype

# Fedora
sudo dnf install pipewire-pulseaudio ydotool wtype
```

**Setup ydotool permissions:**
```bash
sudo usermod -a -G input $USER
# Log out and back in
```

## Installation

### Download Binary

1. Download from [GitHub Releases](https://github.com/sevos/waystt/releases)
2. Install:

```bash
wget https://github.com/sevos/waystt/releases/download/v0.1.0/waystt-linux-x86_64
mkdir -p ~/.local/bin
mv waystt-linux-x86_64 ~/.local/bin/waystt
chmod +x ~/.local/bin/waystt

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$HOME/.local/bin:$PATH"
```

## Quick Start

1. **Setup API key:**
```bash
# Create config file
echo "OPENAI_API_KEY=your_api_key_here" > ~/.config/waystt.env
```

2. **Start the service:**
```bash
nohup waystt > /tmp/waystt.log 2>&1 & disown
```

3. **Use with signals:**
```bash
# Direct typing mode
pkill --signal SIGUSR1 waystt

# Clipboard mode  
pkill --signal SIGUSR2 waystt
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

**Required:** Create `~/.config/waystt.env` with your OpenAI API key:

```bash
OPENAI_API_KEY=your_api_key_here
```

**Optional settings:**
```bash
# Disable audio beeps
ENABLE_AUDIO_FEEDBACK=false

# Change transcription language  
WHISPER_LANGUAGE=en

# Debug logging
RUST_LOG=debug
```


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

## Building from Source

```bash
git clone https://github.com/sevos/waystt.git
cd waystt
echo "OPENAI_API_KEY=your_key" > .env
cargo build --release
./target/release/waystt
```

## License

Licensed under GPL v3.0 or later. Source code: https://github.com/sevos/waystt

See [LICENSE](LICENSE) for full terms.