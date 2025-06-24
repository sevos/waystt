# Ticket 002: Signal Processing & Audio Preparation

## Title
Enhance signal handling and implement audio optimization for Whisper API

## Summary
Process signals to stop recording and optimize audio format for transcription

## User Story
As a user, when I send a SIGUSR1 or SIGUSR2 signal, I want waystt to immediately stop recording, optimize the captured audio, and prepare it for transcription so that I get the best possible speech recognition results.

## Technical Considerations
- Extend existing signal handling to cleanly stop audio recording
- Implement audio preprocessing for optimal Whisper API results
- Handle different signal types (SIGUSR1 vs SIGUSR2) for different output modes
- Optimize audio data format and encoding for API submission
- Implement basic audio quality improvements (silence trimming, normalization)
- Prepare audio buffer for direct API submission without temporary files
- Handle edge cases (empty recording, very short recordings, silence-only)
- Clean up audio resources and prepare for application exit

## Acceptance Criteria
- [ ] SIGUSR1 signal stops recording and prepares audio for transcribe+paste workflow
- [ ] SIGUSR2 signal stops recording and prepares audio for transcribe+copy workflow
- [ ] SIGTERM signal performs clean shutdown with proper resource cleanup
- [ ] Audio buffer is properly converted to WAV format for Whisper API
- [ ] Basic audio optimization: silence trimming at start/end
- [ ] Audio normalization to optimal levels for speech recognition
- [ ] Empty or too-short recordings are handled gracefully with user feedback
- [ ] Memory cleanup after audio processing to prevent leaks
- [ ] Processed audio is ready for direct HTTP multipart upload

## Dependencies
- Ticket 001 (Simple Audio Recording System) completed
- Audio processing utilities (may need additional crates for WAV encoding)
- Existing signal handling framework

## Priority
Critical - MVP blocker