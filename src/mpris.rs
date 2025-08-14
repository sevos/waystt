use crate::{config::Config, socket};
use anyhow::{anyhow, Result};
use mpris_server::zbus::zvariant::ObjectPath;
use mpris_server::{Metadata, PlaybackStatus, Player, Time, TrackId};
use std::sync::{Arc, Mutex};

/// Tracks the current transcription state
#[derive(Debug, Clone)]
struct TranscriptionState {
    is_active: bool,
    start_time: Option<std::time::Instant>,
}

impl TranscriptionState {
    fn new() -> Self {
        Self {
            is_active: false,
            start_time: None,
        }
    }

    fn start(&mut self) {
        self.is_active = true;
        self.start_time = Some(std::time::Instant::now());
    }

    fn stop(&mut self) {
        self.is_active = false;
        self.start_time = None;
    }

    fn toggle(&mut self) {
        if self.is_active {
            self.stop();
        } else {
            self.start();
        }
    }

    fn get_position(&self) -> Time {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed();
            Time::from_millis(elapsed.as_millis() as i64)
        } else {
            Time::from_millis(0)
        }
    }

    fn get_playback_status(&self) -> PlaybackStatus {
        if self.is_active {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Stopped
        }
    }
}

/// Sends a `StopTranscription` command to the daemon.
async fn send_stop_command() -> Result<bool> {
    eprintln!("[MPRIS] Stopping transcription");

    // Create the StopTranscription command.
    let command = socket::Command::StopTranscription;

    // Send the command to the daemon.
    match socket::send_command(&command).await {
        Ok(socket::Response::Success { message }) => {
            eprintln!("[MPRIS] Successfully sent stop command: {}", message);
            Ok(true)
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

/// Sends a `ToggleTranscription` command to the daemon for the specified profile.
async fn send_toggle_command(profile_name: &str) -> Result<bool> {
    eprintln!(
        "[MPRIS] Toggling transcription for profile '{}'",
        profile_name
    );

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
            Ok(true)
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

/// Updates MPRIS properties based on transcription state
async fn update_mpris_properties(
    player: &Player,
    state: &Arc<Mutex<TranscriptionState>>,
    profile_name: &str,
) -> Result<()> {
    let (playback_status, position, is_active) = {
        let state_guard = state.lock().unwrap();
        (
            state_guard.get_playback_status(),
            state_guard.get_position(),
            state_guard.is_active,
        )
    };

    // Update playback status
    player.set_playback_status(playback_status).await?;

    // Update metadata
    let mut metadata = Metadata::new();
    if is_active {
        metadata.set_title(Some("Recording"));
        metadata.set_artist(Some(vec!["☎️ Hotline".to_string()]));
        metadata.set_album(Some(format!("Profile: {}", profile_name)));
        metadata.set_album_artist(Some(vec!["☎️ Hotline".to_string()]));
        metadata.set_genre(Some(vec!["Speech-to-Text".to_string()]));
        metadata.set_comment(Some(vec!["Recording in progress".to_string()]));
        // Set a dummy track ID for active recording
        metadata.set_trackid(Some(TrackId::from(
            ObjectPath::try_from("/org/mpris/MediaPlayer2/hotline_stt/track/recording").unwrap(),
        )));
    } else {
        metadata.set_title(Some("Ready"));
        metadata.set_artist(Some(vec!["☎️ Hotline".to_string()]));
        metadata.set_album(Some(format!("Profile: {}", profile_name)));
        metadata.set_album_artist(Some(vec!["☎️ Hotline".to_string()]));
        metadata.set_genre(Some(vec!["Speech-to-Text".to_string()]));
        metadata.set_comment(Some(vec!["Press play to start".to_string()]));
        // Set a dummy track ID for ready state
        metadata.set_trackid(Some(TrackId::from(
            ObjectPath::try_from("/org/mpris/MediaPlayer2/hotline_stt/track/ready").unwrap(),
        )));
    }
    player.set_metadata(metadata).await?;

    // Update position
    player.set_position(position);

    eprintln!(
        "[MPRIS] Updated properties - Status: {:?}, Position: {:?}",
        playback_status, position
    );
    Ok(())
}

/// Initializes and runs the MPRIS D-Bus server.
pub async fn run_mpris_server(profile_name: String) -> Result<()> {
    eprintln!(
        "[MPRIS] Starting MPRIS server for profile '{}'",
        profile_name
    );

    // Initialize transcription state
    let state = Arc::new(Mutex::new(TranscriptionState::new()));

    // Build the MPRIS player with full capabilities.
    let player: Player = Player::builder("org.mpris.MediaPlayer2.hotline_stt")
        .identity("☎️ Hotline")
        .desktop_entry("hotline")
        .can_play(true)
        .can_pause(true)
        .can_go_next(false)
        .can_go_previous(false)
        .can_seek(false)
        .can_control(true)
        .can_quit(true)
        .can_raise(false)
        .has_track_list(false)
        .can_set_fullscreen(false)
        .supported_uri_schemes(vec!["file".to_string()])
        .supported_mime_types(vec!["audio/wav".to_string(), "audio/mp3".to_string()])
        .build()
        .await?;

    // Set initial metadata
    let mut initial_metadata = Metadata::new();
    initial_metadata.set_title(Some("Ready"));
    initial_metadata.set_artist(Some(vec!["☎️ Hotline".to_string()]));
    initial_metadata.set_album(Some(format!("Profile: {}", profile_name)));
    initial_metadata.set_album_artist(Some(vec!["☎️ Hotline".to_string()]));
    initial_metadata.set_genre(Some(vec!["Speech-to-Text".to_string()]));
    initial_metadata.set_comment(Some(vec!["Press play to start".to_string()]));
    initial_metadata.set_trackid(Some(TrackId::from(
        ObjectPath::try_from("/org/mpris/MediaPlayer2/hotline_stt/track/ready").unwrap(),
    )));
    player.set_metadata(initial_metadata).await?;
    player.set_playback_status(PlaybackStatus::Stopped).await?;
    player.set_position(Time::from_millis(0));

    // Set additional properties that clients expect
    player.set_volume(1.0).await?; // Full volume
    player.set_rate(1.0).await?; // Normal playback rate
    player.set_minimum_rate(1.0).await?;
    player.set_maximum_rate(1.0).await?;

    // Clone profile_name and state for use in handlers.
    let profile_name_clone = profile_name.clone();

    // MediaPlayer2 interface handlers

    // Handle Quit method
    player.connect_quit(|_player| {
        eprintln!("[MPRIS] Quit command received - this is a no-op for hotline");
    });

    // Handle Raise method
    player.connect_raise(|_player| {
        eprintln!("[MPRIS] Raise command received - this is a no-op for hotline");
    });

    // MediaPlayer2.Player interface handlers

    // Handle Play method (should toggle like PlayPause)
    {
        let profile_name = profile_name_clone.clone();
        let state = state.clone();
        player.connect_play(move |_player| {
            eprintln!("[MPRIS] Play command received (toggle behavior)");
            let profile_name = profile_name.clone();
            let state = state.clone();
            tokio::spawn(async move {
                match send_toggle_command(&profile_name).await {
                    Ok(_) => {
                        let mut state_guard = state.lock().unwrap();
                        state_guard.toggle();
                        eprintln!(
                            "[MPRIS] Successfully toggled transcription via Play, state updated"
                        );
                    }
                    Err(e) => {
                        eprintln!("[MPRIS] Error toggling transcription via Play: {}", e);
                    }
                }
            });
        });
    }

    // Handle Pause method (should toggle like PlayPause)
    {
        let profile_name = profile_name_clone.clone();
        let state = state.clone();
        player.connect_pause(move |_player| {
            eprintln!("[MPRIS] Pause command received (toggle behavior)");
            let profile_name = profile_name.clone();
            let state = state.clone();
            tokio::spawn(async move {
                match send_toggle_command(&profile_name).await {
                    Ok(_) => {
                        let mut state_guard = state.lock().unwrap();
                        state_guard.toggle();
                        eprintln!(
                            "[MPRIS] Successfully toggled transcription via Pause, state updated"
                        );
                    }
                    Err(e) => {
                        eprintln!("[MPRIS] Error toggling transcription via Pause: {}", e);
                    }
                }
            });
        });
    }

    // Handle PlayPause method
    {
        let profile_name = profile_name_clone.clone();
        let state = state.clone();
        player.connect_play_pause(move |_player| {
            eprintln!("[MPRIS] PlayPause command received");
            let profile_name = profile_name.clone();
            let state = state.clone();
            tokio::spawn(async move {
                match send_toggle_command(&profile_name).await {
                    Ok(_) => {
                        let mut state_guard = state.lock().unwrap();
                        state_guard.toggle();
                        eprintln!("[MPRIS] Successfully toggled transcription, state updated");
                    }
                    Err(e) => {
                        eprintln!("[MPRIS] Error toggling transcription: {}", e);
                    }
                }
            });
        });
    }

    // Handle Stop method (should actually stop, not toggle)
    {
        let state = state.clone();
        player.connect_stop(move |_player| {
            eprintln!("[MPRIS] Stop command received (force stop)");
            let state = state.clone();
            tokio::spawn(async move {
                match send_stop_command().await {
                    Ok(_) => {
                        let mut state_guard = state.lock().unwrap();
                        state_guard.stop();
                        eprintln!("[MPRIS] Successfully stopped transcription, state updated");
                    }
                    Err(e) => {
                        eprintln!("[MPRIS] Error stopping transcription: {}", e);
                    }
                }
            });
        });
    }

    // Handle Next method (no-op for hotline)
    player.connect_next(|_player| {
        eprintln!("[MPRIS] Next command received - this is a no-op for hotline");
    });

    // Handle Previous method (no-op for hotline)
    player.connect_previous(|_player| {
        eprintln!("[MPRIS] Previous command received - this is a no-op for hotline");
    });

    // Handle Seek method (no-op for hotline)
    player.connect_seek(|_player, offset| {
        eprintln!(
            "[MPRIS] Seek command received with offset: {:?} - this is a no-op for hotline",
            offset
        );
    });

    // Handle SetPosition method (no-op for hotline)
    player.connect_set_position(|_player, track_id, position| {
        eprintln!("[MPRIS] SetPosition command received for track {:?} at position {:?} - this is a no-op for hotline", track_id, position);
    });

    // Handle OpenUri method (no-op for hotline)
    player.connect_open_uri(|_player, uri| {
        eprintln!(
            "[MPRIS] OpenUri command received for URI: {} - this is a no-op for hotline",
            uri
        );
    });

    // Handle SetLoopStatus method (no-op for hotline)
    player.connect_set_loop_status(|_player, loop_status| {
        eprintln!(
            "[MPRIS] SetLoopStatus command received: {:?} - this is a no-op for hotline",
            loop_status
        );
    });

    // Handle SetRate method (no-op for hotline)
    player.connect_set_rate(|_player, rate| {
        eprintln!(
            "[MPRIS] SetRate command received: {:?} - this is a no-op for hotline",
            rate
        );
    });

    // Handle SetShuffle method (no-op for hotline)
    player.connect_set_shuffle(|_player, shuffle| {
        eprintln!(
            "[MPRIS] SetShuffle command received: {} - this is a no-op for hotline",
            shuffle
        );
    });

    // Handle SetVolume method (no-op for hotline)
    player.connect_set_volume(|_player, volume| {
        eprintln!(
            "[MPRIS] SetVolume command received: {:?} - this is a no-op for hotline",
            volume
        );
    });

    // Handle SetFullscreen method (no-op for hotline)
    player.connect_set_fullscreen(|_player, fullscreen| {
        eprintln!(
            "[MPRIS] SetFullscreen command received: {} - this is a no-op for hotline",
            fullscreen
        );
    });

    eprintln!("[MPRIS] Server is running. Waiting for MPRIS events...");
    eprintln!("[MPRIS] Supported commands: Play, Pause, PlayPause, Stop, Next, Previous, Seek, SetPosition, OpenUri, SetLoopStatus, SetRate, SetShuffle, SetVolume, Quit, Raise, SetFullscreen");
    eprintln!("[MPRIS] Properties will be updated based on transcription state");

    // Create a LocalSet to run the player task
    let local = tokio::task::LocalSet::new();

    // Run the player and property update tasks concurrently
    local
        .run_until(async {
            // Spawn the player run task
            tokio::task::spawn_local(player.run());

            // Main loop to update MPRIS properties periodically
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                interval.tick().await;

                // Update MPRIS properties based on current state
                if let Err(e) = update_mpris_properties(&player, &state, &profile_name).await {
                    eprintln!("[MPRIS] Failed to update properties: {}", e);
                }
            }
        })
        .await;

    Ok(())
}
