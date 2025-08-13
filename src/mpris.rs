use anyhow::{anyhow, Result};
use mpris_server::Player;
use crate::{config::Config, socket};

/// Sends a `ToggleTranscription` command to the daemon for the specified profile.
async fn send_toggle_command(profile_name: &str) -> Result<()> {
    eprintln!("[MPRIS] Toggling transcription for profile '{}'", profile_name);

    // Load configuration to find the specified profile.
    let config = Config::load_with_precedence()?;
    let profile = config
        .get_profile(profile_name)
        .ok_or_else(|| anyhow!("Profile '{}' not found in configuration", profile_name))?;

    // Create the ToggleTranscription command.
    let command = socket::Command::ToggleTranscription(socket::StartTranscriptionArgs {
        model: profile.model.clone(),
        language: profile.language.clone(),
        prompt: profile.prompt.clone(),
        vad_config: profile.vad_config.clone(),
        hooks: profile.hooks.clone(),
    });

    // Send the command to the daemon.
    match socket::send_command(&command).await {
        Ok(socket::Response::Success { message }) => {
            eprintln!("[MPRIS] Successfully sent toggle command: {}", message);
            Ok(())
        }
        Ok(socket::Response::Error { message }) => {
            eprintln!("[MPRIS] Error from daemon: {}", message);
            Err(anyhow!("Daemon returned an error: {}", message))
        }
        Err(e) => {
            eprintln!("[MPRIS] Failed to send command to daemon: {}", e);
            Err(e)
        }
    }
}

/// Initializes and runs the MPRIS D-Bus server.
pub async fn run_mpris_server(profile_name: String) -> Result<()> {
    eprintln!("[MPRIS] Starting MPRIS server for profile '{}'", profile_name);

    // Build the MPRIS player.
    let player: Player = Player::builder("org.mpris.MediaPlayer2.hotline_stt")
        .identity("HotLine STT")
        .can_play(true)
        .can_pause(true)
        .build()
        .await?;

    // Clone profile_name for use in the handler.
    let profile_name_clone = profile_name.clone();

    // Handle the PlayPause method call.
    player.connect_play_pause(move |_player| {
        let profile_name = profile_name_clone.clone();
        tokio::spawn(async move {
            if let Err(e) = send_toggle_command(&profile_name).await {
                eprintln!("[MPRIS] Error toggling transcription: {}", e);
            }
        });
    });

    // Run the player's event handler task.
    let run_task = player.run();

    eprintln!("[MPRIS] Server is running. Waiting for Play/Pause events...");

    // Keep the process alive.
    run_task.await;

    Ok(())
}
