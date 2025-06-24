## QA Testing Workflow

- For QAing, run the app with `nohup` and `&` to properly detach from terminal:
  ```bash
  nohup ./target/release/waystt > /tmp/waystt.log 2>&1 & disown
  ```
- Then:
  - Ask the user to speak something
  - Wait 5 seconds
  - Run `pkill --signal SIGUSR1 waystt` to trigger transcription
  - Check logs with `tail /tmp/waystt.log`
- Future improvement: Ask user to press RETURN, as their focus will likely be on the Claude Code terminal, which will send the transcribed text to the agent