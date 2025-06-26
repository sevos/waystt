# Ticket 004: Transcription Provider Abstraction

## Title
Modularize transcription system with pluggable provider architecture

## Summary
Refactor the current OpenAI Whisper-only transcription system into a modular architecture that supports multiple speech-to-text providers (OpenAI, Google Speech-to-Text, etc.) selectable via environment variables.

## User Story
As a user, I want to be able to choose between different speech-to-text providers (OpenAI Whisper, Google Speech-to-Text, etc.) using environment variables so that I can use the provider that works best for my language, accuracy needs, and cost preferences.

## Technical Considerations
- Create a `TranscriptionProvider` trait that abstracts the transcription interface
- Implement a factory pattern to instantiate providers based on configuration
- Maintain backward compatibility with existing OpenAI Whisper configuration
- Create unified error handling across all providers
- Support provider-specific configuration while maintaining a consistent interface
- Ensure proper async/await support across all providers
- Add comprehensive test coverage for the abstraction layer

## Acceptance Criteria
- [ ] Create `TranscriptionProvider` trait with standardized `transcribe()` method signature
- [ ] Implement `TranscriptionFactory` that creates providers based on `TRANSCRIPTION_PROVIDER` env var
- [ ] Refactor existing OpenAI implementation to use the new trait
- [ ] Add Google Speech-to-Text provider implementation
- [ ] Maintain 100% backward compatibility with existing OpenAI configuration
- [ ] Add `TRANSCRIPTION_PROVIDER` environment variable (default: "openai")
- [ ] Create unified `TranscriptionError` enum for consistent error handling
- [ ] Update configuration module to support provider-specific settings
- [ ] Write comprehensive unit tests for trait implementations
- [ ] Write integration tests for provider switching
- [ ] Update documentation with new provider configuration options
- [ ] Ensure all existing functionality continues to work unchanged

## Implementation Plan (TDD)

### Phase 1: Create Abstraction Layer
1. Create `src/transcription/mod.rs` with trait definition
2. Write tests for trait interface and factory pattern
3. Implement `TranscriptionProvider` trait and `TranscriptionFactory`

### Phase 2: Refactor OpenAI Implementation  
1. Move `src/whisper.rs` to `src/transcription/openai.rs`
2. Update existing tests to work with trait-based approach
3. Implement `TranscriptionProvider` trait for OpenAI

### Phase 3: Add Google Provider
1. Write tests for Google Speech-to-Text provider
2. Create `src/transcription/google.rs` 
3. Implement Google provider with `TranscriptionProvider` trait

### Phase 4: Update Configuration & Integration
1. Add provider selection to `config.rs`
2. Update `main.rs` to use factory pattern
3. Add integration tests for provider switching

## Environment Variables
- `TRANSCRIPTION_PROVIDER=openai|google` (default: openai)
- Keep all existing OpenAI configuration variables unchanged
- Add Google-specific configuration variables:
  - `GOOGLE_APPLICATION_CREDENTIALS` (path to service account JSON)
  - `GOOGLE_SPEECH_LANGUAGE_CODE` (default: "en-US")
  - `GOOGLE_SPEECH_MODEL` (default: "latest_long")

## Dependencies
- Ticket 003 (OpenAI Whisper Integration) completed
- Google Cloud Speech-to-Text client library
- Maintain existing reqwest dependency for OpenAI

## Priority
High - Foundation for future provider additions

## Benefits
- Users can choose optimal provider for their use case
- Easy to add new providers in the future
- Maintains backward compatibility
- Unified error handling and configuration
- Better testability with trait-based architecture