//! Shared test utilities to prevent race conditions between test modules

use tokio::sync::Mutex as AsyncMutex;

// Global unified mutex for all environment variable tests to prevent race conditions
// This single async mutex ensures that both sync and async tests using environment variables
// cannot run simultaneously, preventing interference between tests.
pub static ENV_MUTEX: AsyncMutex<()> = AsyncMutex::const_new(());
