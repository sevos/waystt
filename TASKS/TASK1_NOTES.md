# Task 1 Notes

## Important: Transcription Model Configuration

The OpenAI Real-time Transcription API requires specific models for transcription (not conversation models).

### Supported Transcription Models:
- `whisper-1` - The standard Whisper model
- `gpt-4o-transcribe` - GPT-4o optimized for transcription
- `gpt-4o-mini-transcribe` - GPT-4o mini optimized for transcription

### Issue Fixed:
The initial implementation was using `gpt-4o-mini-realtime-preview` which is for the conversation API, not transcription. This has been corrected to use `whisper-1` as the default.

### Configuration:
The model can be configured via:
1. The `REALTIME_MODEL` environment variable
2. The `realtime_model` field in the TOML configuration file
3. The `model` field in transcription profiles

## API Format Clarification

When connecting with `intent=transcription`:
- Use `transcription_session.update` (not `session.update`)
- **IMPORTANT**: Despite what the documentation shows, the API requires configuration to be wrapped in a `session` object
- The correct format is:
  ```json
  {
    "type": "transcription_session.update",
    "session": {
      "input_audio_format": "pcm16",
      "input_audio_transcription": {...},
      "turn_detection": {...}
    }
  }
  ```
- Events may use either `transcription_session.*` or `session.*` prefixes

This appears to be an inconsistency between the OpenAI documentation examples and the actual API implementation. The API requires the `session` wrapper even though the documentation examples show the configuration fields directly in the message.