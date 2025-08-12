
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_lossless)]

use anyhow::Result;

/// WAV file encoder for converting f32 audio samples to 16-bit PCM WAV format
/// Optimized for OpenAI Whisper API requirements: 16kHz mono, 16-bit PCM
pub struct WavEncoder {
    sample_rate: u32,
    channels: u16,
}

impl WavEncoder {
    /// Create a new WAV encoder with specified sample rate and channels
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    /// Generate WAV header for the given number of samples
    pub fn generate_header(&self, num_samples: usize) -> Vec<u8> {
        let bits_per_sample = 16u16;
        let byte_rate = self.sample_rate * self.channels as u32 * (bits_per_sample as u32 / 8);
        let block_align = self.channels * (bits_per_sample / 8);
        let data_size = (num_samples * (bits_per_sample as usize / 8)) as u32;
        let file_size = 36 + data_size;

        let mut header = Vec::with_capacity(44);

        // RIFF header
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&file_size.to_le_bytes());
        header.extend_from_slice(b"WAVE");

        // fmt chunk
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        header.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        header.extend_from_slice(&self.channels.to_le_bytes());
        header.extend_from_slice(&self.sample_rate.to_le_bytes());
        header.extend_from_slice(&byte_rate.to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data chunk header
        header.extend_from_slice(b"data");
        header.extend_from_slice(&data_size.to_le_bytes());

        header
    }

    /// Convert f32 samples to i16 PCM format
    /// Input samples should be in range [-1.0, 1.0]
    pub fn convert_samples(&self, samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|&sample| {
                // Clamp to [-1.0, 1.0] range to prevent overflow
                let clamped = sample.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect()
    }

    /// Convert f32 audio buffer to complete WAV file bytes
    pub fn encode_to_wav(&self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.is_empty() {
            return Err(anyhow::anyhow!("Cannot encode empty audio buffer to WAV"));
        }

        // Convert samples to i16 PCM
        let pcm_samples = self.convert_samples(samples);

        // Generate WAV header
        let header = self.generate_header(pcm_samples.len());

        // Combine header and PCM data
        let mut wav_data = Vec::with_capacity(header.len() + pcm_samples.len() * 2);
        wav_data.extend_from_slice(&header);

        // Add PCM data as little-endian bytes
        for sample in pcm_samples {
            wav_data.extend_from_slice(&sample.to_le_bytes());
        }

        Ok(wav_data)
    }
}

impl Default for WavEncoder {
    /// Create encoder with Whisper-optimized defaults: 16kHz mono
    fn default() -> Self {
        Self::new(16000, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_encoder_creation() {
        let encoder = WavEncoder::new(16000, 1);
        assert_eq!(encoder.sample_rate, 16000);
        assert_eq!(encoder.channels, 1);
    }

    #[test]
    fn test_wav_encoder_default() {
        let encoder = WavEncoder::default();
        assert_eq!(encoder.sample_rate, 16000);
        assert_eq!(encoder.channels, 1);
    }

    #[test]
    fn test_wav_header_generation_mono_16khz() {
        let encoder = WavEncoder::new(16000, 1);
        let header = encoder.generate_header(1000);

        // Should be exactly 44 bytes
        assert_eq!(header.len(), 44);

        // Check RIFF signature
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");

        // Check fmt chunk
        assert_eq!(&header[12..16], b"fmt ");

        // Check PCM format (1)
        assert_eq!(u16::from_le_bytes([header[20], header[21]]), 1);

        // Check channels
        assert_eq!(u16::from_le_bytes([header[22], header[23]]), 1);

        // Check sample rate
        assert_eq!(
            u32::from_le_bytes([header[24], header[25], header[26], header[27]]),
            16000
        );

        // Check bits per sample
        assert_eq!(u16::from_le_bytes([header[34], header[35]]), 16);

        // Check data chunk header
        assert_eq!(&header[36..40], b"data");
    }

    #[test]
    fn test_wav_header_generation_stereo_44khz() {
        let encoder = WavEncoder::new(44100, 2);
        let header = encoder.generate_header(2000);

        assert_eq!(header.len(), 44);

        // Check channels
        assert_eq!(u16::from_le_bytes([header[22], header[23]]), 2);

        // Check sample rate
        assert_eq!(
            u32::from_le_bytes([header[24], header[25], header[26], header[27]]),
            44100
        );
    }

    #[test]
    fn test_f32_to_i16_conversion_normal_range() {
        let encoder = WavEncoder::default();
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let converted = encoder.convert_samples(&samples);

        assert_eq!(converted.len(), 5);
        assert_eq!(converted[0], 0); // 0.0 -> 0
        assert_eq!(converted[1], 16383); // 0.5 -> ~16383
        assert_eq!(converted[2], -16383); // -0.5 -> ~-16383
        assert_eq!(converted[3], i16::MAX); // 1.0 -> 32767
        assert_eq!(converted[4], i16::MIN + 1); // -1.0 -> -32767 (not -32768 due to asymmetry)
    }

    #[test]
    fn test_f32_to_i16_conversion_clamping() {
        let encoder = WavEncoder::default();
        let samples = vec![2.0, -2.0, 1.5, -1.5];
        let converted = encoder.convert_samples(&samples);

        assert_eq!(converted.len(), 4);
        // Values outside [-1.0, 1.0] should be clamped
        assert_eq!(converted[0], i16::MAX); // 2.0 clamped to 1.0
        assert_eq!(converted[1], i16::MIN + 1); // -2.0 clamped to -1.0
        assert_eq!(converted[2], i16::MAX); // 1.5 clamped to 1.0
        assert_eq!(converted[3], i16::MIN + 1); // -1.5 clamped to -1.0
    }

    #[test]
    fn test_empty_audio_buffer_handling() {
        let encoder = WavEncoder::default();
        let empty_samples: Vec<f32> = vec![];

        let result = encoder.encode_to_wav(&empty_samples);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("empty audio buffer"));
    }

    #[test]
    fn test_wav_bytes_output_structure() {
        let encoder = WavEncoder::default();
        let samples = vec![0.0, 0.5, -0.5];

        let wav_data = encoder.encode_to_wav(&samples).unwrap();

        // Should have 44-byte header + 6 bytes of PCM data (3 samples * 2 bytes each)
        assert_eq!(wav_data.len(), 50);

        // First 44 bytes should be the header
        assert_eq!(&wav_data[0..4], b"RIFF");
        assert_eq!(&wav_data[8..12], b"WAVE");
        assert_eq!(&wav_data[36..40], b"data");

        // Data size in header should be 6 bytes
        let data_size =
            u32::from_le_bytes([wav_data[40], wav_data[41], wav_data[42], wav_data[43]]);
        assert_eq!(data_size, 6);

        // PCM data should start at byte 44
        assert_eq!(wav_data.len(), 44 + 6);
    }

    #[test]
    fn test_wav_size_calculation() {
        // Test WAV size calculation manually since calculate_wav_size was removed
        let encoder = WavEncoder::default();
        let samples = vec![0.1; 100];

        let wav_data = encoder.encode_to_wav(&samples).unwrap();
        // WAV should be 44 bytes header + 100 samples * 2 bytes per sample = 244 bytes
        assert_eq!(wav_data.len(), 244);

        // Test empty case - should fail gracefully
        let empty_result = encoder.encode_to_wav(&[]);
        assert!(empty_result.is_err()); // Empty buffer should be rejected
    }

    #[test]
    fn test_whisper_optimized_format() {
        let encoder = WavEncoder::default();
        let samples = vec![0.1, 0.2, 0.3]; // Small test sample

        let wav_data = encoder.encode_to_wav(&samples).unwrap();
        let header = &wav_data[0..44];

        // Verify it matches Whisper requirements
        assert_eq!(u16::from_le_bytes([header[22], header[23]]), 1); // Mono
        assert_eq!(
            u32::from_le_bytes([header[24], header[25], header[26], header[27]]),
            16000
        ); // 16kHz
        assert_eq!(u16::from_le_bytes([header[34], header[35]]), 16); // 16-bit
    }

    #[test]
    fn test_large_audio_buffer() {
        let encoder = WavEncoder::default();
        // Test with 1 second of audio at 16kHz (16000 samples)
        let samples: Vec<f32> = (0..16000).map(|i| (i as f32 / 16000.0).sin()).collect();

        let wav_data = encoder.encode_to_wav(&samples).unwrap();

        // Should be 44 bytes header + 32000 bytes data (16000 samples * 2 bytes each)
        assert_eq!(wav_data.len(), 44 + 32000);

        // Verify header indicates correct data size
        let data_size =
            u32::from_le_bytes([wav_data[40], wav_data[41], wav_data[42], wav_data[43]]);
        assert_eq!(data_size, 32000);
    }
}
