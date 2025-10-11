//! Audio processing + WAV encoding + transcription pipeline utilities.

use anyhow::{anyhow, Result};

use crate::audio_processing::AudioProcessor;
use crate::transcription::{TranscriptionError, TranscriptionProvider};
use crate::wav::WavEncoder;

pub struct AudioPipeline {
    processor: AudioProcessor,
    encoder: WavEncoder,
}

impl AudioPipeline {
    /// Create a new audio processing pipeline
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let processor = AudioProcessor::new(sample_rate);
        let encoder = WavEncoder::new(sample_rate, 1);
        Self { processor, encoder }
    }

    /// Preprocess audio samples for speech recognition
    ///
    /// # Errors
    ///
    /// Returns an error if audio is empty or contains only silence
    pub fn preprocess(&self, audio: &[f32]) -> Result<Vec<f32>> {
        if audio.is_empty() {
            return Err(anyhow!("No audio captured or microphone not available"));
        }
        self.processor
            .process_for_speech_recognition(audio)
            .map_err(|e| anyhow!(e))
    }

    /// Encode audio samples to WAV format
    ///
    /// # Errors
    ///
    /// Returns an error if the samples are empty or encoding fails
    pub fn to_wav(&self, samples: &[f32]) -> Result<Vec<u8>> {
        self.encoder.encode_to_wav(samples).map_err(|e| anyhow!(e))
    }

    /// Transcribe audio using the provided transcription provider
    ///
    /// # Errors
    ///
    /// Returns a transcription error if the API call fails or the audio cannot be processed
    pub async fn transcribe(
        &self,
        wav: Vec<u8>,
        provider: &dyn TranscriptionProvider,
        language: Option<String>,
    ) -> Result<String, TranscriptionError> {
        provider.transcribe_with_language(wav, language).await
    }
}

impl Default for AudioPipeline {
    fn default() -> Self {
        Self::new(16000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_and_wav() {
        let sample_rate = 16000u32;
        let p = AudioPipeline::new(sample_rate);
        // silence - speech - silence
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let window = (sample_rate as f32 * 0.01) as usize;
        let mut audio = vec![0.0; window];
        audio.extend(vec![0.2; window * 20]);
        audio.extend(vec![0.0; window]);

        let processed = p.preprocess(&audio).unwrap();
        let wav = p.to_wav(&processed).unwrap();
        assert!(wav.len() > 44);
    }
}
