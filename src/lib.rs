//! waystt library entrypoint and public exports.
//!
//! This crate exposes a library-first API with a thin binary wrapper.
//! It coordinates configuration bootstrap, provider creation, and running the app.

use anyhow::Result;
use futures::stream::StreamExt;
use std::io::Write;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

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
///
/// # Errors
///
/// Returns an error if configuration bootstrap fails, model download fails, or application initialization fails
pub async fn run(options: cli::RunOptions) -> Result<i32> {
    // Bootstrap configuration
    let config = crate::config::bootstrap(options.envfile.as_deref())?;

    // Handle model download early and exit if requested
    if options.download_model {
        let model = &config.whisper_model;
        let path = crate::config::Config::model_path(model);
        // If the model already exists, consider it success and exit 0
        if !path.exists() {
            download_model_with_progress(model).await?;
        }
        let display_path = path.display();
        eprintln!("Model available at {display_path}");
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

/// Download a model with progress tracking.
///
/// Shows real-time download progress including percentage, speed (MB/s), and ETA.
///
/// # Errors
///
/// Returns an error if the download fails, the HTTP request fails, or file writing fails
#[allow(clippy::cast_precision_loss)]
async fn download_model_with_progress(model: &str) -> Result<std::path::PathBuf> {
    let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
    let url = format!("{base_url}/{model}");
    let dir = crate::config::Config::model_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(model);

    let resp = reqwest::get(&url).await?;
    if !resp.status().is_success() {
        let status = resp.status();
        anyhow::bail!("Failed to download model: {status}");
    }

    let total_size = resp.content_length();
    let mut file = tokio::fs::File::create(&path).await?;
    let mut stream = resp.bytes_stream();

    let mut downloaded = 0u64;
    let start_time = Instant::now();

    print!("{model}... ");
    std::io::stdout().flush()?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("Download error: {e}"))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if let Some(total) = total_size {
            let percentage = (downloaded as f64 / total as f64) * 100.0;
            let elapsed = start_time.elapsed().as_secs_f64();

            if elapsed > 0.0 {
                let speed = downloaded as f64 / elapsed / 1024.0 / 1024.0; // MB/s
                let eta = if speed > 0.0 {
                    (total - downloaded) as f64 / (speed * 1024.0 * 1024.0)
                } else {
                    0.0
                };

                print!(
                    "\r{model}... {percentage:.1}% ({speed:.1} MB/s, ETA: {eta:.0}s)    "
                );
                std::io::stdout().flush()?;
            }
        } else {
            print!(
                "\r{model}... {:.1} MB downloaded    ",
                downloaded as f64 / 1024.0 / 1024.0
            );
            std::io::stdout().flush()?;
        }
    }

    file.flush().await?;
    Ok(path)
}
