# Ticket 001: Simple Audio Recording System

## Title
Implement basic continuous audio recording with PipeWire

## Summary
Record audio continuously to growing buffer until signal received

## User Story
As a user, I want waystt to start recording audio immediately when launched so that I can capture speech for later transcription when I send a signal.

## Technical Considerations
- Integrate PipeWire via pipewire-rs crate for native Linux audio capture
- Use Vec<u8> or similar growing buffer (not circular buffer for MVP simplicity)
- Configure audio format optimized for Whisper API: 16kHz sample rate, mono channel
- Handle PipeWire connection errors and device discovery
- Implement basic memory management to prevent unbounded growth
- Start recording immediately on application launch
- Stop recording cleanly when signal received
- No real-time processing or optimization required at this stage

## Acceptance Criteria
- [ ] PipeWire integration successfully captures audio from default input device
- [ ] Audio is recorded continuously in 16kHz mono format suitable for Whisper
- [ ] Recording starts immediately when waystt launches
- [ ] Recording stops cleanly when SIGUSR1 or SIGUSR2 signal received
- [ ] Audio buffer is accessible for processing after recording stops
- [ ] Basic error handling for audio device unavailable/permission issues
- [ ] Memory usage remains reasonable for typical 30-60 second recordings
- [ ] No audio dropouts or corruption during recording

## Dependencies
- PipeWire development packages installed on system
- pipewire-rs crate added to Cargo.toml
- Basic signal handling framework (already implemented)

## Priority
Critical - MVP blocker