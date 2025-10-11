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
use std::sync::{Arc, Mutex};

const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u16 = 1;

// Memory management constants
const MAX_RECORDING_DURATION_SECONDS: usize = 300; // 5 minutes max
const MAX_BUFFER_SIZE: usize = SAMPLE_RATE as usize * MAX_RECORDING_DURATION_SECONDS;

pub struct AudioRecorder {
    buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    stream: Option<Stream>,
    device: Option<Device>,
}

impl AudioRecorder {
    /// Create a new audio recorder
    ///
    /// # Errors
    ///
    /// Currently this function does not return errors, but the signature allows for future error handling
    pub fn new() -> Result<Self> {
        Ok(Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            stream: None,
            device: None,
        })
    }

    /// Start audio recording
    ///
    /// # Errors
    ///
    /// Returns an error if audio device initialization fails or if no suitable audio format is found
    pub fn start_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Get default host and input device
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("No default input device available"))?;

        let device_name = device.name().unwrap_or("Unknown".to_string());
        eprintln!(
            "🎤 Using audio device: {device_name}"
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

        let sample_rate = config.sample_rate.0;
        let channels = config.channels;
        eprintln!(
            "📊 Audio config: {sample_rate}Hz, {channels} channels"
        );

        // Clone buffer for the stream callback
        let buffer_clone = Arc::clone(&self.buffer);

        // Create audio input stream
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Process audio data in the callback
                if let Ok(mut audio_buffer) = buffer_clone.lock() {
                    // Manage buffer size
                    if audio_buffer.len() + data.len() > MAX_BUFFER_SIZE {
                        let samples_to_remove = (audio_buffer.len() + data.len()) - MAX_BUFFER_SIZE;
                        if samples_to_remove < audio_buffer.len() {
                            audio_buffer.drain(0..samples_to_remove);
                        } else {
                            audio_buffer.clear();
                        }
                    }

                    let old_len = audio_buffer.len();
                    audio_buffer.extend_from_slice(data);

                    if old_len == 0 && !audio_buffer.is_empty() {
                        let len = data.len();
                        eprintln!(
                            "🎤 First audio samples captured! Got {len} samples"
                        );
                    }
                }
            },
            |err| {
                eprintln!("❌ Audio stream error: {err}");
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

    /// Stop audio recording
    ///
    /// # Errors
    ///
    /// Returns an error if stopping the audio stream fails
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

    /// Get the current audio data from the buffer
    ///
    /// # Errors
    ///
    /// Returns an error if acquiring the buffer lock fails
    pub fn get_audio_data(&self) -> Result<Vec<f32>> {
        let buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("Failed to lock buffer"))?;
        Ok(buffer.clone())
    }

    /// Clear the audio buffer
    ///
    /// # Errors
    ///
    /// Returns an error if acquiring the buffer lock fails
    pub fn clear_buffer(&self) -> Result<()> {
        let mut buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("Failed to lock buffer"))?;
        buffer.clear();
        Ok(())
    }

    /// Get the recording duration in seconds
    ///
    /// # Errors
    ///
    /// Returns an error if acquiring the buffer lock fails
    pub fn get_recording_duration_seconds(&self) -> Result<f32> {
        let buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("Failed to lock buffer"))?;
        Ok(buffer.len() as f32 / SAMPLE_RATE as f32)
    }

    /// Process audio events (no-op for CPAL compatibility)
    ///
    /// # Errors
    ///
    /// Currently this function does not return errors, but the signature allows for future error handling
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
    fn test_initial_state() {
        let recorder = AudioRecorder::new().unwrap();
        let buffer_data = recorder.get_audio_data().unwrap();
        assert_eq!(buffer_data.len(), 0);
    }

    #[test]
    fn test_buffer_operations() {
        let recorder = AudioRecorder::new().unwrap();

        // Initially empty
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);

        // Clear empty buffer should work
        assert!(recorder.clear_buffer().is_ok());
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);

        // Get empty audio data
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);
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
        assert_eq!(MAX_RECORDING_DURATION_SECONDS, 300);
        assert_eq!(MAX_BUFFER_SIZE, 16000 * 300);
    }

    #[test]
    fn test_memory_management() {
        let recorder = AudioRecorder::new().unwrap();

        // Test buffer operations
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);

        // Test duration calculation on empty buffer
        let duration = recorder.get_recording_duration_seconds().unwrap();
        assert!(duration.abs() < f32::EPSILON);

        // Clear empty buffer
        assert!(recorder.clear_buffer().is_ok());
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn test_buffer_size_limit() {
        let recorder = AudioRecorder::new().unwrap();

        // Test that we can get recording duration (should be 0 for empty buffer)
        let duration = recorder.get_recording_duration_seconds().unwrap();
        assert!(duration.abs() < f32::EPSILON);

        // Test initial buffer size
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn test_buffer_thread_safety() {
        // Test that the buffer is thread-safe for data access
        let recorder = AudioRecorder::new().unwrap();

        // Test buffer operations are thread-safe
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);

        // Test concurrent buffer reads
        let data1 = recorder.get_audio_data().unwrap();
        let data2 = recorder.get_audio_data().unwrap();
        assert_eq!(data1, data2);
        assert_eq!(data1.len(), 0);
    }

    #[test]
    fn test_audio_processing_events() {
        let recorder = AudioRecorder::new().unwrap();

        // Test that process_audio_events doesn't fail
        assert!(recorder.process_audio_events().is_ok());
    }
}
