//! waystt library entrypoint and public exports.
//!
//! This crate exposes a library-first API with a thin binary wrapper.
//! It coordinates configuration bootstrap, provider creation, and running the app.

use anyhow::Result;

pub mod app;
pub mod cli;
pub mod config;
pub mod pipeline;
pub mod signals;

// Re-export existing modules for backward-compatibility and tests
pub mod audio;
pub mod audio_processing;
pub mod beep;
pub mod command;
#[cfg(test)]
pub mod test_utils;
pub mod transcription;
pub mod wav;

/// Run the application given CLI-level `RunOptions`.
/// Returns a process exit code.
pub async fn run(options: cli::RunOptions) -> Result<i32> {
    // Bootstrap configuration
    let config = crate::config::bootstrap(options.envfile.as_deref())?;

    // Handle model download early and exit if requested
    if options.download_model {
        let model = &config.whisper_model;
        let path = crate::config::Config::model_path(model);
        // If the model already exists, consider it success and exit 0
        if !path.exists() {
            // Download via existing helper in app layer (reuse current behavior)
            // We keep the simple inline implementation here to avoid public API churn.
            let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
            let url = format!("{}/{}", base_url, model);
            let dir = crate::config::Config::model_dir();
            tokio::fs::create_dir_all(&dir).await?;
            let resp = reqwest::get(&url).await?;
            if !resp.status().is_success() {
                anyhow::bail!("Failed to download model: {}", resp.status());
            }
            let bytes = resp.bytes().await?;
            tokio::fs::write(&path, &bytes).await?;
        }
        eprintln!("Model available at {}", path.display());
        return Ok(0);
    }

    // Create provider using explicit config injection
    let kind = config.provider_kind();
    let provider =
        crate::transcription::TranscriptionFactory::create_provider(kind, &config).await?;

    // Create app and run
    let app = crate::app::App::init(options, config, provider).await?;
    app.run().await
}
