#![allow(clippy::doc_markdown)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::single_match_else)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::match_same_arms)]

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Types of beeps for different events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeepType {
    /// Recording started - short, ascending tone (400Hz, 150ms)
    RecordingStart,
    /// Recording stopped - short, descending tone (400Hz→200Hz, 150ms)
    RecordingStop,
    /// Success (typing/clipboard complete) - double beep (800Hz, 100ms each with 50ms gap)
    Success,
    /// Error occurred - low, warbling tone (200Hz, 300ms)
    Error,
}

/// Configuration for audio feedback
#[derive(Debug, Clone)]
pub struct BeepConfig {
    pub enabled: bool,
    pub volume: f32,
}

impl Default for BeepConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.1,
        }
    }
}

/// Audio feedback player for user notifications
pub struct BeepPlayer {
    config: BeepConfig,
}

impl BeepPlayer {
    /// Create a new BeepPlayer with the given configuration
    pub fn new(config: BeepConfig) -> Result<Self> {
        Ok(Self { config })
    }

    /// Play a beep asynchronously (non-blocking)
    pub async fn play_async(&self, beep_type: BeepType) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let beep_type_copy = beep_type;
        let volume = self.config.volume;

        tokio::task::spawn_blocking(move || Self::play_beep_internal(beep_type_copy, volume))
            .await??;

        Ok(())
    }

    /// Internal beep generation using CPAL
    fn play_beep_internal(beep_type: BeepType, volume: f32) -> Result<()> {
        // Gracefully handle audio device conflicts
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(device) => device,
            None => {
                eprintln!("Warning: No audio output device available for beeps");
                return Ok(());
            }
        };

        let config = match device.default_output_config() {
            Ok(config) => config,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to get audio output config for beeps: {}",
                    e
                );
                return Ok(());
            }
        };

        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        let (frequency, duration_ms) = Self::get_beep_params(beep_type);
        let sample_count = (sample_rate * duration_ms / 1000.0) as usize;

        let playing = Arc::new(AtomicBool::new(true));
        let playing_clone = playing.clone();

        let mut sample_index = 0usize;
        let mut phase = 0.0f32;

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                match device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::fill_audio_buffer_f32(
                            data,
                            &mut sample_index,
                            &mut phase,
                            sample_count,
                            frequency,
                            sample_rate,
                            channels,
                            volume,
                            &playing_clone,
                            beep_type,
                        );
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None,
                ) {
                    Ok(stream) => stream,
                    Err(e) => {
                        eprintln!("Warning: Failed to create audio output stream: {}", e);
                        return Ok(());
                    }
                }
            }
            cpal::SampleFormat::I16 => {
                match device.build_output_stream(
                    &config.into(),
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::fill_audio_buffer_i16(
                            data,
                            &mut sample_index,
                            &mut phase,
                            sample_count,
                            frequency,
                            sample_rate,
                            channels,
                            volume,
                            &playing_clone,
                            beep_type,
                        );
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None,
                ) {
                    Ok(stream) => stream,
                    Err(e) => {
                        eprintln!("Warning: Failed to create audio output stream: {}", e);
                        return Ok(());
                    }
                }
            }
            _ => {
                eprintln!("Warning: Unsupported audio format for beeps");
                return Ok(());
            }
        };

        if let Err(e) = stream.play() {
            eprintln!("Warning: Failed to start audio stream for beep: {}", e);
            return Ok(());
        }

        // Wait for beep to complete
        while playing.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(10));
        }

        // Explicitly drop the stream to release resources
        drop(stream);

        Ok(())
    }

    /// Get frequency and duration parameters for different beep types
    fn get_beep_params(beep_type: BeepType) -> (f32, f32) {
        match beep_type {
            BeepType::RecordingStart => (261.63, 500.0), // C major (C4), 500ms total for "ding dong"
            BeepType::RecordingStop => (329.63, 500.0), // E major (E4), 500ms total for "dong ding"
            BeepType::Success => (329.63, 400.0),       // E major (E4), 400ms total for "ding ding"
            BeepType::Error => (200.0, 300.0),          // 200Hz, 300ms (unchanged)
        }
    }

    /// Fill audio buffer with f32 samples
    #[allow(clippy::too_many_arguments)]
    fn fill_audio_buffer_f32(
        data: &mut [f32],
        sample_index: &mut usize,
        phase: &mut f32,
        sample_count: usize,
        base_frequency: f32,
        sample_rate: f32,
        channels: usize,
        volume: f32,
        playing: &Arc<AtomicBool>,
        beep_type: BeepType,
    ) {
        for frame in data.chunks_mut(channels) {
            if *sample_index >= sample_count {
                playing.store(false, Ordering::Relaxed);
                for sample in frame {
                    *sample = 0.0;
                }
                continue;
            }

            let frequency = Self::get_frequency_at_sample(
                *sample_index,
                sample_count,
                base_frequency,
                beep_type,
            );
            let volume_multiplier = Self::get_volume_multiplier(beep_type);
            let sample_value =
                (*phase * 2.0 * std::f32::consts::PI).sin() * volume * volume_multiplier;

            for sample in frame {
                *sample = sample_value;
            }

            *phase += frequency / sample_rate;
            if *phase > 1.0 {
                *phase -= 1.0;
            }

            *sample_index += 1;
        }
    }

    /// Fill audio buffer with i16 samples
    #[allow(clippy::too_many_arguments)]
    fn fill_audio_buffer_i16(
        data: &mut [i16],
        sample_index: &mut usize,
        phase: &mut f32,
        sample_count: usize,
        base_frequency: f32,
        sample_rate: f32,
        channels: usize,
        volume: f32,
        playing: &Arc<AtomicBool>,
        beep_type: BeepType,
    ) {
        for frame in data.chunks_mut(channels) {
            if *sample_index >= sample_count {
                playing.store(false, Ordering::Relaxed);
                for sample in frame {
                    *sample = 0;
                }
                continue;
            }

            let frequency = Self::get_frequency_at_sample(
                *sample_index,
                sample_count,
                base_frequency,
                beep_type,
            );
            let volume_multiplier = Self::get_volume_multiplier(beep_type);
            let sample_value = ((*phase * 2.0 * std::f32::consts::PI).sin()
                * volume
                * volume_multiplier
                * i16::MAX as f32) as i16;

            for sample in frame {
                *sample = sample_value;
            }

            *phase += frequency / sample_rate;
            if *phase > 1.0 {
                *phase -= 1.0;
            }

            *sample_index += 1;
        }
    }

    /// Get volume multiplier for different beep types
    fn get_volume_multiplier(beep_type: BeepType) -> f32 {
        match beep_type {
            BeepType::RecordingStart => 2.0, // Twice as loud
            BeepType::RecordingStop => 2.0,  // Twice as loud
            BeepType::Success => 1.0,        // Normal volume
            BeepType::Error => 1.0,          // Normal volume
        }
    }

    /// Get frequency at a specific sample for different beep effects
    fn get_frequency_at_sample(
        sample_index: usize,
        total_samples: usize,
        base_frequency: f32,
        beep_type: BeepType,
    ) -> f32 {
        const C4: f32 = 261.63; // C major (C4)
        const E4: f32 = 329.63; // E major (E4)

        match beep_type {
            BeepType::RecordingStart => {
                // "Ding dong": C major then E major (low to high)
                let progress = sample_index as f32 / total_samples as f32;
                if progress < 0.45 {
                    C4 // First beep: C major
                } else if progress < 0.55 {
                    0.0 // Short gap between beeps
                } else {
                    E4 // Second beep: E major
                }
            }
            BeepType::RecordingStop => {
                // "Dong ding": E major then C major (high to low) - symmetrical
                let progress = sample_index as f32 / total_samples as f32;
                if progress < 0.45 {
                    E4 // First beep: E major
                } else if progress < 0.55 {
                    0.0 // Short gap between beeps
                } else {
                    C4 // Second beep: C major
                }
            }
            BeepType::Success => {
                // "Ding ding": Double E major beeps
                let progress = sample_index as f32 / total_samples as f32;
                if progress < 0.4 {
                    E4 // First ding: E major
                } else if progress < 0.6 {
                    0.0 // Gap between dings
                } else {
                    E4 // Second ding: E major
                }
            }
            BeepType::Error => {
                // Warbling tone: oscillate between 180Hz and 220Hz (unchanged)
                let wobble =
                    (sample_index as f32 * 8.0 / total_samples as f32 * 2.0 * std::f32::consts::PI)
                        .sin();
                base_frequency + (20.0 * wobble)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beep_config_default() {
        let config = BeepConfig::default();
        assert!(config.enabled);
        assert_eq!(config.volume, 0.1);
    }

    #[test]
    fn test_beep_config_custom() {
        let config = BeepConfig {
            enabled: false,
            volume: 0.5,
        };
        assert!(!config.enabled);
        assert_eq!(config.volume, 0.5);
    }

    #[test]
    fn test_beep_player_creation() {
        let config = BeepConfig::default();
        let player = BeepPlayer::new(config.clone());
        assert!(player.is_ok());

        let player = player.unwrap();
        assert_eq!(player.config.enabled, config.enabled);
        assert_eq!(player.config.volume, config.volume);
    }

    #[tokio::test]
    async fn test_beep_player_disabled_config() {
        let config = BeepConfig {
            enabled: false,
            volume: 0.1,
        };
        let player = BeepPlayer::new(config).unwrap();

        // Should return Ok(()) when disabled, not attempt to play
        let result = player.play_async(BeepType::RecordingStart).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_beep_player_async_disabled_config() {
        let config = BeepConfig {
            enabled: false,
            volume: 0.1,
        };
        let player = BeepPlayer::new(config).unwrap();

        // Should return Ok(()) when disabled, not attempt to play
        let result = player.play_async(BeepType::Success).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_beep_types_equality() {
        assert_eq!(BeepType::RecordingStart, BeepType::RecordingStart);
        assert_eq!(BeepType::RecordingStop, BeepType::RecordingStop);
        assert_eq!(BeepType::Success, BeepType::Success);
        assert_eq!(BeepType::Error, BeepType::Error);

        assert_ne!(BeepType::RecordingStart, BeepType::RecordingStop);
        assert_ne!(BeepType::Success, BeepType::Error);
    }

    #[test]
    fn test_beep_types_debug() {
        // Ensure BeepType implements Debug for logging
        let types = vec![
            BeepType::RecordingStart,
            BeepType::RecordingStop,
            BeepType::Success,
            BeepType::Error,
        ];

        for beep_type in types {
            let debug_string = format!("{:?}", beep_type);
            assert!(!debug_string.is_empty());
        }
    }

    #[test]
    fn test_beep_config_volume_bounds() {
        // Test that we can set volume to various values
        let volumes = vec![0.0, 0.1, 0.5, 1.0];

        for volume in volumes {
            let config = BeepConfig {
                enabled: true,
                volume,
            };
            let player = BeepPlayer::new(config).unwrap();
            assert_eq!(player.config.volume, volume);
        }
    }

    #[tokio::test]
    async fn test_beep_player_play_async_all_types() {
        let config = BeepConfig::default();
        let player = BeepPlayer::new(config).unwrap();

        // Test all beep types asynchronously
        let beep_types = vec![
            BeepType::RecordingStart,
            BeepType::RecordingStop,
            BeepType::Success,
            BeepType::Error,
        ];

        for beep_type in beep_types {
            let result = player.play_async(beep_type).await;
            // We don't assert success here because audio devices might not be available in test environments
            match result {
                Ok(_) => {
                    if std::env::var("CI").is_err() {
                        println!("Async beep {:?} played successfully", beep_type);
                    }
                }
                Err(e) => {
                    if std::env::var("CI").is_err() {
                        println!(
                            "Async beep {:?} failed (expected in test env): {}",
                            beep_type, e
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_beep_params() {
        // Test that beep parameters are reasonable
        let params = [
            (BeepType::RecordingStart, 261.63, 500.0), // C major (C4)
            (BeepType::RecordingStop, 329.63, 500.0),  // E major (E4)
            (BeepType::Success, 329.63, 400.0),        // E major (E4)
            (BeepType::Error, 200.0, 300.0),           // Unchanged
        ];

        for (beep_type, expected_freq, expected_duration) in params {
            let (freq, duration) = BeepPlayer::get_beep_params(beep_type);
            assert_eq!(
                freq, expected_freq,
                "Frequency mismatch for {:?}",
                beep_type
            );
            assert_eq!(
                duration, expected_duration,
                "Duration mismatch for {:?}",
                beep_type
            );
        }
    }

    #[test]
    fn test_volume_multipliers() {
        // Test volume multipliers for different beep types
        assert_eq!(
            BeepPlayer::get_volume_multiplier(BeepType::RecordingStart),
            2.0
        );
        assert_eq!(
            BeepPlayer::get_volume_multiplier(BeepType::RecordingStop),
            2.0
        );
        assert_eq!(BeepPlayer::get_volume_multiplier(BeepType::Success), 1.0);
        assert_eq!(BeepPlayer::get_volume_multiplier(BeepType::Error), 1.0);
    }

    #[test]
    fn test_frequency_at_sample() {
        // Test frequency calculation for different beep types
        let total_samples = 1000;
        const C4: f32 = 261.63; // C major (C4)
        const E4: f32 = 329.63; // E major (E4)

        // Test recording start: C major then E major (ding dong)
        let start_freq =
            BeepPlayer::get_frequency_at_sample(0, total_samples, C4, BeepType::RecordingStart);
        let end_freq = BeepPlayer::get_frequency_at_sample(
            total_samples - 1,
            total_samples,
            C4,
            BeepType::RecordingStart,
        );
        assert_eq!(start_freq, C4, "Recording start should begin with C major");
        assert_eq!(end_freq, E4, "Recording start should end with E major");

        // Test recording stop: E major then C major (dong ding - symmetrical)
        let start_freq =
            BeepPlayer::get_frequency_at_sample(0, total_samples, E4, BeepType::RecordingStop);
        let end_freq = BeepPlayer::get_frequency_at_sample(
            total_samples - 1,
            total_samples,
            E4,
            BeepType::RecordingStop,
        );
        assert_eq!(start_freq, E4, "Recording stop should begin with E major");
        assert_eq!(end_freq, C4, "Recording stop should end with C major");

        // Test success: Double E major (ding ding)
        let first_beep =
            BeepPlayer::get_frequency_at_sample(100, total_samples, E4, BeepType::Success);
        let gap_freq = BeepPlayer::get_frequency_at_sample(
            total_samples / 2,
            total_samples,
            E4,
            BeepType::Success,
        );
        let second_beep = BeepPlayer::get_frequency_at_sample(
            total_samples - 100,
            total_samples,
            E4,
            BeepType::Success,
        );
        assert_eq!(first_beep, E4, "Success first beep should be E major");
        assert_eq!(gap_freq, 0.0, "Success should have silence in the middle");
        assert_eq!(second_beep, E4, "Success second beep should be E major");
    }
}
