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

[profiles.coding-spanish.vad_config.SemanticVad]
eagerness = "medium"

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

# Toggle transcription (start if stopped, stop if running)
hotline toggle-transcription coding
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
    "hooks": {
      "on_transcription_receive": {
        "type": "spawn_with_stdin",
        "command": ["wl-copy"]
      }
    }
  }
}' | hotline sendcmd

# Stop transcription
echo '{"StopTranscription": null}' | hotline sendcmd
```

### Lifecycle Hooks

HotLine supports hooks that execute commands at different stages of the transcription lifecycle:

- **on_transcription_start**: Executed when transcription begins
- **on_transcription_receive**: Executed when transcription text is received (text piped to stdin)
- **on_transcription_stop**: Executed when transcription ends

Each hook supports different command types:
- **spawn**: Execute command without stdin (for start/stop hooks)
- **spawn_with_stdin**: Execute command with text piped to stdin (for receive hook)

Example configuration with all hooks:
```toml
[profiles.example.hooks.on_transcription_start]
type = "spawn"
command = ["notify-send", "Recording started"]

[profiles.example.hooks.on_transcription_receive]
type = "spawn_with_stdin"
command = ["wl-copy"]  # Copy to clipboard

[profiles.example.hooks.on_transcription_stop]
type = "spawn"
command = ["notify-send", "Recording stopped"]
```

Common use cases:
- Notifications when recording starts/stops
- Copying text to clipboard
- Typing text directly
- Logging to files with timestamps
- Triggering webhooks or external services

## Voice Activity Detection (VAD)

HotLine supports two VAD modes:

### Server VAD (Default)
Traditional threshold-based voice detection (TOML configuration):
```toml
[profiles.example.vad_config.ServerVad]
threshold = 0.5
prefix_padding_ms = 300
silence_duration_ms = 500
```

### Semantic VAD
AI-powered context-aware voice detection (TOML configuration):
```toml
[profiles.example.vad_config.SemanticVad]
eagerness = "medium"  # Options: "low", "medium", "high"
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
    
    [profiles.typing.hooks.on_transcription_receive]
    type = "spawn_with_stdin"
    command = ["ydotool", "type", "--file", "-"]
    ```

## Home Manager Configuration (NixOS)

For NixOS users, HotLine can be configured declaratively using Home Manager:

```nix
{
  programs.hotline = {
    enable = true;
    
    # Enable systemd service with environment variables
    systemdService = {
      enable = true;
      environmentFile = "/path/to/.env";  # Contains OPENAI_API_KEY
      environment = {
        # Required for ydotool integration
        YDOTOOL_SOCKET = "/run/user/1000/.ydotool_socket";
      };
    };

    settings = {
      realtime_model = "whisper-1";
      audio_buffer_duration_seconds = 300;
      audio_sample_rate = 16000;
      audio_channels = 1;
      whisper_language = "auto";
      enable_audio_feedback = true;
      beep_volume = 0.1;
      rust_log = "info";
      
      profiles = {
        coding = {
          model = "gpt-4o-mini-transcribe";
          language = "en";
          prompt = "The user is a programmer, so expect technical terms.";
          
          # AI-powered voice detection for better accuracy
          vad_config = {
            SemanticVad = {
              eagerness = "medium";
            };
          };
          
          # Type transcribed text directly and add newline
          hooks = {
            on_transcription_receive = {
              type = "spawn_with_stdin";
              command = [ "ydotool" "type" "--file" "-" ];
            };
            on_transcription_stop = {
              type = "spawn";
              command = [ "ydotool" "key" "28:1" "28:0" ];  # Press Enter
            };
          };
        };
      };
    };
  };
}
```

**Key features shown:**
- **Environment file**: Secure API key storage using `environmentFile`
- **Environment variables**: Pass `YDOTOOL_SOCKET` for tool integration
- **Semantic VAD**: AI-powered voice activity detection for coding contexts
- **Direct typing**: Use `ydotool` to type transcribed text into any application
- **Automatic newline**: Press Enter after transcription stops

**Required setup for ydotool integration:**
```bash
# Install and enable ydotool service
sudo usermod -a -G input $USER
systemctl --user enable --now ydotool.service
```

## Keybinding Integration

The `toggle-transcription` command is perfect for keybindings in your window manager:

### Sway/i3 Example
```bash
# Add to ~/.config/sway/config or ~/.config/i3/config
bindsym $mod+t exec hotline toggle-transcription coding
bindsym $mod+Shift+t exec hotline toggle-transcription meeting
```

### GNOME Example
```bash
# Set custom keybinding via command line
gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/']"
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ name 'Toggle Transcription'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ command 'hotline toggle-transcription coding'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ binding '<Super>t'
```

### KDE Plasma Example
1. System Settings → Shortcuts → Custom Shortcuts
2. Edit → New → Global Shortcut → Command/URL
3. Set trigger to your preferred key combination
4. Set action to: `hotline toggle-transcription coding`

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
*Source code: https://github.com/nilp0inter/hotline*

## Fork Information

This project is a heavily modified fork of [sevos/waystt](https://github.com/sevos/waystt). While the core concept and inspiration are credited to the original author, this version has been almost completely rewritten to modernize the architecture, expand features, and improve the overall user experience.
