# Ticket 003: OpenAI Whisper Integration

## Title
Implement OpenAI Whisper API client with multipart upload and response handling

## Summary
Send processed audio to OpenAI Whisper API and handle transcription response

## User Story
As a user, I want waystt to send my recorded audio to OpenAI Whisper API and receive accurate transcription text so that I can have my speech converted to text for pasting or copying.

## Technical Considerations
- Implement HTTP client using reqwest for OpenAI Whisper API
- Handle multipart form data upload for audio files
- Manage OpenAI API authentication via environment variable (OPENAI_API_KEY)
- Implement proper error handling for network issues, API errors, quota limits
- Add basic retry logic with exponential backoff for transient failures
- Parse JSON response and extract transcribed text
- Handle different API response scenarios (success, error, rate limiting)
- Stream audio data directly from memory buffer (no temporary files)
- Prepare transcription result for clipboard/paste operations
- Implement timeout handling for API requests

## Acceptance Criteria
- [ ] Successfully authenticate with OpenAI API using OPENAI_API_KEY environment variable
- [ ] Upload audio buffer as multipart form data to Whisper API endpoint
- [ ] Handle successful API responses and extract transcribed text
- [ ] Implement proper error handling for common API failures (auth, quota, network)
- [ ] Add retry logic for transient failures (network timeouts, 5xx errors)
- [ ] Handle rate limiting with appropriate backoff
- [ ] Parse and validate JSON responses from API
- [ ] Provide clear error messages for different failure scenarios
- [ ] Support direct memory-to-API streaming without temporary files
- [ ] Return clean, formatted transcription text ready for output
- [ ] Implement reasonable timeout values for API requests (30-60 seconds)

## Dependencies
- Ticket 002 (Signal Processing & Audio Preparation) completed
- reqwest crate already in Cargo.toml with multipart support
- OpenAI API account and key for testing
- Processed audio buffer in correct format

## Priority
Critical - MVP blocker