
use super::TranscriptionError;
use bytes::Bytes;
use futures::stream::Stream;
use reqwest::Body;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

/// A stream that can be fed audio data while the HTTP request is in progress
pub struct AudioStream {
    receiver: mpsc::Receiver<Result<Bytes, std::io::Error>>,
}

impl AudioStream {
    pub fn new(receiver: mpsc::Receiver<Result<Bytes, std::io::Error>>) -> Self {
        Self { receiver }
    }
}

impl Stream for AudioStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.receiver.poll_recv(cx) {
            Poll::Ready(Some(data)) => Poll::Ready(Some(data)),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Handles streaming transcription to OpenAI
pub struct StreamingTranscriber {
    api_key: String,
    base_url: String,
    model: String,
}

impl StreamingTranscriber {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key,
            base_url,
            model,
        }
    }

    /// Start a streaming transcription session
    /// Returns a sender for audio data and a future that resolves to the transcription
    pub async fn start_streaming(
        &self,
        language: Option<String>,
    ) -> Result<
        (
            mpsc::Sender<Vec<u8>>,
            tokio::task::JoinHandle<Result<String, TranscriptionError>>,
        ),
        TranscriptionError,
    > {
        let (audio_tx, mut audio_rx) = mpsc::channel::<Vec<u8>>(100);
        let (stream_tx, stream_rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(100);

        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();
        let model = self.model.clone();

        // Task to forward audio chunks to the stream
        tokio::spawn(async move {
            while let Some(chunk) = audio_rx.recv().await {
                // Send each chunk as-is to the stream
                if stream_tx.send(Ok(Bytes::from(chunk))).await.is_err() {
                    break;
                }
            }
            // Signal end of stream
            drop(stream_tx);
        });

        // Create the multipart boundary
        let boundary = format!("----WebKitFormBoundary{}", uuid::Uuid::new_v4().simple());

        // Build the multipart body manually
        let mut body_parts = Vec::new();

        // Add model field
        body_parts.push(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{}\r\n",
            boundary, model
        ));

        // Add language field if specified
        if let Some(lang) = language {
            body_parts.push(format!(
                "--{}\r\nContent-Disposition: form-data; name=\"language\"\r\n\r\n{}\r\n",
                boundary, lang
            ));
        }

        // Add file field header
        body_parts.push(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n",
            boundary
        ));

        let header = body_parts.join("");
        let footer = format!("\r\n--{}--\r\n", boundary);

        // Create a new channel for the complete multipart stream
        let (multipart_tx, multipart_rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(100);

        // Task to build the complete multipart stream
        let _multipart_task = tokio::spawn(async move {
            // Send header
            let _ = multipart_tx.send(Ok(Bytes::from(header))).await;

            // Forward audio stream
            let mut stream_rx = stream_rx;
            while let Some(chunk) = stream_rx.recv().await {
                if multipart_tx.send(chunk).await.is_err() {
                    break;
                }
            }

            // Send footer
            let _ = multipart_tx.send(Ok(Bytes::from(footer))).await;
        });

        // Create the streaming body
        let audio_stream = AudioStream::new(multipart_rx);
        let body = Body::wrap_stream(audio_stream);

        // Start the HTTP request
        let url = format!("{}/audio/transcriptions", base_url);
        let client = reqwest::Client::new();

        let request_task = tokio::spawn(async move {
            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header(
                    "Content-Type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(body)
                .send()
                .await
                .map_err(|e| {
                    TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                        provider: "OpenAI".to_string(),
                        error_type: "Request failed".to_string(),
                        error_message: e.to_string(),
                    })
                })?;

            let status = response.status();
            let response_text = response.text().await.map_err(|e| {
                TranscriptionError::NetworkError(crate::transcription::NetworkErrorDetails {
                    provider: "OpenAI".to_string(),
                    error_type: "Response reading error".to_string(),
                    error_message: e.to_string(),
                })
            })?;

            if status.is_success() {
                // Parse the JSON response
                let json: serde_json::Value =
                    serde_json::from_str(&response_text).map_err(|e| {
                        TranscriptionError::JsonError(format!("Failed to parse response: {}", e))
                    })?;

                let text = json["text"]
                    .as_str()
                    .ok_or_else(|| {
                        TranscriptionError::JsonError(
                            "Missing 'text' field in response".to_string(),
                        )
                    })?
                    .to_string();

                Ok(text)
            } else {
                Err(TranscriptionError::ApiError(
                    crate::transcription::ApiErrorDetails {
                        provider: "OpenAI".to_string(),
                        status_code: Some(status.as_u16()),
                        error_code: None,
                        error_message: response_text.clone(),

                    },
                ))
            }
        });

        Ok((audio_tx, request_task))
    }
}
