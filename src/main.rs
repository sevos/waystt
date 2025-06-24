use anyhow::Result;
use signal_hook::consts::{SIGTERM, SIGUSR1, SIGUSR2};
use signal_hook_tokio::Signals;
use futures::stream::StreamExt;

mod audio;
use audio::AudioRecorder;

#[tokio::main]
async fn main() -> Result<()> {
    println!("waystt - Wayland Speech-to-Text Tool");
    println!("Starting audio recording...");
    
    // Initialize audio recorder
    let mut recorder = AudioRecorder::new()?;
    
    // Start recording immediately
    if let Err(e) = recorder.start_recording() {
        eprintln!("Failed to start audio recording: {}", e);
        eprintln!("This may be due to PipeWire not being available or insufficient permissions.");
        return Err(e);
    }
    
    println!("Audio recording started successfully!");
    
    // Give PipeWire a moment to start capturing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    let mut signals = Signals::new(&[SIGUSR1, SIGUSR2, SIGTERM])?;
    
    println!("Ready. Send SIGUSR1 to transcribe and paste, SIGUSR2 to transcribe and copy.");
    
    // Main event loop - process audio and wait for signals
    loop {
        // Process audio events to capture microphone data
        if let Err(e) = recorder.process_audio_events() {
            eprintln!("Error processing audio events: {}", e);
        }

        // Check for signals with timeout
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(50),
            signals.next()
        ).await {
            Ok(Some(signal)) => {
                match signal {
                    SIGUSR1 => {
                        println!("Received SIGUSR1: Stop recording, transcribe, and paste");
                        
                        // Stop recording
                        if let Err(e) = recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {}", e);
                        }
                        
                        // Get recorded audio data
                        match recorder.get_audio_data() {
                            Ok(audio_data) => {
                                let duration = recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                println!("Captured {} audio samples ({:.2} seconds)", audio_data.len(), duration);
                                // TODO: Send to transcription service
                                // TODO: Paste result to active window
                            }
                            Err(e) => {
                                eprintln!("Failed to get audio data: {}", e);
                            }
                        }
                        
                        break;
                    }
                    SIGUSR2 => {
                        println!("Received SIGUSR2: Stop recording, transcribe, and copy");
                        
                        // Stop recording
                        if let Err(e) = recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {}", e);
                        }
                        
                        // Get recorded audio data
                        match recorder.get_audio_data() {
                            Ok(audio_data) => {
                                let duration = recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                println!("Captured {} audio samples ({:.2} seconds)", audio_data.len(), duration);
                                // TODO: Send to transcription service
                                // TODO: Copy result to clipboard
                            }
                            Err(e) => {
                                eprintln!("Failed to get audio data: {}", e);
                            }
                        }
                        
                        break;
                    }
                    SIGTERM => {
                        println!("Received SIGTERM: Shutting down gracefully");
                        if let Err(e) = recorder.stop_recording() {
                            eprintln!("Failed to stop recording: {}", e);
                        }
                        break;
                    }
                    _ => {
                        println!("Received unexpected signal: {}", signal);
                    }
                }
            }
            Ok(None) => {
                // Signal stream ended
                break;
            }
            Err(_) => {
                // Timeout occurred, continue processing audio
                continue;
            }
        }
    }
    
    println!("Exiting waystt");
    Ok(())
}
