# ☎️ HotLine

**HotLine** is an open source, minimalist speech-to-text (STT) tool that channels the charm of classic telephony into a modern user experience powered by OpenAI’s streaming transcription.

## 🔊 Philosophy

**HotLine** is built on the idea that speech interfaces should be effortless, responsive, and maybe even a little fun.

- **Zero friction.** No windows, prompts, or configuration menus. You trigger it, you speak, it types.
- **Uncanny familiarity.** We grew up with phones that clicked, hummed, and beeped. HotLine borrows those auditory signals to communicate its state: on, off, listening, or error — all without cluttering your screen.
- **Free as in freedom.** HotLine is fully open source, made to be hacked, extended, and improved.
- **Focused on joy.** Computers should be delightful. HotLine takes a serious tool — speech recognition — and wraps it in a nostalgic, playful skin.

## 🎧 User Experience

When you activate HotLine, it plays a soft **off-hook tone**, just like picking up an old-school phone. That’s your cue: HotLine is now ready.

Tap the "start listening" signal? You’ll hear a crisp **beep**, like an answering machine — and you're live. Your speech starts flowing into the current text field as keystrokes, automatically and naturally.

Need to stop? The familiar **"beep-beep-beep"** of a dropped call lets you know it’s over.

And if something goes sideways — say, no API connection — you’ll hear the unmistakable **three-tone SIT error signal** you’ve heard a thousand times before.

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

`hotline` is configured using environment variables. It automatically loads them from `~/.config/hotline/.env`.

1.  **Create the configuration file**:
    ```bash
    mkdir -p ~/.config/hotline
    touch ~/.config/hotline/.env
    ```

2.  **Edit the file** and add your OpenAI API key:
    ```ini
    # Required: Your OpenAI API Key
    OPENAI_API_KEY=your_api_key_here

    # Optional: Specify the real-time model to use
    # Default: gpt-4o-mini-realtime-preview
    REALTIME_MODEL=gpt-4o-mini-realtime-preview

    # Optional: Set the language for transcription (default: auto-detect)
    # Use ISO 639-1 codes, e.g., "en", "es", "fr"
    WHISPER_LANGUAGE=en

    # Optional: Disable audio feedback beeps (default: true)
    ENABLE_AUDIO_FEEDBACK=false

    # Optional: Adjust beep volume (default: 0.1, range: 0.0 to 1.0)
    BEEP_VOLUME=0.05
    ```

## Usage

The core of `hotline` is a long-running process that you control with signals.

1.  **Start the `hotline` service** in a terminal or as a background process. This will immediately start recording audio.
    ```bash
    # Run in a terminal to see logs
    hotline

    # Or, run in the background
    hotline &
    ```
    You can optionally pipe the output to another command:
    ```bash
    # Pipe to wl-copy to automatically copy transcripts to the clipboard
    hotline --pipe-to wl-copy &

    # Pipe to ydotool to type the transcript directly (see ydotool setup below)
    hotline --pipe-to "ydotool type --file -" &
    ```

2.  **Control transcription with signals**:
    -   **Start Streaming (`SIGUSR1`)**: Send the `SIGUSR1` signal to `hotline` to begin streaming audio to OpenAI.
        ```bash
        pkill --signal SIGUSR1 hotline
        ```
    -   **Stop Streaming (`SIGUSR2`)**: Send the `SIGUSR2` signal to stop the stream. The application will continue recording in the background, ready for the next time you send `SIGUSR1`.
        ```bash
        pkill --signal SIGUSR2 hotline
        ```
    -   **Shutdown (`SIGTERM`)**: To stop the `hotline` application completely, send the `SIGTERM` signal.
        ```bash
        pkill --signal SIGTERM hotline
        ```

## Keybinding Examples

The most effective way to use `hotline` is to bind the `SIGUSR1` and `SIGUSR2` signals to hotkeys. Here is an example of a "push-to-talk" style keybinding.

**Keybinding Logic**:
-   When you press and hold the key, it sends `SIGUSR1` to start streaming.
-   When you release the key, it sends `SIGUSR2` to stop streaming.

### Hyprland

Add this to your `~/.config/hypr/hyprland.conf`:

```ini
# Real-time speech-to-text with hotline
bind = SUPER, R, submap, speech
submap = speech
binde=, SUPER_L, exec, pkill --signal SIGUSR1 hotline
bindr=, SUPER_L, exec, pkill --signal SIGUSR2 hotline
bind=, escape, submap, reset
submap = reset
```
*Note: This uses the `SUPER` key itself to trigger speech. You can change `SUPER_L` to another key if you prefer.*

### Niri

Key release events are not directly supported in the same way. A toggle approach is more suitable.

Add to `~/.config/niri/config.kdl`:
```kdl
binds {
    // Toggle hotline streaming
    Mod+R {
        spawn "sh" "-c" "if pgrep -x hotline-streaming >/dev/null; then pkill --signal SIGUSR2 hotline && rm /tmp/hotline-streaming; else pkill --signal SIGUSR1 hotline && touch /tmp/hotline-streaming; fi";
    }
}
```
*This binding uses a temporary file to track the streaming state.*

### Example: Using `ydotool` for Direct Typing

If you want `hotline` to type your speech directly into any application, you need `ydotool`.

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
4.  **Start `hotline` with the correct pipe**:
    ```bash
    hotline --pipe-to "ydotool type --file -" &
    ```

## Troubleshooting

-   **No transcription**:
    -   Ensure `hotline` is running (`pgrep -x hotline`).
    -   Verify your `OPENAI_API_KEY` is correct and has credits.
    -   Check the `hotline` logs for error messages by running it in a terminal.
-   **Audio issues**:
    -   Ensure `pipewire` is running: `systemctl --user status pipewire`.
    -   Check that your microphone is not muted (`pavucontrol`, `alsamixer`).

## Development

-   **Build**: `cargo build --release`
-   **Run tests**: `BEEP_VOLUME=0.0 cargo test`
-   **Check formatting**: `cargo fmt --all -- --check`
-   **Run linter**: `cargo clippy --all-targets -- -D warnings`

---
*Licensed under GPL v3.0 or later. Source code: https://github.com/sevos/hotline*
