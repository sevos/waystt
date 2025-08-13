### 1. Switch to the Correct OpenAI Real-time API for Transcription

**Description:**

The current implementation is using OpenAI's Real-time API for conversations, but it should be using the API specifically designed for real-time transcription. This task involves updating the WebSocket connection and session management to align with the correct API specifications.

**Documentation:**

Refer to the `openai_docs` directory for detailed documentation on the Real-time Transcription API.

**Key Changes:**

1.  **Update WebSocket URL:**
    -   In `src/transcription/realtime.rs`, change the WebSocket connection URL from `wss://api.openai.com/v1/realtime?model={model}` to `wss://api.openai.com/v1/realtime?intent=transcription`.

2.  **Modify Session Configuration:**
    -   The session configuration should use a `transcription_session.update` event instead of `session.update`.
    -   The configuration object should be updated to use transcription-specific properties. For example:

        ```json
        {
          "type": "transcription_session.update",
          "input_audio_format": "pcm16",
          "input_audio_transcription": {
            "model": "whisper-1",
            "language": "en"
          }
        }
        ```

3.  **Handle Transcription-Specific Events:**
    -   Update the event handling logic to process `conversation.item.input_audio_transcription.delta` and `conversation.item.input_audio_transcription.completed` events.
    -   Remove the logic for handling conversation-specific events like `response.text.delta` and `response.text.done`.

4.  **Review VAD (Voice Activity Detection) Logic:**
    -   The current VAD logic is tailored for the conversation API. Review the `openai_docs/vad.md` documentation and adjust the implementation as needed for the transcription API.

**Acceptance Criteria:**

-   The application successfully connects to the OpenAI Real-time Transcription API.
-   Audio is streamed to the API, and transcriptions are received in real-time.
-   The application correctly handles transcription-specific events and no longer processes conversation-related events.
