//! CLI-facing options decoupled from parsing.
//! The actual clap parsing lives in the binary and maps into this struct.

use std::path::PathBuf;

/// Options passed from the CLI into the library entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub envfile: Option<PathBuf>,
    pub pipe_to: Option<Vec<String>>,
    pub download_model: bool,
}

/// Default path for the env file.
pub fn default_envfile_path() -> PathBuf {
    crate::config::default_envfile()
}
