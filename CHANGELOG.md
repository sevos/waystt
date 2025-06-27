# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2025-06-27

### Fixed
- AUR publishing workflow SSH key errors

## [0.1.2] - 2025-06-26

### Added
- AUR publishing workflow

## [0.1.1] - 2025-06-26

### Added
- Google Speech-to-Text provider support as alternative to OpenAI Whisper
- Transcription provider abstraction with configurable providers
- Comprehensive configuration options for Google Speech-to-Text:
  - Language code selection (GOOGLE_SPEECH_LANGUAGE_CODE)
  - Model selection (GOOGLE_SPEECH_MODEL) 
  - Alternative languages for auto-detection
- Updated documentation with detailed setup instructions for both providers

### Changed
- Enhanced README with clear configuration sections for OpenAI and Google providers
- Improved troubleshooting documentation for provider-specific issues

## [0.1.0] - 2025-06-25

### Added
- Initial release of waystt - Wayland Speech-to-Text Tool
- Signal-driven speech-to-text with dual output modes:
  - SIGUSR1: Direct text typing via ydotool 
  - SIGUSR2: Clipboard copy for manual pasting
- OpenAI Whisper API integration for high-quality transcription
- Continuous background audio recording with PipeWire/CPAL
- Musical audio feedback system with configurable beeps:
  - Recording start: "Ding dong" (C4 → E4)
  - Recording stop: "Dong ding" (E4 → C4)
  - Success: "Ding ding" (E4 → E4)
  - Error: Warbling tone for failures
- Persistent clipboard daemon for clipboard operations
- Comprehensive configuration via environment variables
- Support for multiple audio backends (PipeWire, PulseAudio, ALSA)
- Cross-platform Wayland compatibility (tested on Hyprland, Niri)
- Error handling with graceful fallbacks and retry mechanisms

### System Requirements
- **Audio System**: PipeWire (recommended) or PulseAudio/ALSA
- **Text Input**: ydotool for direct typing functionality
- **Clipboard**: wtype for clipboard operations
- **Environment**: Wayland display server
- **API**: OpenAI API key for Whisper transcription

### Configuration Options
- Audio feedback enable/disable and volume control
- Audio recording parameters (sample rate, channels, buffer duration)
- Whisper API settings (model, language, timeout, retries)
- Comprehensive logging configuration

### Keybinding Examples
- Hyprland and Niri configuration examples provided
- Process detection using `pgrep -x waystt` for reliable signal handling

### Dependencies
- tokio (async runtime)
- cpal (audio capture)
- reqwest (HTTP client for OpenAI API)
- signal-hook (Unix signal handling)
- wl-clipboard-rs (Wayland clipboard integration)
- Plus development and build dependencies

### License
- Released under GPL-3.0-or-later license