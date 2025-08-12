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
    /// Recording started - single long beep (1kHz, 800ms) - classic answering machine style
    RecordingStart,
    /// Recording stopped - busy signal pattern (480Hz+620Hz pulses)
    RecordingStop,
    /// Error occurred - busy signal pattern (same as RecordingStop)
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
#[derive(Clone)]
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
            BeepType::RecordingStart => (1000.0, 800.0), // 1kHz beep, 800ms (classic answering machine beep)
            BeepType::RecordingStop => (480.0, 1500.0), // Busy signal base freq, 1.5s total (3 pulses)
            BeepType::Error => (480.0, 1500.0),         // Same as busy signal (3 pulses)
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

            // Special handling for busy signal (mix two frequencies)
            let sample_value = if (beep_type == BeepType::RecordingStop
                || beep_type == BeepType::Error)
                && frequency > 0.0
            {
                // Mix 480Hz and 620Hz for authentic busy signal
                let phase2 = (*sample_index as f32 * 620.0 / sample_rate) % 1.0;
                let signal1 = (*phase * 2.0 * std::f32::consts::PI).sin();
                let signal2 = (phase2 * 2.0 * std::f32::consts::PI).sin();
                (signal1 + signal2) * 0.5 * volume * volume_multiplier
            } else {
                (*phase * 2.0 * std::f32::consts::PI).sin() * volume * volume_multiplier
            };

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

            // Special handling for busy signal (mix two frequencies)
            let sample_value = if (beep_type == BeepType::RecordingStop
                || beep_type == BeepType::Error)
                && frequency > 0.0
            {
                // Mix 480Hz and 620Hz for authentic busy signal
                let phase2 = (*sample_index as f32 * 620.0 / sample_rate) % 1.0;
                let signal1 = (*phase * 2.0 * std::f32::consts::PI).sin();
                let signal2 = (phase2 * 2.0 * std::f32::consts::PI).sin();
                ((signal1 + signal2) * 0.5 * volume * volume_multiplier * i16::MAX as f32) as i16
            } else {
                ((*phase * 2.0 * std::f32::consts::PI).sin()
                    * volume
                    * volume_multiplier
                    * i16::MAX as f32) as i16
            };

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
        match beep_type {
            BeepType::RecordingStart => {
                // Single long beep (classic answering machine/movie style)
                base_frequency // Constant 1kHz tone
            }
            BeepType::RecordingStop => {
                // Busy signal: 480Hz + 620Hz mixed, pulsing pattern (250ms on, 250ms off)
                let progress = sample_index as f32 / total_samples as f32;
                // Create 3 pulses in 1.5 seconds (each pulse is 250ms on, 250ms off)
                if (progress < 0.167) || // First pulse (0-250ms)
                   (0.333..0.5).contains(&progress) || // Second pulse (500-750ms)
                   (0.667..0.833).contains(&progress)
                {
                    // Third pulse (1000-1250ms)
                    // Mix two frequencies for authentic busy signal
                    base_frequency // We'll handle the mixing in the fill_audio_buffer functions
                } else {
                    0.0 // Silence between pulses
                }
            }

            BeepType::Error => {
                // Same as busy signal: 480Hz + 620Hz mixed, pulsing pattern
                let progress = sample_index as f32 / total_samples as f32;
                // Create 3 pulses in 1.5 seconds (each pulse is 250ms on, 250ms off)
                if (progress < 0.167) || // First pulse (0-250ms)
                   (0.333..0.5).contains(&progress) || // Second pulse (500-750ms)
                   (0.667..0.833).contains(&progress)
                {
                    // Third pulse (1000-1250ms)
                    base_frequency // We'll handle the mixing in the fill_audio_buffer functions
                } else {
                    0.0 // Silence between pulses
                }
            }
        }
    }
}
