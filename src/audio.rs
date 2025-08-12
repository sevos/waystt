#![allow(clippy::cast_precision_loss)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_wraps)]

use anyhow::{anyhow, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u16 = 1;

pub struct AudioRecorder {
    is_recording: Arc<AtomicBool>,
    stream: Option<Stream>,
    device: Option<Device>,
    audio_sender: Option<mpsc::UnboundedSender<Vec<f32>>>,
}

impl AudioRecorder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            stream: None,
            device: None,
            audio_sender: None,
        })
    }

    pub fn set_audio_sender(&mut self, sender: mpsc::UnboundedSender<Vec<f32>>) {
        self.audio_sender = Some(sender);
    }

    pub fn start_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Get default host and input device
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("No default input device available"))?;

        eprintln!(
            "🎤 Using audio device: {}",
            device.name().unwrap_or("Unknown".to_string())
        );

        // Get supported input config close to our target format
        let mut supported_configs = device.supported_input_configs()?;
        let _supported_config = supported_configs
            .find(|config| {
                config.channels() <= CHANNELS
                    && config.min_sample_rate().0 <= SAMPLE_RATE
                    && config.max_sample_rate().0 >= SAMPLE_RATE
            })
            .ok_or_else(|| anyhow!("No suitable audio format found"))?;

        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        eprintln!(
            "📊 Audio config: {}Hz, {} channels",
            config.sample_rate.0, config.channels
        );

        // Clone sender for the stream callback
        let sender_clone = self.audio_sender.clone();

        // Create audio input stream
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Send audio data directly to the channel if available
                if let Some(sender) = &sender_clone {
                    // Send a copy of the data
                    let _ = sender.send(data.to_vec());
                }
            },
            |err| {
                eprintln!("❌ Audio stream error: {}", err);
            },
            None,
        )?;

        // Start the stream
        stream.play()?;

        self.is_recording.store(true, Ordering::Relaxed);
        self.stream = Some(stream);
        self.device = Some(device);

        eprintln!("✅ CPAL audio recording started successfully");
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.is_recording.store(false, Ordering::Relaxed);

        // Stop and drop the stream
        if let Some(stream) = self.stream.take() {
            stream.pause()?;
        }

        self.device.take();

        eprintln!("🛑 CPAL audio recording stopped");
        Ok(())
    }

    // Method to process audio events (for compatibility with main loop)
    pub fn process_audio_events(&self) -> Result<()> {
        // CPAL handles audio processing in background threads
        // This method is a no-op for compatibility
        Ok(())
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        let _ = self.stop_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_audio_recorder_creation() {
        let recorder = AudioRecorder::new();
        assert!(recorder.is_ok());
    }

    #[test]
    fn test_recording_lifecycle() {
        let mut recorder = AudioRecorder::new().unwrap();

        // Multiple stop calls should not fail
        assert!(recorder.stop_recording().is_ok());
        assert!(recorder.stop_recording().is_ok());
    }

    #[test]
    fn test_cpal_recording_initialization() {
        let mut recorder = AudioRecorder::new().unwrap();

        // This test attempts to start CPAL recording
        // It may fail if no audio device is available
        match recorder.start_recording() {
            Ok(()) => {
                // Let CPAL capture some data
                std::thread::sleep(Duration::from_millis(100));

                // Test audio event processing
                for _ in 0..10 {
                    let _ = recorder.process_audio_events();
                    std::thread::sleep(Duration::from_millis(10));
                }

                // Stop recording
                assert!(recorder.stop_recording().is_ok());

                println!("CPAL recording test completed successfully");
            }
            Err(e) => {
                // No audio device available - acceptable in test environments
                println!("CPAL recording test skipped: {}", e);
            }
        }
    }

    #[test]
    fn test_audio_format_constants() {
        assert_eq!(SAMPLE_RATE, 16000);
        assert_eq!(CHANNELS, 1);
    }

    #[test]
    fn test_audio_processing_events() {
        let recorder = AudioRecorder::new().unwrap();

        // Test that process_audio_events doesn't fail
        assert!(recorder.process_audio_events().is_ok());
    }
}
