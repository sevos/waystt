use anyhow::Result;
use signal_hook::consts::{SIGTERM, SIGUSR1, SIGUSR2};
use signal_hook_tokio::Signals;
use futures::stream::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    println!("waystt - Wayland Speech-to-Text Tool");
    println!("Starting audio recording...");
    
    // TODO: Initialize audio recording
    // TODO: Set up signal handlers
    // TODO: Implement main event loop
    
    let mut signals = Signals::new(&[SIGUSR1, SIGUSR2, SIGTERM])?;
    
    println!("Ready. Send SIGUSR1 to transcribe and paste, SIGUSR2 to transcribe and copy.");
    
    while let Some(signal) = signals.next().await {
        match signal {
            SIGUSR1 => {
                println!("Received SIGUSR1: Stop recording, transcribe, and paste");
                // TODO: Implement transcribe and paste logic
                break;
            }
            SIGUSR2 => {
                println!("Received SIGUSR2: Stop recording, transcribe, and copy");
                // TODO: Implement transcribe and copy logic
                break;
            }
            SIGTERM => {
                println!("Received SIGTERM: Shutting down gracefully");
                break;
            }
            _ => {
                println!("Received unexpected signal: {}", signal);
            }
        }
    }
    
    println!("Exiting waystt");
    Ok(())
}
