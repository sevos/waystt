## Project Overview

waystt is a Wayland speech-to-text tool with dual output modes:
- **SIGUSR1**: Direct text typing via ydotool (immediate insertion at cursor)
- **SIGUSR2**: Clipboard copy with persistent daemon for manual pasting

## Installation and Configuration

### Installation
Install the binary to your local bin directory:
```bash
cargo build --release
mkdir -p ~/.local/bin
cp ./target/release/waystt ~/.local/bin/
```

Ensure `~/.local/bin` is in your PATH:
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Configuration Setup
Create your configuration directory and file:
```bash
mkdir -p ~/.config/waystt
cp .env.example ~/.config/waystt/.env
```

Edit `~/.config/waystt/.env` with your settings:
- `OPENAI_API_KEY=sk-your-key` - Required for transcription
- `ENABLE_AUDIO_FEEDBACK=true/false` - Enable/disable beeps
- `BEEP_VOLUME=0.0-1.0` - Volume control (default: 0.1)
- Other optional settings (see `.env.example` for full list)

## Audio Feedback System

The app provides musical beep patterns for user feedback:
- **Recording Start**: "Ding dong" (C4 → E4) - plays before recording begins
- **Recording Stop**: "Dong ding" (E4 → C4) - plays after recording stops
- **Success**: "Ding ding" (E4 → E4) - plays after successful typing/clipboard operation
- **Error**: Warbling tone for failures

## QA Testing Workflow

- For QAing, run the app with `nohup` and `&` to properly detach from terminal:
  ```bash
  cd ~/.config/waystt && nohup ~/.local/bin/waystt > /tmp/waystt.log 2>&1 & disown
  ```
- Then:
  - Listen for "ding dong" sound confirming recording started
  - Ask the user to speak something
  - Wait 5 seconds
  - Run `pkill --signal SIGUSR1 waystt` to trigger transcription and direct typing
  - OR run `pkill --signal SIGUSR2 waystt` to trigger transcription and clipboard copy
  - Listen for "dong ding" (recording stopped) then "ding ding" (success) sounds
  - Check logs with `tail /tmp/waystt.log`
- Future improvement: Ask user to press RETURN, as their focus will likely be on the Claude Code terminal, which will send the transcribed text to the agent

## System Dependencies

Required packages:
- **pipewire**: Audio recording system
- **ydotool**: Direct text input (SIGUSR1 mode)
- **wtype**: Clipboard operations (SIGUSR2 mode)

Installation varies by distro:
```bash
# Arch Linux
sudo pacman -S pipewire pipewire-pulse pipewire-alsa ydotool wtype

# Ubuntu/Debian  
sudo apt install pipewire pipewire-pulse pipewire-alsa ydotool wtype

# Fedora
sudo dnf install pipewire pipewire-pulseaudio pipewire-alsa ydotool wtype
```

Post-installation for ydotool:
```bash
sudo usermod -a -G input $USER  # Requires re-login
```

## Keybinding Setup

For proper process detection in keybindings, use `pgrep -x waystt` to avoid matching the clipboard daemon:

```bash
# Direct typing mode (SIGUSR1)
bindkey "Super+R" "pgrep -x waystt >/dev/null && pkill -USR1 waystt || (cd ~/.config/waystt && ~/.local/bin/waystt &)"

# Clipboard mode (SIGUSR2)  
bindkey "Super+Shift+R" "pgrep -x waystt >/dev/null && pkill -USR2 waystt || (cd ~/.config/waystt && ~/.local/bin/waystt &)"
```

The `cd ~/.config/waystt` ensures the application finds your `.env` configuration file, while `~/.local/bin/waystt` points to the installed binary.

The clipboard daemon renames itself to `waystt-clipboard-daemon` to prevent interference with main process detection.

## Configuration Files

Key files for future development:
- `src/main.rs`: Main application logic, signal handling, audio feedback integration
- `src/beep.rs`: Musical audio feedback system with CPAL
- `src/audio.rs`: Audio recording via PipeWire/CPAL
- `src/clipboard.rs`: Clipboard operations and ydotool integration
- `src/config.rs`: Environment variable configuration
- `src/whisper.rs`: OpenAI Whisper API client
- `.env.example`: Configuration template

## Architecture Notes

- Uses signal-driven workflow (SIGUSR1/SIGUSR2) for triggering transcription
- Background audio recording with signal-triggered processing
- Dual output modes: direct typing vs clipboard
- Musical audio feedback for user experience
- Error handling with graceful fallbacks
- Comprehensive test coverage (95+ tests)