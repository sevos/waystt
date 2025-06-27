//! Shared test utilities to prevent race conditions between test modules

use std::sync::Mutex;
use tokio::sync::Mutex as AsyncMutex;

// Global sync mutex for synchronous tests
pub static ENV_MUTEX: Mutex<()> = Mutex::new(());

// Global async mutex for async tests that need to hold lock across await points
pub static ASYNC_ENV_MUTEX: AsyncMutex<()> = AsyncMutex::const_new(());
