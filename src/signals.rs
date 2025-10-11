//! Signal utilities and constants.

pub const TRANSCRIBE_SIG: i32 = signal_hook::consts::SIGUSR1;
pub const SHUTDOWN_SIG: i32 = signal_hook::consts::SIGTERM;

/// Build a signal stream for async handling of signals used by the app.
///
/// # Errors
///
/// Returns an error if signal registration fails
pub fn build_signal_stream() -> anyhow::Result<signal_hook_tokio::Signals> {
    use signal_hook::consts::signal::{SIGTERM, SIGUSR1};
    let signals = signal_hook_tokio::Signals::new([SIGUSR1, SIGTERM])?;
    Ok(signals)
}
