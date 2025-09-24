## Project Overview

waystt is a Wayland speech-to-text tool that outputs transcribed text to stdout:
- **SIGUSR1**: Transcribe audio and output text to stdout (for piping to other tools)

## Audio Feedback System

Configuration:
- `ENABLE_AUDIO_FEEDBACK=true/false` - Enable/disable beeps
- `BEEP_VOLUME=0.0-1.0` - Volume control (default: 0.1)

## Testing

### Environment Variables and Race Conditions

**Critical**: Tests that modify environment variables must use proper mutex protection to prevent race conditions when running in parallel.

#### Test Mutex System
The project uses a dual mutex system in `src/test_utils.rs`:

- `ENV_MUTEX` (sync): For synchronous tests
- `ASYNC_ENV_MUTEX` (async): For async tests that need to hold locks across await points

#### Synchronous Test Pattern
```rust
#[test]
fn test_name() {
    let _lock = ENV_MUTEX.lock().unwrap();
    
    // Save current environment state
    let original_value = std::env::var("ENV_VAR").ok();
    
    // Modify environment for test
    std::env::set_var("ENV_VAR", "test-value");
    
    // Run test logic
    let result = some_function();
    
    // Restore environment state
    if let Some(value) = original_value {
        std::env::set_var("ENV_VAR", value);
    } else {
        std::env::remove_var("ENV_VAR");
    }
    
    // Assertions
    assert!(result.is_ok());
}
```

#### Async Test Pattern
```rust
#[tokio::test]
async fn test_name() {
    #[allow(clippy::await_holding_lock)]
    {
        let _lock = ASYNC_ENV_MUTEX.lock().await;
        
        // Save current environment state
        let original_value = std::env::var("ENV_VAR").ok();
        
        // Modify environment for test
        std::env::set_var("ENV_VAR", "test-value");
        
        // Run async test logic
        let result = some_async_function().await;
        
        // Restore environment state
        if let Some(value) = original_value {
            std::env::set_var("ENV_VAR", value);
        } else {
            std::env::remove_var("ENV_VAR");
        }
        
        // Assertions
        assert!(result.is_ok());
    }
}
```

#### Key Principles
1. **Entire test must be protected**: Hold the mutex for the complete test duration, not just environment manipulation
2. **Always save and restore**: Capture original environment state and restore it after the test
3. **Pedantic lint compliance**: Use `#[allow(clippy::await_holding_lock)]` for async tests - this is intentional and necessary
4. **Import from test_utils**: `use crate::test_utils::{ENV_MUTEX, ASYNC_ENV_MUTEX};`

#### Why This Approach
- **Prevents race conditions**: No gaps between environment setup and async operations
- **Proper test isolation**: Each test has exclusive access to environment variables
- **Parallel execution safe**: Tests can run in parallel without interfering with each other
- **Lint compliant**: Explicitly acknowledges that holding async locks is intentional for test correctness

### Running Tests
- Always set the beep volume to 0, when running tests `BEEP_VOLUME=0.0 cargo test...`
- When developing/testing, use `--envfile .env` to use the project-local .env file instead of ~/.config/waystt/.env
- Example: `BEEP_VOLUME=0.0 cargo run -- --envfile .env`

### GPU Acceleration
The project supports optional GPU acceleration for local whisper transcription:

#### Build Options
- **CPU only** (default): `cargo build --release --bin waystt`
- **AMD GPU**: `cargo build --release --features vulkan --bin waystt-vulkan`

This creates separate binaries:
- `target/release/waystt` - CPU-only, minimal
- `target/release/waystt-vulkan` - AMD GPU support

#### Configuration
Set these environment variables in your `.env` file:
```bash
TRANSCRIPTION_PROVIDER=local
WHISPER_USE_GPU=true
WHISPER_GPU_DEVICE=0  # GPU device ID
```

#### Prerequisites
- **AMD**: Install `vulkan-devel/libvulkan-dev` `vulkan-tools`

## QA Testing Workflow

- For QAing, run the app with `nohup` and `&` to properly detach from terminal:
  ```bash
  # Using production config (~/.config/waystt/.env)
  nohup ./target/release/waystt > /tmp/waystt.log 2>&1 & disown
  
  # Or during development using project-local .env file
  nohup ./target/release/waystt --envfile .env > /tmp/waystt.log 2>&1 & disown
  ```
- Then:
  - Listen for "ding dong" sound confirming recording started
  - Ask the user to speak something
  - Wait 5 seconds
  - Run `pkill --signal SIGUSR1 waystt` to trigger transcription and stdout output
  - Listen for "dong ding" (recording stopped) then "ding ding" (success) sounds
  - Check logs with `tail /tmp/waystt.log`
  - The transcribed text will be output to stdout and can be captured or piped to other tools

## Configuration Files

Key files for future development:
- `src/main.rs`: Main application logic, signal handling, audio feedback integration
- `src/beep.rs`: Musical audio feedback system with CPAL
- `src/audio.rs`: Audio recording via PipeWire/CPAL
- `src/config.rs`: Environment variable configuration
- `src/whisper.rs`: OpenAI Whisper API client
- `.env.example`: Configuration template