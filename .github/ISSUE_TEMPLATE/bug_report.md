---
name: Bug report
about: Create a report to help us improve
title: '[BUG] '
labels: bug
assignees: ''

---

## Bug Description
A clear and concise description of what the bug is.

## Steps to Reproduce
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error

## Expected Behavior
A clear and concise description of what you expected to happen.

## Actual Behavior
A clear and concise description of what actually happened.

## Environment
- OS: [e.g. Arch Linux, Ubuntu 22.04]
- Audio System: [e.g. PipeWire, PulseAudio]
- HotLine Version: [e.g. v0.1.0]

## Audio Setup
- Audio devices: `hotline` output from startup
- PipeWire status: `systemctl --user status pipewire`
- Audio feedback enabled: [yes/no]

## Configuration
Please share your `.env` configuration (remove sensitive information like API keys):

```
ENABLE_AUDIO_FEEDBACK=true
BEEP_VOLUME=0.1
# ... other settings
```

## Logs
Please include relevant log output:

```
# From: tail -f /tmp/hotline.log
[paste logs here]
```

## Additional Context
Add any other context about the problem here, such as:
- Does it happen with both SIGUSR1 (direct typing) and SIGUSR2 (clipboard)?
- Does it happen consistently or intermittently?
- Any error messages in system logs?