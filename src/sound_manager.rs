use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::beep::{BeepConfig, BeepPlayer, BeepType};

/// Request to play a sound
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
enum SoundRequest {
    /// Play a standard beep that completes on its own
    PlayBeep {
        beep_type: BeepType,
        response: oneshot::Sender<Result<()>>,
    },
    /// Start playing a beep indefinitely (returns a handle to stop it)
    PlayIndefinite {
        beep_type: BeepType,
        response: oneshot::Sender<Result<SoundHandle>>,
    },
    /// Play a beep and wait for it to complete
    PlayAndWait {
        beep_type: BeepType,
        response: oneshot::Sender<Result<()>>,
    },
}

/// Handle to control an indefinitely playing sound
#[derive(Clone, Debug)]
pub struct SoundHandle {
    stop_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl SoundHandle {
    /// Stop the indefinitely playing sound
    pub async fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().await.take() {
            let _ = tx.send(());
        }
    }
}

/// Centralized sound manager that ensures only one sound plays at a time
#[derive(Clone)]
pub struct SoundManager {
    request_tx: mpsc::UnboundedSender<SoundRequest>,
}

impl SoundManager {
    /// Create a new sound manager with the given beep configuration
    pub fn new(config: BeepConfig) -> Result<Self> {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<SoundRequest>();

        // Spawn the sound processing task
        tokio::spawn(async move {
            let beep_player = match BeepPlayer::new(config) {
                Ok(player) => player,
                Err(e) => {
                    eprintln!("Failed to create beep player: {}", e);
                    return;
                }
            };

            // Process sound requests sequentially
            while let Some(request) = request_rx.recv().await {
                match request {
                    SoundRequest::PlayBeep {
                        beep_type,
                        response,
                    } => {
                        let result = beep_player.play_async(beep_type).await;
                        let _ = response.send(result);
                    }
                    SoundRequest::PlayIndefinite {
                        beep_type,
                        response,
                    } => {
                        let (stop_tx, stop_rx) = oneshot::channel();
                        let handle = SoundHandle {
                            stop_tx: Arc::new(Mutex::new(Some(stop_tx))),
                        };

                        // Start playing indefinitely
                        let player_clone = beep_player.clone();
                        let play_task = tokio::spawn(async move {
                            player_clone.play_indefinite(beep_type, stop_rx).await
                        });

                        let _ = response.send(Ok(handle));

                        // Wait for the sound to complete (either naturally or via stop signal)
                        let _ = play_task.await;
                    }
                    SoundRequest::PlayAndWait {
                        beep_type,
                        response,
                    } => {
                        let result = beep_player.play_async(beep_type).await;
                        let _ = response.send(result);
                    }
                }
            }
        });

        Ok(Self { request_tx })
    }

    /// Play the LineReady sound (fire and forget)
    pub fn play_line_ready(&self) {
        let (response_tx, _) = oneshot::channel();
        let _ = self.request_tx.send(SoundRequest::PlayBeep {
            beep_type: BeepType::LineReady,
            response: response_tx,
        });
    }

    /// Start playing RecordingStart sound indefinitely (returns handle to stop it)
    pub async fn play_recording_start(&self) -> Result<SoundHandle> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(SoundRequest::PlayIndefinite {
                beep_type: BeepType::RecordingStart,
                response: response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Sound manager channel closed"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to get response from sound manager"))?
    }

    /// Play RecordingStop sound and wait for completion
    pub async fn play_recording_stop(&self) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(SoundRequest::PlayAndWait {
                beep_type: BeepType::RecordingStop,
                response: response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Sound manager channel closed"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to get response from sound manager"))?
    }

    /// Play Error sound (queued but non-blocking)
    pub fn play_error(&self) {
        let (response_tx, _) = oneshot::channel();
        let _ = self.request_tx.send(SoundRequest::PlayBeep {
            beep_type: BeepType::Error,
            response: response_tx,
        });
    }
}
