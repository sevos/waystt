### 6. Introduce Profiles and User-Friendly Subcommands

**Description:**

To improve usability, we will introduce a profile system in the configuration file. This will allow users to define and easily switch between different sets of transcription settings. We will also add dedicated subcommands for starting and stopping transcription, making the tool more intuitive to use.

**Key Changes:**

1.  **Profile Configuration:**
    -   In `hotline.toml`, add a `[profiles]` section.
    -   Each sub-section under `[profiles]` will represent a named profile.
    -   Each profile can contain all the valid options for a `StartTranscription` command (e.g., `model`, `language`, `command`, etc.).

    ```toml
    [daemon]
    # ... daemon settings

    [profiles.default]
    model = "whisper-1"
    language = "en"

    [profiles.coding]
    model = "whisper-1"
    language = "en"
    prompt = "The user is a programmer, so expect technical terms."
    command = { SpawnForEachTranscription = { command = ["xdotool", "type", "--file", "-"] } }
    ```

2.  **Implement `start-transcription` Subcommand:**
    -   Create a new `hotline start-transcription <profilename>` subcommand.
    -   This subcommand takes a single argument: the name of the profile to use.
    -   It should read the `hotline.toml` file, find the specified profile, and send a `StartTranscription` command to the daemon with the settings from that profile.

3.  **Implement `stop-transcription` Subcommand:**
    -   Create a new `hotline stop-transcription` subcommand.
    -   This subcommand will send a `StopTranscription` command to the daemon.

4.  **Update `sendcmd` Subcommand:**
    -   The `sendcmd` subcommand will remain for advanced users who want to send raw JSON commands, but the new `start-transcription` and `stop-transcription` subcommands will be the primary way for users to interact with the daemon.

**Acceptance Criteria:**

-   The application correctly loads and parses profiles from the `hotline.toml` file.
-   The `hotline start-transcription <profilename>` command successfully starts a transcription session with the settings from the specified profile.
-   The `hotline stop-transcription` command successfully stops the current transcription session.
-   The user experience is improved by providing a simpler, more intuitive way to control the application.
