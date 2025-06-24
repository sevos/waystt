# Technology Stack Documentation

## Overview
This document outlines the technology choices for **waystt**, a signal-driven speech-to-text tool built with Rust. The stack prioritizes minimal dependencies, optimal performance, and reliable operation.

## Core Technologies

### Language: Rust (Edition 2021)
**Rationale**: 
- **Memory Safety**: Prevents audio buffer overruns and signal handler race conditions
- **Zero-cost Abstractions**: Minimal runtime overhead for real-time audio processing
- **Single Binary**: No runtime dependencies, easy deployment
- **Excellent Async**: Perfect for handling audio streams and HTTP requests concurrently
- **Signal Handling**: Robust Unix signal support via signal-hook crate

### Architecture: Event-Driven Tool
**Design**: Single-threaded event loop with async I/O
- **Main Thread**: Audio recording loop with signal handling
- **Async Tasks**: HTTP requests for transcription
- **Memory Management**: Circular buffer for audio data
- **State Management**: Simple state machine (Recording → Transcribing → Ready)

## Audio Pipeline

### Audio Capture: CPAL (Cross-Platform)
**Crate**: `cpal`
**Features**:
- Cross-platform audio capture (works with PipeWire, ALSA, PulseAudio)
- Automatic device discovery and format negotiation
- Low-latency audio capture with reliable stream management
- Native integration with Linux audio stack via multiple backends

**Configuration**: Audio settings optimized for Whisper API (16kHz mono, f32 samples)

**Backend Support**: Automatically uses best available backend (PipeWire → ALSA → PulseAudio)

### Audio Processing: Minimal Native
**No external audio libraries needed**:
- **Format**: Record directly to WAV format for API compatibility
- **Buffering**: Circular buffer implementation in safe Rust
- **Encoding**: Simple WAV header generation for API submission


## Signal Handling

### Unix Signals: signal-hook
**Crate**: `signal-hook` + `signal-hook-tokio`
**Signals**:
- **SIGUSR1**: Stop recording, transcribe, paste to active window
- **SIGUSR2**: Stop recording, transcribe, copy to clipboard only  
- **SIGTERM/SIGINT**: Graceful shutdown with buffer cleanup

**Implementation**: Non-blocking signal handling in async context using signal-hook-tokio

## Transcription Services

### Primary: OpenAI Whisper API
**Crate**: `reqwest` (HTTP client)
**Features**:
- **Async HTTP**: Non-blocking API calls
- **Multipart Upload**: Direct WAV file upload
- **Retry Logic**: Exponential backoff for network failures
- **Error Handling**: Comprehensive error types

**API Integration**: Async multipart form upload to OpenAI Whisper API with retry logic

### Fallback: Local Transcription (Optional)
**Crate**: `candle-whisper` or direct `whisper.cpp` bindings
**Models**: Optimized for speed vs accuracy trade-off
- **tiny.en**: Ultra-fast, basic accuracy
- **base.en**: Balanced speed/accuracy
- **small.en**: High accuracy, slower

## Text Injection System

### Primary: Clipboard + Paste
**Approach**: Fastest method for any text length
**Crates**: 
- `wl-clipboard-rs`: Wayland clipboard integration
- `enigo`: Cross-platform input simulation

**Implementation**:
```rust
use wl_clipboard_rs::copy::{MimeType, Options, Source};

async fn inject_text(text: &str) -> Result<()> {
    // 1. Copy to clipboard
    let opts = Options::new();
    opts.copy(Source::Bytes(text.as_bytes().into()), MimeType::Text)?;
    
    // 2. Simulate Ctrl+V
    simulate_paste().await?;
    
    Ok(())
}
```

### Fallback: Direct Text Input
**Crate**: `wayland-client` + `wayland-protocols`
**Protocol**: `text-input-unstable-v3`
**Usage**: When clipboard method fails or is unavailable

## Configuration Management

### Minimal Configuration
**Format**: Environment variables only (no config files)
**Variables**: OpenAI API key, buffer duration, audio device selection, local model choice

**Rationale**: Zero-config operation with sensible defaults

## Dependency Minimization Strategy

### Core Dependencies (Essential)
- **cpal**: Cross-platform audio capture
- **reqwest**: HTTP client with multipart support
- **signal-hook-tokio**: Async signal handling
- **wl-clipboard-rs**: Wayland clipboard integration
- **tokio**: Async runtime
- **anyhow**: Error handling

### Optional Dependencies (Features)
- **candle-whisper**: Local transcription support
- **enigo**: Input simulation fallback
- **serde**: Advanced configuration support

## Performance Optimizations

### Memory Management
- **Zero-copy Audio**: Direct buffer management without unnecessary allocations
- **Circular Buffer**: Fixed-size buffer prevents memory growth
- **Streaming Upload**: Stream audio data directly to API without temporary files

### CPU Efficiency
- **Single Thread**: No thread synchronization overhead
- **Async I/O**: Non-blocking operations for network and file I/O
- **Lazy Initialization**: Load transcription backends only when needed

### Binary Size Optimization
- **Size optimization**: Minimize binary footprint through compiler flags
- **Link-time optimization**: Enable LTO for smaller binaries
- **Symbol stripping**: Remove debug symbols in release builds

## Security Considerations

### Memory Safety
- **Buffer Overflows**: Prevented by Rust's ownership system
- **Signal Safety**: signal-hook provides async-signal-safe operations
- **API Key Handling**: Never logged or stored persistently

### Process Security
- **Minimal Privileges**: Runs as user process, no root required
- **Sandboxing Ready**: Compatible with systemd service restrictions
- **No Network Storage**: Audio data never written to disk

## Testing Strategy

### Unit Tests
- **Audio Buffer**: Circular buffer correctness
- **Signal Handling**: Mock signal delivery
- **Text Injection**: Mock clipboard operations

### Integration Tests
- **Audio Recording**: Test with generated audio
- **API Integration**: Mock OpenAI API responses
- **End-to-End**: Automated workflow testing

### Performance Benchmarks
- **Memory Usage**: Continuous monitoring during recording
- **Latency Measurement**: Signal → transcription → text injection
- **Audio Quality**: Ensure no dropouts or corruption

## Deployment Strategy

### Single Binary Distribution
**Target**: `x86_64-unknown-linux-gnu`
**Size Goal**: <10MB statically linked binary
**Dependencies**: Only glibc and Linux kernel APIs

### Installation Methods
1. **Direct Download**: Single binary from GitHub releases
2. **Cargo Install**: `cargo install waystt`
3. **Package Managers**: AUR package for Arch Linux
4. **Systemd Service**: Template service file included

This technology stack ensures **waystt** remains a lightweight, reliable, and fast speech-to-text solution with minimal dependencies while leveraging Rust's strengths for systems programming.