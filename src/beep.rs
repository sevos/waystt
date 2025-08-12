#![allow(clippy::doc_markdown)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::single_match_else)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::too_many_arguments)]

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Types of beeps for different events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeepType {
    /// System ready - dual-tone hum (350Hz+440Hz, 1s)
    LineReady,
    /// Recording started - single long beep (1kHz, 800ms)
    RecordingStart,
    /// Recording stopped - busy signal pattern (480Hz+620Hz pulses)
    RecordingStop,
    /// Error occurred - SIT tone (three rising beeps)
    Error,
}

/// The type of sound to play in a segment.
#[derive(Debug, Clone, Copy)]
pub enum SoundType {
    /// A single frequency tone.
    Single(f32),
    /// A dual-frequency tone.
    Dual(f32, f32),
    /// Silence.
    Silence,
}

/// A segment of a beep, consisting of a sound type and a duration.
#[derive(Debug, Clone, Copy)]
pub struct BeepSegment {
    duration_ms: f32,
    sound_type: SoundType,
}

/// Configuration for audio feedback.
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

/// Audio feedback player for user notifications.
#[derive(Clone)]
pub struct BeepPlayer {
    config: BeepConfig,
}

impl BeepPlayer {
    /// Create a new BeepPlayer with the given configuration.
    pub fn new(config: BeepConfig) -> Result<Self> {
        Ok(Self { config })
    }

    /// Play a beep asynchronously (non-blocking).
    pub async fn play_async(&self, beep_type: BeepType) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let volume = self.config.volume;
        tokio::task::spawn_blocking(move || Self::play_beep_internal(beep_type, volume)).await??;
        Ok(())
    }

    /// Defines the sound sequence for each beep type.
    fn get_beep_sequence(beep_type: BeepType) -> Vec<BeepSegment> {
        match beep_type {
            BeepType::LineReady => vec![BeepSegment {
                duration_ms: 1500.0,
                sound_type: SoundType::Dual(350.0, 440.0),
            }],
            BeepType::RecordingStart => vec![BeepSegment {
                duration_ms: 750.0,
                sound_type: SoundType::Single(1024.0),
            }],
            BeepType::RecordingStop => vec![
                BeepSegment {
                    duration_ms: 250.0,
                    sound_type: SoundType::Dual(480.0, 620.0),
                },
                BeepSegment {
                    duration_ms: 250.0,
                    sound_type: SoundType::Silence,
                },
                BeepSegment {
                    duration_ms: 250.0,
                    sound_type: SoundType::Dual(480.0, 620.0),
                },
                BeepSegment {
                    duration_ms: 250.0,
                    sound_type: SoundType::Silence,
                },
                BeepSegment {
                    duration_ms: 250.0,
                    sound_type: SoundType::Dual(480.0, 620.0),
                },
                BeepSegment {
                    duration_ms: 250.0,
                    sound_type: SoundType::Silence,
                },
            ],
            BeepType::Error => vec![
                BeepSegment {
                    duration_ms: 276.0,
                    sound_type: SoundType::Dual(913.8, 985.2),
                },
                BeepSegment {
                    duration_ms: 2.0,
                    sound_type: SoundType::Silence,
                },
                BeepSegment {
                    duration_ms: 276.0,
                    sound_type: SoundType::Dual(1370.6, 1428.5),
                },
                BeepSegment {
                    duration_ms: 2.0,
                    sound_type: SoundType::Silence,
                },
                BeepSegment {
                    duration_ms: 380.0,
                    sound_type: SoundType::Single(1776.7),
                },
            ],
        }
    }

    /// Get volume multiplier for different beep types.
    fn get_volume_multiplier(beep_type: BeepType) -> f32 {
        match beep_type {
            BeepType::LineReady => 0.5,
            BeepType::RecordingStart => 2.0,
            BeepType::RecordingStop => 2.0,
            BeepType::Error => 1.0,
        }
    }

    /// Internal beep generation using CPAL.
    fn play_beep_internal(beep_type: BeepType, volume: f32) -> Result<()> {
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

        let sequence = Self::get_beep_sequence(beep_type);
        let total_duration_ms: f32 = sequence.iter().map(|s| s.duration_ms).sum();
        let sample_count = (sample_rate * total_duration_ms / 1000.0) as usize;
        let volume_multiplier = Self::get_volume_multiplier(beep_type);
        let final_volume = volume * volume_multiplier;

        let playing = Arc::new(AtomicBool::new(true));
        let playing_clone = playing.clone();

        let mut sample_index = 0usize;
        let mut phase1 = 0.0f32;
        let mut phase2 = 0.0f32;

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    Self::fill_audio_buffer_f32(
                        data,
                        &mut sample_index,
                        &mut phase1,
                        &mut phase2,
                        sample_count,
                        &sequence,
                        sample_rate,
                        channels,
                        final_volume,
                        &playing_clone,
                    );
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.into(),
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    Self::fill_audio_buffer_i16(
                        data,
                        &mut sample_index,
                        &mut phase1,
                        &mut phase2,
                        sample_count,
                        &sequence,
                        sample_rate,
                        channels,
                        final_volume,
                        &playing_clone,
                    );
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )?,
            _ => {
                eprintln!("Warning: Unsupported audio format for beeps");
                return Ok(());
            }
        };

        stream.play()?;
        while playing.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(stream);
        Ok(())
    }

    /// Fill audio buffer with f32 samples.
    fn fill_audio_buffer_f32(
        data: &mut [f32],
        sample_index: &mut usize,
        phase1: &mut f32,
        phase2: &mut f32,
        sample_count: usize,
        sequence: &[BeepSegment],
        sample_rate: f32,
        channels: usize,
        volume: f32,
        playing: &Arc<AtomicBool>,
    ) {
        for frame in data.chunks_mut(channels) {
            if *sample_index >= sample_count {
                playing.store(false, Ordering::Relaxed);
                for sample in frame {
                    *sample = 0.0;
                }
                continue;
            }

            let current_ms = (*sample_index as f32 / sample_rate) * 1000.0;
            let mut elapsed_ms = 0.0;
            let mut current_segment = None;
            for segment in sequence {
                if current_ms < elapsed_ms + segment.duration_ms {
                    current_segment = Some(segment);
                    break;
                }
                elapsed_ms += segment.duration_ms;
            }

            let sample_value = if let Some(segment) = current_segment {
                match segment.sound_type {
                    SoundType::Single(freq) => {
                        let val = (*phase1 * 2.0 * std::f32::consts::PI).sin() * volume;
                        *phase1 = (*phase1 + freq / sample_rate) % 1.0;
                        val
                    }
                    SoundType::Dual(freq1, freq2) => {
                        let signal1 = (*phase1 * 2.0 * std::f32::consts::PI).sin();
                        let signal2 = (*phase2 * 2.0 * std::f32::consts::PI).sin();
                        *phase1 = (*phase1 + freq1 / sample_rate) % 1.0;
                        *phase2 = (*phase2 + freq2 / sample_rate) % 1.0;
                        (signal1 + signal2) * 0.5 * volume
                    }
                    SoundType::Silence => 0.0,
                }
            } else {
                0.0
            };

            for sample in frame {
                *sample = sample_value;
            }
            *sample_index += 1;
        }
    }

    /// Fill audio buffer with i16 samples.
    fn fill_audio_buffer_i16(
        data: &mut [i16],
        sample_index: &mut usize,
        phase1: &mut f32,
        phase2: &mut f32,
        sample_count: usize,
        sequence: &[BeepSegment],
        sample_rate: f32,
        channels: usize,
        volume: f32,
        playing: &Arc<AtomicBool>,
    ) {
        for frame in data.chunks_mut(channels) {
            if *sample_index >= sample_count {
                playing.store(false, Ordering::Relaxed);
                for sample in frame {
                    *sample = 0;
                }
                continue;
            }

            let current_ms = (*sample_index as f32 / sample_rate) * 1000.0;
            let mut elapsed_ms = 0.0;
            let mut current_segment = None;
            for segment in sequence {
                if current_ms < elapsed_ms + segment.duration_ms {
                    current_segment = Some(segment);
                    break;
                }
                elapsed_ms += segment.duration_ms;
            }

            let sample_value = if let Some(segment) = current_segment {
                match segment.sound_type {
                    SoundType::Single(freq) => {
                        let val = (*phase1 * 2.0 * std::f32::consts::PI).sin() * volume;
                        *phase1 = (*phase1 + freq / sample_rate) % 1.0;
                        (val * i16::MAX as f32) as i16
                    }
                    SoundType::Dual(freq1, freq2) => {
                        let signal1 = (*phase1 * 2.0 * std::f32::consts::PI).sin();
                        let signal2 = (*phase2 * 2.0 * std::f32::consts::PI).sin();
                        *phase1 = (*phase1 + freq1 / sample_rate) % 1.0;
                        *phase2 = (*phase2 + freq2 / sample_rate) % 1.0;
                        ((signal1 + signal2) * 0.5 * volume * i16::MAX as f32) as i16
                    }
                    SoundType::Silence => 0,
                }
            } else {
                0
            };

            for sample in frame {
                *sample = sample_value;
            }
            *sample_index += 1;
        }
    }
}
