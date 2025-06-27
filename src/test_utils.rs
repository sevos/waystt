//! Shared test utilities to prevent race conditions between test modules

use std::sync::Mutex;

// Global mutex to ensure tests that modify environment variables run sequentially
pub static ENV_MUTEX: Mutex<()> = Mutex::new(());
