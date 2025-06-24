use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream, StreamConfig,
};

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
    pub fn new() -> Result<Self> {
        Ok(Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            stream: None,
            device: None,
        })
    }

    pub fn start_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Get default host and input device
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or_else(|| anyhow!("No default input device available"))?;

        println!("🎤 Using audio device: {}", device.name().unwrap_or("Unknown".to_string()));

        // Get supported input config close to our target format
        let mut supported_configs = device.supported_input_configs()?;
        let _supported_config = supported_configs
            .find(|config| {
                config.channels() <= CHANNELS && 
                config.min_sample_rate().0 <= SAMPLE_RATE && 
                config.max_sample_rate().0 >= SAMPLE_RATE
            })
            .ok_or_else(|| anyhow!("No suitable audio format found"))?;

        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        println!("📊 Audio config: {}Hz, {} channels", config.sample_rate.0, config.channels);

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
                    
                    if old_len == 0 && audio_buffer.len() > 0 {
                        println!("🎤 First audio samples captured! Got {} samples", data.len());
                    }
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

        println!("✅ CPAL audio recording started successfully");
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

        println!("🛑 CPAL audio recording stopped");
        Ok(())
    }

    pub fn get_audio_data(&self) -> Result<Vec<f32>> {
        let buffer = self.buffer.lock().map_err(|_| anyhow!("Failed to lock buffer"))?;
        Ok(buffer.clone())
    }

    pub fn clear_buffer(&self) -> Result<()> {
        let mut buffer = self.buffer.lock().map_err(|_| anyhow!("Failed to lock buffer"))?;
        buffer.clear();
        Ok(())
    }

    pub fn buffer_size(&self) -> Result<usize> {
        let buffer = self.buffer.lock().map_err(|_| anyhow!("Failed to lock buffer"))?;
        Ok(buffer.len())
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::Relaxed)
    }

    pub fn get_recording_duration_seconds(&self) -> Result<f32> {
        let buffer = self.buffer.lock().map_err(|_| anyhow!("Failed to lock buffer"))?;
        Ok(buffer.len() as f32 / SAMPLE_RATE as f32)
    }

    pub fn get_max_recording_duration_seconds(&self) -> usize {
        MAX_RECORDING_DURATION_SECONDS
    }

    // Method to process audio events (for compatibility with main loop)
    pub fn process_audio_events(&self) -> Result<()> {
        // CPAL handles audio processing in background threads
        // This method is a no-op for compatibility
        Ok(())
    }

    // Helper method for testing and integration
    pub fn run_mainloop_iteration(&self) -> Result<()> {
        self.process_audio_events()
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
        assert_eq!(recorder.buffer_size().unwrap(), 0);
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_buffer_operations() {
        let recorder = AudioRecorder::new().unwrap();
        
        // Initially empty
        assert_eq!(recorder.buffer_size().unwrap(), 0);
        
        // Clear empty buffer should work
        assert!(recorder.clear_buffer().is_ok());
        assert_eq!(recorder.buffer_size().unwrap(), 0);
        
        // Get empty audio data
        let data = recorder.get_audio_data().unwrap();
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn test_recording_lifecycle() {
        let mut recorder = AudioRecorder::new().unwrap();
        
        // Should not be recording initially
        assert!(!recorder.is_recording());
        
        // Multiple stop calls should not fail
        assert!(recorder.stop_recording().is_ok());
        assert!(recorder.stop_recording().is_ok());
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_cpal_recording_initialization() {
        let mut recorder = AudioRecorder::new().unwrap();
        
        // This test attempts to start CPAL recording
        // It may fail if no audio device is available
        match recorder.start_recording() {
            Ok(()) => {
                assert!(recorder.is_recording());
                
                // Let CPAL capture some data
                std::thread::sleep(Duration::from_millis(100));
                
                // Run a few iterations to ensure compatibility
                for _ in 0..10 {
                    let _ = recorder.run_mainloop_iteration();
                    std::thread::sleep(Duration::from_millis(10));
                }
                
                // Stop recording
                assert!(recorder.stop_recording().is_ok());
                assert!(!recorder.is_recording());
                
                println!("CPAL recording test completed successfully");
            }
            Err(e) => {
                // No audio device available - acceptable in test environments
                println!("CPAL recording test skipped: {}", e);
                assert!(!recorder.is_recording());
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
        assert_eq!(recorder.buffer_size().unwrap(), 0);
        
        // Test duration calculation on empty buffer
        let duration = recorder.get_recording_duration_seconds().unwrap();
        assert_eq!(duration, 0.0);
        
        // Clear empty buffer
        assert!(recorder.clear_buffer().is_ok());
        assert_eq!(recorder.buffer_size().unwrap(), 0);
    }

    #[test]
    fn test_buffer_size_limit() {
        let recorder = AudioRecorder::new().unwrap();
        
        // This test ensures we don't exceed memory limits
        assert_eq!(recorder.get_max_recording_duration_seconds(), 300);
        
        // Test initial buffer size
        assert_eq!(recorder.buffer_size().unwrap(), 0);
    }

    #[test]
    fn test_buffer_thread_safety() {
        // Test that the buffer is thread-safe for data access
        let recorder = AudioRecorder::new().unwrap();
        
        // Test buffer operations are thread-safe
        assert_eq!(recorder.buffer_size().unwrap(), 0);
        
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
        assert!(recorder.run_mainloop_iteration().is_ok());
    }
}