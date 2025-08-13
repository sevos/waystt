# HotLine Manual Pages

This directory contains the manual pages for the HotLine speech-to-text tool.

## Available Man Pages

### Section 1 - User Commands
- `hotline.1` - Main command overview
- `hotline-daemon.1` - Background service documentation
- `hotline-start-transcription.1` - Start transcription command
- `hotline-stop-transcription.1` - Stop transcription command
- `hotline-sendcmd.1` - Send raw JSON commands
- `hotline-config.1` - Configuration validation and display

### Section 5 - File Formats
- `hotline.toml.5` - Configuration file format


## Viewing Man Pages

After installation:
```bash
man hotline
man hotline-daemon
man hotline-start-transcription
man hotline-stop-transcription
man hotline-sendcmd
man hotline-config
man hotline.toml
```

Without installation (from source directory):
```bash
man ./man/hotline.1
man ./man/hotline.toml.5
# etc.
```

## Building Man Pages

The man pages are written in troff format. To convert to other formats:

### HTML:
```bash
groff -man -Thtml hotline.1 > hotline.html
```

### PDF:
```bash
groff -man -Tpdf hotline.1 > hotline.pdf
```

### Plain text:
```bash
groff -man -Tutf8 hotline.1 | col -b > hotline.txt
```

## Quick Reference

### Command Structure
```
hotline COMMAND [OPTIONS]
```

### Available Commands
- `daemon` - Run background service
- `start-transcription PROFILE` - Start transcription
- `stop-transcription` - Stop transcription
- `sendcmd` - Send JSON commands
- `config` - Show configuration

### Common Examples
```bash
# Start daemon
hotline daemon &

# Start transcription
hotline start-transcription default

# Stop transcription
hotline stop-transcription

# Check configuration
hotline config
```

## See Also

- Project repository: https://github.com/sevos/hotline
- TOML specification: https://toml.io
- OpenAI API documentation: https://platform.openai.com/docs