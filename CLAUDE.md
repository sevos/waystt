## Project Overview

waystt is a Wayland speech-to-text tool with dual output modes:
- **SIGUSR1**: Direct text typing via ydotool (immediate insertion at cursor)
- **SIGUSR2**: Clipboard copy with persistent daemon for manual pasting

## Audio Feedback System

Configuration:
- `ENABLE_AUDIO_FEEDBACK=true/false` - Enable/disable beeps
- `BEEP_VOLUME=0.0-1.0` - Volume control (default: 0.1)

## Testing
- Always set the beep volume to 0, when running tests `BEEP_VOLUME=0.0 cargo test...`
- When developing/testing, use `--envfile .env` to use the project-local .env file instead of ~/.config/waystt/.env
- Example: `BEEP_VOLUME=0.0 cargo run -- --envfile .env`

## QA Testing Workflow

- For QAing, run the app with `nohup` and `&` to properly detach from terminal:
  ```bash
  # Using production config (~/.config/waystt/.env)
  nohup ./target/release/waystt > /tmp/waystt.log 2>&1 & disown
  
  # Or during development using project-local .env file
  nohup ./target/release/waystt --envfile .env > /tmp/waystt.log 2>&1 & disown
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

## Configuration Files

Key files for future development:
- `src/main.rs`: Main application logic, signal handling, audio feedback integration
- `src/beep.rs`: Musical audio feedback system with CPAL
- `src/audio.rs`: Audio recording via PipeWire/CPAL
- `src/clipboard.rs`: Clipboard operations and ydotool integration
- `src/config.rs`: Environment variable configuration
- `src/whisper.rs`: OpenAI Whisper API client
- `.env.example`: Configuration template