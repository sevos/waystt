# Environment Configuration

HotLine supports configuration via environment variables loaded from a `.env` file.

## Quick Start

1. **Copy the example file:**
   ```bash
   cp .env.example .env
   ```

2. **Edit `.env` with your settings:**
   ```bash
   # Required: OpenAI API key for transcription
   OPENAI_API_KEY=sk-your-actual-api-key-here
   ```

3. **Run HotLine:**
   ```bash
   ./hotline
   # or with custom env file:
   ./hotline --envfile /path/to/custom.env
   ```

## Configuration Options

### Required Settings

- **`OPENAI_API_KEY`**: Your OpenAI API key for Whisper transcription
  - Get one from: https://platform.openai.com/api-keys
  - Example: `sk-proj-...`

### Optional Audio Settings

- **`AUDIO_BUFFER_DURATION_SECONDS`**: Maximum recording duration (default: 300)
- **`AUDIO_SAMPLE_RATE`**: Sample rate in Hz (default: 16000, optimized for Whisper)
- **`AUDIO_CHANNELS`**: Number of channels (default: 1, mono)

### Optional Transcription Settings

- **`WHISPER_MODEL`**: OpenAI Whisper model to use (default: whisper-1)
- **`WHISPER_LANGUAGE`**: Language code or "auto" for auto-detection (default: auto)

### Optional Logging Settings

- **`RUST_LOG`**: Log level (default: info)
  - Options: `error`, `warn`, `info`, `debug`, `trace`

## Command Line Usage

```bash
# Use default .env file
./hotline

# Use custom environment file
./hotline --envfile ~/.config/hotline/config.env

# Show help
./hotline --help

# Show version
./hotline --version
```

## Environment File Priority

1. File specified by `--envfile` parameter
2. Default `./.env` file in current directory
3. System environment variables (if no env file found)

## Security Notes

- **Never commit `.env` files** to version control (already in `.gitignore`)
- **Protect your API keys** - treat them like passwords
- **Use `.env.example`** as a template for sharing configuration structure