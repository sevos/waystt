#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::unused_self)]
#![allow(clippy::similar_names)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_closure_for_method_calls)]

use anyhow::Result;

/// Audio processing utilities for speech recognition optimization
/// Implements silence detection, trimming, and normalization
pub struct AudioProcessor {
    sample_rate: u32,
    window_size_ms: u32,
}

impl AudioProcessor {
    /// Create new audio processor with specified sample rate
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            window_size_ms: 10, // 10ms windows for RMS calculation
        }
    }

    /// Calculate RMS (Root Mean Square) for a window of audio samples
    pub fn calculate_rms(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    /// Detect silence in audio using RMS-based threshold
    /// Returns indices of silence regions as (start, end) pairs
    pub fn detect_silence(&self, samples: &[f32], silence_threshold: f32) -> Vec<(usize, usize)> {
        let window_size = self.get_window_size_samples();
        let mut silence_regions = Vec::new();
        let mut silence_start = None;

        for (i, window) in samples.chunks(window_size).enumerate() {
            let rms = self.calculate_rms(window);
            let window_start = i * window_size;

            if rms <= silence_threshold {
                // Start of silence region
                if silence_start.is_none() {
                    silence_start = Some(window_start);
                }
            } else {
                // End of silence region
                if let Some(start) = silence_start {
                    silence_regions.push((start, window_start));
                    silence_start = None;
                }
            }
        }

        // Handle silence extending to end of audio
        if let Some(start) = silence_start {
            silence_regions.push((start, samples.len()));
        }

        silence_regions
    }

    /// Calculate adaptive silence threshold based on audio content
    /// Uses 10% of peak RMS as threshold
    pub fn calculate_silence_threshold(&self, samples: &[f32]) -> f32 {
        let window_size = self.get_window_size_samples();
        let mut max_rms: f32 = 0.0;

        for window in samples.chunks(window_size) {
            let rms = self.calculate_rms(window);
            max_rms = max_rms.max(rms);
        }

        max_rms * 0.1 // 10% of peak RMS
    }

    /// Trim silence from start and end of audio buffer
    pub fn trim_silence(&self, samples: &[f32]) -> Result<Vec<f32>> {
        if samples.is_empty() {
            return Err(anyhow::anyhow!(
                "Cannot trim silence from empty audio buffer"
            ));
        }

        let threshold = self.calculate_silence_threshold(samples);
        let silence_regions = self.detect_silence(samples, threshold);

        // Find first non-silence region
        let start_trim = silence_regions
            .iter()
            .find(|(start, _)| *start == 0)
            .map(|(_, end)| *end)
            .unwrap_or(0);

        // Find last non-silence region
        let end_trim = silence_regions
            .iter()
            .rev()
            .find(|(_, end)| *end == samples.len())
            .map(|(start, _)| *start)
            .unwrap_or(samples.len());

        if start_trim >= end_trim {
            return Err(anyhow::anyhow!("Audio contains only silence"));
        }

        Ok(samples[start_trim..end_trim].to_vec())
    }

    /// Normalize audio to optimal levels for speech recognition
    /// Uses peak normalization to 80% of maximum amplitude
    pub fn normalize_audio(&self, samples: &[f32]) -> Vec<f32> {
        if samples.is_empty() {
            return samples.to_vec();
        }

        // Find peak amplitude
        let peak = samples
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, |acc, x| acc.max(x));

        if peak == 0.0 {
            return samples.to_vec(); // Silent audio, no normalization needed
        }

        // Normalize to 80% of maximum amplitude to prevent clipping
        let target_amplitude = 0.8;
        let gain = target_amplitude / peak;

        samples.iter().map(|&s| s * gain).collect()
    }

    /// Validate audio quality and duration
    pub fn validate_audio(&self, samples: &[f32]) -> Result<()> {
        if samples.is_empty() {
            return Err(anyhow::anyhow!("Audio buffer is empty"));
        }

        let duration_seconds = samples.len() as f32 / self.sample_rate as f32;

        if duration_seconds < 0.1 {
            return Err(anyhow::anyhow!(
                "Audio duration too short: {:.2}s (minimum 0.1s required)",
                duration_seconds
            ));
        }

        // Check if audio is completely silent (all zeros). Use a very small epsilon
        // to guard against exact-zero buffers while allowing very quiet audio to pass
        // to the trimming/normalization stage.
        let peak = samples
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, |acc, x| acc.max(x));
        if peak <= f32::EPSILON {
            return Err(anyhow::anyhow!("Audio contains no detectable signal"));
        }

        Ok(())
    }

    /// Complete audio processing pipeline for speech recognition
    pub fn process_for_speech_recognition(&self, samples: &[f32]) -> Result<Vec<f32>> {
        // 1. Validate input
        self.validate_audio(samples)?;

        // 2. Trim silence from start and end
        let trimmed = self.trim_silence(samples)?;

        // 3. Validate trimmed audio
        self.validate_audio(&trimmed)?;

        // 4. Normalize for optimal recognition
        let normalized = self.normalize_audio(&trimmed);

        Ok(normalized)
    }

    /// Get window size in samples for RMS calculation
    fn get_window_size_samples(&self) -> usize {
        (self.sample_rate as f32 * self.window_size_ms as f32 / 1000.0) as usize
    }

    /// Get audio duration in seconds
    pub fn get_duration_seconds(&self, samples: &[f32]) -> f32 {
        samples.len() as f32 / self.sample_rate as f32
    }
}

impl Default for AudioProcessor {
    /// Create processor with Whisper-optimized defaults: 16kHz
    fn default() -> Self {
        Self::new(16000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_processor_creation() {
        let processor = AudioProcessor::new(16000);
        assert_eq!(processor.sample_rate, 16000);
        assert_eq!(processor.window_size_ms, 10);
    }

    #[test]
    fn test_audio_processor_default() {
        let processor = AudioProcessor::default();
        assert_eq!(processor.sample_rate, 16000);
        assert_eq!(processor.window_size_ms, 10);
    }

    #[test]
    fn test_calculate_rms_empty() {
        let processor = AudioProcessor::default();
        let empty_samples: Vec<f32> = vec![];
        assert_eq!(processor.calculate_rms(&empty_samples), 0.0);
    }

    #[test]
    fn test_calculate_rms_silent() {
        let processor = AudioProcessor::default();
        let silent_samples = vec![0.0, 0.0, 0.0, 0.0];
        assert_eq!(processor.calculate_rms(&silent_samples), 0.0);
    }

    #[test]
    fn test_calculate_rms_signal() {
        let processor = AudioProcessor::default();
        let samples = vec![0.5, -0.5, 0.5, -0.5];
        let rms = processor.calculate_rms(&samples);
        assert!((rms - 0.5).abs() < 0.001); // RMS of ±0.5 square wave is 0.5
    }

    #[test]
    fn test_silence_detection_threshold() {
        let processor = AudioProcessor::default();
        // Create audio with quiet section (silence) and loud section (speech)
        let mut samples = vec![0.01; 1600]; // 0.1s of quiet audio
        samples.extend(vec![0.5; 1600]); // 0.1s of loud audio
        samples.extend(vec![0.01; 1600]); // 0.1s of quiet audio again

        let threshold = processor.calculate_silence_threshold(&samples);
        // Should be 10% of 0.5 = 0.05
        assert!((threshold - 0.05).abs() < 0.01);
    }

    #[test]
    fn test_detect_silence_regions() {
        let processor = AudioProcessor::default();
        let window_size = processor.get_window_size_samples(); // 160 samples at 16kHz

        // Create pattern: silence - speech - silence
        let mut samples = vec![0.0; window_size * 2]; // 2 windows of silence
        samples.extend(vec![0.5; window_size * 3]); // 3 windows of speech
        samples.extend(vec![0.0; window_size]); // 1 window of silence

        let silence_regions = processor.detect_silence(&samples, 0.1);

        // Should detect silence at start and end
        assert_eq!(silence_regions.len(), 2);
        assert_eq!(silence_regions[0], (0, window_size * 2));
        assert_eq!(silence_regions[1], (window_size * 5, window_size * 6));
    }

    #[test]
    fn test_trim_silence_normal_audio() {
        let processor = AudioProcessor::default();
        let window_size = processor.get_window_size_samples();

        // Create audio: silence - speech - silence
        let mut samples = vec![0.0; window_size]; // Leading silence
        let speech_samples = vec![0.5; window_size * 2]; // Speech content
        samples.extend(&speech_samples);
        samples.extend(vec![0.0; window_size]); // Trailing silence

        let trimmed = processor.trim_silence(&samples).unwrap();

        // Should be just the speech portion
        assert_eq!(trimmed.len(), window_size * 2);
        assert!((trimmed[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_trim_silence_empty_buffer() {
        let processor = AudioProcessor::default();
        let empty_samples: Vec<f32> = vec![];

        let result = processor.trim_silence(&empty_samples);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("empty audio buffer"));
    }

    #[test]
    fn test_trim_silence_only_silence() {
        let processor = AudioProcessor::default();
        let silent_samples = vec![0.0; 1600]; // All silence

        let result = processor.trim_silence(&silent_samples);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("only silence"));
    }

    #[test]
    fn test_normalize_audio_empty() {
        let processor = AudioProcessor::default();
        let empty_samples: Vec<f32> = vec![];
        let normalized = processor.normalize_audio(&empty_samples);
        assert_eq!(normalized.len(), 0);
    }

    #[test]
    fn test_normalize_audio_silent() {
        let processor = AudioProcessor::default();
        let silent_samples = vec![0.0; 100];
        let normalized = processor.normalize_audio(&silent_samples);
        assert_eq!(normalized, silent_samples);
    }

    #[test]
    fn test_normalize_audio_peak_normalization() {
        let processor = AudioProcessor::default();
        let samples = vec![0.5, -0.5, 0.25]; // Peak is 0.5
        let normalized = processor.normalize_audio(&samples);

        // Should be normalized to 80% peak (0.8 / 0.5 = 1.6 gain)
        assert!((normalized[0] - 0.8).abs() < 0.001);
        assert!((normalized[1] - (-0.8)).abs() < 0.001);
        assert!((normalized[2] - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_validate_audio_empty() {
        let processor = AudioProcessor::default();
        let empty_samples: Vec<f32> = vec![];

        let result = processor.validate_audio(&empty_samples);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_validate_audio_too_short() {
        let processor = AudioProcessor::default();
        let short_samples = vec![0.5; 160]; // 0.01s at 16kHz (too short)

        let result = processor.validate_audio(&short_samples);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn test_validate_audio_no_signal() {
        let processor = AudioProcessor::default();
        let silent_samples = vec![0.0; 1600]; // 0.1s of complete silence

        let result = processor.validate_audio(&silent_samples);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no detectable signal"));
    }

    #[test]
    fn test_validate_audio_valid() {
        let processor = AudioProcessor::default();
        let valid_samples = vec![0.1; 1600]; // 0.1s of valid audio

        let result = processor.validate_audio(&valid_samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_duration_seconds() {
        let processor = AudioProcessor::default();
        let samples_1s = vec![0.0; 16000]; // 1 second at 16kHz
        let samples_half_s = vec![0.0; 8000]; // 0.5 seconds at 16kHz

        assert_eq!(processor.get_duration_seconds(&samples_1s), 1.0);
        assert_eq!(processor.get_duration_seconds(&samples_half_s), 0.5);
    }

    #[test]
    fn test_process_for_speech_recognition_valid() {
        let processor = AudioProcessor::default();
        let window_size = processor.get_window_size_samples();

        // Create audio: silence - speech - silence
        let mut samples = vec![0.0; window_size]; // Leading silence
        samples.extend(vec![0.2; window_size * 10]); // 10 windows of speech (enough duration)
        samples.extend(vec![0.0; window_size]); // Trailing silence

        let processed = processor.process_for_speech_recognition(&samples).unwrap();

        // Should be trimmed and normalized
        assert!(processed.len() < samples.len()); // Trimmed
        assert!(processed.len() >= window_size * 10); // Contains the speech

        // Check normalization - peak should be around 0.8
        let peak = processed
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, |acc, x| acc.max(x));
        assert!((peak - 0.8).abs() < 0.1);
    }

    #[test]
    fn test_process_for_speech_recognition_invalid() {
        let processor = AudioProcessor::default();

        // Test with too short audio
        let short_samples = vec![0.5; 160]; // 0.01s
        let result = processor.process_for_speech_recognition(&short_samples);
        assert!(result.is_err());

        // Test with only silence
        let silent_samples = vec![0.0; 1600]; // 0.1s of silence
        let result = processor.process_for_speech_recognition(&silent_samples);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_window_size_samples() {
        let processor_16k = AudioProcessor::new(16000);
        let processor_44k = AudioProcessor::new(44100);

        // 10ms windows
        assert_eq!(processor_16k.get_window_size_samples(), 160); // 16000 * 0.01
        assert_eq!(processor_44k.get_window_size_samples(), 441); // 44100 * 0.01
    }
}
