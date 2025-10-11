use anyhow::Result;
use futures::stream::StreamExt;

use crate::audio::AudioRecorder;
use crate::beep::{BeepConfig, BeepPlayer, BeepType};
use crate::cli::RunOptions;
use crate::command;
use crate::config::Config;
use crate::pipeline::AudioPipeline;
use crate::signals;
use crate::transcription::{TranscriptionError, TranscriptionProvider};

pub struct App {
    config: Config,
    recorder: AudioRecorder,
    beeps: BeepPlayer,
    pipeline: AudioPipeline,
    provider: Box<dyn TranscriptionProvider>,
    pipe_to: Option<Vec<String>>,
}

impl App {
    /// Initialize the application
    ///
    /// # Errors
    ///
    /// Returns an error if audio devices cannot be initialized or configured
    #[allow(clippy::unused_async)]
    pub async fn init(
        options: RunOptions,
        config: Config,
        provider: Box<dyn TranscriptionProvider>,
    ) -> Result<Self> {
        let beep_config = BeepConfig {
            enabled: config.enable_audio_feedback,
            volume: config.beep_volume,
        };
        let beeps = BeepPlayer::new(beep_config)?;
        let recorder = AudioRecorder::new()?;
        let pipeline = AudioPipeline::new(config.audio_sample_rate);

        Ok(Self {
            config,
            recorder,
            beeps,
            pipeline,
            provider,
            pipe_to: options.pipe_to,
        })
    }

    /// Run the application main loop
    ///
    /// # Errors
    ///
    /// Returns an error if signal handling fails or audio recording cannot be started
    pub async fn run(mut self) -> Result<i32> {
        eprintln!("waystt - Wayland Speech-to-Text Tool");
        eprintln!("Starting audio recording...");

        if let Err(e) = self.beeps.play_async(BeepType::RecordingStart).await {
            eprintln!("Warning: Failed to play recording start beep: {e}");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

        if let Err(e) = self.recorder.start_recording() {
            eprintln!("Failed to start recording: {e}");
            return Ok(1);
        }

        // Signals
        let mut signals = signals::build_signal_stream()?;

        loop {
            // Drive background audio events
            if let Err(e) = self.recorder.process_audio_events() {
                eprintln!("Audio event processing error: {e}");
            }

            // Poll signals with timeout to keep loop responsive
            match tokio::time::timeout(tokio::time::Duration::from_millis(100), signals.next())
                .await
            {
                Ok(Some(signal)) => match signal {
                    s if s == signals::TRANSCRIBE_SIG => {
                        // Stop recording for processing
                        if let Err(e) = self.recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {e}");
                        }
                        // Play stop beep to signal end of capture
                        if let Err(e) = self.beeps.play_async(BeepType::RecordingStop).await {
                            eprintln!("Warning: Failed to play recording stop beep: {e}");
                        }

                        let duration = self
                            .recorder
                            .get_recording_duration_seconds()
                            .unwrap_or_default();
                        eprintln!(
                            "Received SIGUSR1: Starting transcription for {duration:.2}s buffer"
                        );

                        let audio_data = match self.recorder.get_audio_data() {
                            Ok(d) => d,
                            Err(e) => {
                                eprintln!("Failed to get audio data: {e}");
                                return Ok(1);
                            }
                        };

                        let res = self.process_and_transcribe(audio_data).await;

                        // Clear buffer to free memory regardless of outcome
                        if let Err(e) = self.recorder.clear_buffer() {
                            eprintln!("Failed to clear audio buffer: {e}");
                        }

                        match res {
                            Ok(code) => return Ok(code),
                            Err(_) => return Ok(1),
                        }
                    }
                    s if s == signals::SHUTDOWN_SIG => {
                        eprintln!("Received SIGTERM: Shutting down gracefully");
                        if let Err(e) = self.recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {e}");
                        }
                        // Play stop beep on shutdown as well
                        if let Err(e) = self.beeps.play_async(BeepType::RecordingStop).await {
                            eprintln!("Warning: Failed to play recording stop beep: {e}");
                        }
                        if let Err(e) = self.recorder.clear_buffer() {
                            eprintln!("Failed to clear audio buffer during shutdown: {e}");
                        }
                        return Ok(0);
                    }
                    other => {
                        eprintln!("Received unexpected signal: {other}");
                    }
                },
                Ok(None) => break,  // stream ended
                Err(_) => {} // timeout
            }
        }

        eprintln!("Exiting waystt");
        Ok(0)
    }

    async fn process_and_transcribe(&self, audio_data: Vec<f32>) -> Result<i32> {
        let len = audio_data.len();
        eprintln!("Processing audio: {len} samples");

        // Preprocess
        let processed = match self.pipeline.preprocess(&audio_data) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Audio processing failed: {e}");
                let _ = self.beeps.play_async(BeepType::Error).await;
                return Ok(1);
            }
        };

        // Encode WAV
        let wav = match self.pipeline.to_wav(&processed) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to encode WAV: {e}");
                return Ok(1);
            }
        };

        // Transcribe
        // Normalize language: treat "auto" or empty as None for providers like OpenAI
        let language_opt = {
            let s = self.config.whisper_language.trim();
            if s.is_empty() || s.eq_ignore_ascii_case("auto") {
                None
            } else {
                Some(s.to_string())
            }
        };

        match self
            .pipeline
            .transcribe(wav, self.provider.as_ref(), language_opt)
            .await
        {
            Ok(text) => {
                if text.is_empty() {
                    println!();
                    let _ = self.beeps.play_async(BeepType::Success).await;
                    return Ok(0);
                }
                eprintln!("Transcription successful: \"{text}\"");
                let exit_code = if let Some(cmd) = &self.pipe_to {
                    match command::execute_with_input(cmd, &text).await {
                        Ok(code) => code,
                        Err(e) => {
                            eprintln!("Failed to execute pipe command: {e}");
                            let _ = self.beeps.play_async(BeepType::Error).await;
                            1
                        }
                    }
                } else {
                    println!("{text}");
                    0
                };
                let _ = self.beeps.play_async(BeepType::Success).await;
                Ok(exit_code)
            }
            Err(e) => {
                eprintln!("❌ Transcription failed: {e}");
                let _ = self.beeps.play_async(BeepType::Error).await;
                // Provide helpful hints based on error type (minimal version)
                match &e {
                    TranscriptionError::AuthenticationFailed { provider, .. } => {
                        eprintln!("💡 Check your {provider} API key configuration");
                    }
                    TranscriptionError::NetworkError(details) => {
                        let error_type = &details.error_type;
                        let error_message = &details.error_message;
                        eprintln!(
                            "🌐 Network details: {error_type} - {error_message}"
                        );
                    }
                    TranscriptionError::FileTooLarge(size) => {
                        eprintln!("💡 Audio file too large: {size} bytes (max 25MB)");
                    }
                    TranscriptionError::ConfigurationError(_) => {
                        eprintln!("💡 Check your transcription provider configuration");
                    }
                    TranscriptionError::UnsupportedProvider(provider) => {
                        eprintln!(
                            "💡 Unsupported provider: {provider}. Check TRANSCRIPTION_PROVIDER setting"
                        );
                    }
                    TranscriptionError::ApiError(details) => {
                        if let Some(status) = details.status_code {
                            eprintln!("📡 API Response: HTTP {status}");
                        }
                        if let Some(code) = &details.error_code {
                            eprintln!("🏷️  Error Code: {code}");
                        }
                    }
                    TranscriptionError::JsonError(_) => {
                        eprintln!("💡 Failed to parse API response");
                    }
                }
                Ok(1)
            }
        }
    }
}
