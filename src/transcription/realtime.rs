use super::TranscriptionError;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        handshake::client::{generate_key, Request},
        Message,
    },
};

#[derive(Debug, Serialize, Deserialize)]
struct RealtimeEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<serde_json::Value>,
}

/// Handles real-time streaming transcription via OpenAI's WebSocket API
pub struct RealtimeTranscriber {
    api_key: String,
    model: String,
}

impl RealtimeTranscriber {
    pub fn with_model(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }

    /// Start a real-time transcription session
    /// Returns a sender for audio data and a receiver for transcription results
    pub async fn start_session(
        &self,
        _language: Option<String>,
    ) -> Result<
        (
            mpsc::Sender<Vec<u8>>,                  // Send PCM16 audio data
            mpsc::Receiver<Result<String, String>>, // Receive transcriptions or errors
            tokio::task::JoinHandle<()>,            // WebSocket task handle
        ),
        TranscriptionError,
    > {
        let url = format!("wss://api.openai.com/v1/realtime?model={}", self.model);

        // Parse the URL for tokio-tungstenite
        let url = url
            .parse::<tokio_tungstenite::tungstenite::http::Uri>()
            .map_err(|e| {
                TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                    provider: "OpenAI Realtime".to_string(),
                    error_type: "Invalid URL".to_string(),
                    error_message: e.to_string(),
                })
            })?;

        // Create request with authentication header
        let request = Request::builder()
            .method("GET")
            .uri(url)
            .header("Host", "api.openai.com")
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Key", generate_key())
            .header("Sec-WebSocket-Version", "13")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("OpenAI-Beta", "realtime=v1")
            .body(())
            .map_err(|e| {
                TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                    provider: "OpenAI Realtime".to_string(),
                    error_type: "WebSocket connection error".to_string(),
                    error_message: e.to_string(),
                })
            })?;

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(request).await.map_err(|e| {
            TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                provider: "OpenAI Realtime".to_string(),
                error_type: "WebSocket connection failed".to_string(),
                error_message: e.to_string(),
            })
        })?;

        eprintln!("Connected to OpenAI Realtime API");

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Channels for audio input and transcription output
        let (audio_tx, mut audio_rx) = mpsc::channel::<Vec<u8>>(100);
        let (transcript_tx, transcript_rx) = mpsc::channel::<Result<String, String>>(100);

        // Configure session for transcription only (not conversation)
        let session_config = json!({
            "type": "session.update",
            "session": {
                "modalities": ["text"],
                "instructions": "You must ONLY respond with the word 'OK' to any input. Never say anything else. Just 'OK'. Nothing more, nothing less. Only 'OK'.",
                "input_audio_format": "pcm16",
                "input_audio_transcription": {
                    "model": "whisper-1"
                },
                "turn_detection": {
                    "type": "server_vad",
                    "threshold": 0.5,
                    "prefix_padding_ms": 300,
                    "silence_duration_ms": 200  // Reduced for faster response
                },
                "temperature": 0.6,
                "max_response_output_tokens": 2  // Just enough for "OK"
            }
        });

        // Send session configuration
        ws_sender
            .send(Message::text(session_config.to_string()))
            .await
            .map_err(|e| {
                TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                    provider: "OpenAI Realtime".to_string(),
                    error_type: "Failed to configure session".to_string(),
                    error_message: e.to_string(),
                })
            })?;

        // WebSocket task to handle bidirectional communication
        let ws_task = tokio::spawn(async move {
            let ws_sender = Arc::new(Mutex::new(ws_sender));
            let ws_sender_clone = ws_sender.clone();

            // Task to send audio data to WebSocket
            let audio_task = tokio::spawn(async move {
                while let Some(audio_data) = audio_rx.recv().await {
                    // Convert PCM16 audio to base64
                    let audio_base64 = BASE64.encode(&audio_data);

                    // Create audio append event
                    let audio_event = json!({
                        "type": "input_audio_buffer.append",
                        "audio": audio_base64
                    });

                    // Send to WebSocket
                    {
                        let mut sender = ws_sender_clone.lock().await;
                        if sender
                            .send(Message::text(audio_event.to_string()))
                            .await
                            .is_err()
                        {
                            eprintln!("Failed to send audio to WebSocket");
                            break;
                        }
                    }

                    // With VAD mode, we don't need to manually commit
                    // The server will automatically detect speech and commit
                }

                // Don't send final commit with VAD mode - it handles it automatically
            });

            // Task to receive events from WebSocket
            let receive_task = tokio::spawn(async move {
                while let Some(msg) = ws_receiver.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            // Log response events for debugging
                            if text.contains("response.text") {
                                eprintln!("Raw response event: {}", text);
                            }

                            // Parse the event
                            if let Ok(event) = serde_json::from_str::<RealtimeEvent>(&text) {
                                match event.event_type.as_str() {
                                    "conversation.item.input_audio_transcription.completed" => {
                                        // Extract final transcription from the event
                                        if let Some(item) = event.item {
                                            if let Some(transcript) =
                                                item.get("transcript").and_then(|t| t.as_str())
                                            {
                                                let _ = transcript_tx
                                                    .send(Ok(transcript.to_string()))
                                                    .await;
                                            }
                                        } else if let Some(transcript) = event.transcript {
                                            let _ = transcript_tx.send(Ok(transcript)).await;
                                        }
                                    }
                                    "conversation.item.input_audio_transcription.delta" => {
                                        // Partial transcriptions are ignored for now.
                                    }
                                    "conversation.item.input_audio_transcription.failed" => {
                                        let _ = transcript_tx
                                            .send(Err("Transcription failed".to_string()))
                                            .await;
                                    }
                                    "input_audio_buffer.speech_started" => {}
                                    "input_audio_buffer.speech_stopped" => {}
                                    "input_audio_buffer.committed" => {}
                                    "error" => {
                                        if let Some(error) = event.error {
                                            let error_msg = serde_json::to_string_pretty(&error)
                                                .unwrap_or_else(|_| error.to_string());
                                            eprintln!("Realtime API error: {}", error_msg);
                                            let _ = transcript_tx.send(Err(error_msg)).await;
                                        }
                                    }
                                    "session.created" => {
                                        eprintln!("WebSocket session established");
                                    }
                                    "session.updated" => {
                                        // Session configuration updated
                                    }
                                    // Log AI responses to verify system prompt is working
                                    "response.text.delta" => {
                                        eprintln!("AI response delta event");
                                        if let Some(delta) = event.delta {
                                            eprintln!("Delta content: {:?}", delta);
                                            if let Some(text) =
                                                delta.get("text").and_then(|t| t.as_str())
                                            {
                                                eprintln!("AI response (delta): {}", text);
                                            } else if let Some(text) = delta.as_str() {
                                                eprintln!("AI response (delta str): {}", text);
                                            }
                                        }
                                    }
                                    "response.text.done" => {
                                        eprintln!("AI response done event");
                                        if let Some(item) = event.item {
                                            eprintln!("Item content: {:?}", item);
                                            if let Some(text) =
                                                item.get("text").and_then(|t| t.as_str())
                                            {
                                                eprintln!("AI response (complete): {}", text);
                                            } else if let Some(content) = item.get("content") {
                                                eprintln!("Content field: {:?}", content);
                                            }
                                        }
                                    }
                                    "response.audio_transcript.delta"
                                    | "response.audio_transcript.done"
                                    | "response.created"
                                    | "response.done"
                                    | "response.content_part.added"
                                    | "response.content_part.done"
                                    | "response.output_item.added"
                                    | "response.output_item.done" => {
                                        // Ignore other response events
                                    }
                                    _ => {
                                        // Ignore other events
                                    }
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            eprintln!("WebSocket closed");
                            break;
                        }
                        Err(e) => {
                            eprintln!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            });

            // Wait for both tasks
            let _ = tokio::join!(audio_task, receive_task);
        });

        Ok((audio_tx, transcript_rx, ws_task))
    }
}
