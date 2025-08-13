### 3. Replace Signal Handling with a UNIX Socket Interface

**Description:**

The current signal-based interface (`SIGUSR1`, `SIGUSR2`) for controlling transcription is too simplistic. We need to replace it with a more flexible and extensible UNIX socket-based command interface. This will allow for richer, per-session configuration of the transcription process.

**Key Changes:**

1.  **Remove Signal Handling:**
    -   In `src/main.rs`, remove the `signal-hook` and `signal-hook-tokio` dependencies and all related signal handling logic.

2.  **Implement UNIX Socket in `daemon`:**
    -   The `hotline daemon` should create and listen on a UNIX socket.
    -   The socket should be located in an idiomatic path, such as `$XDG_RUNTIME_DIR/hotline.sock`.
    -   The daemon should listen for incoming connections and parse JSON commands from the socket.

3.  **Implement `sendcmd` Subcommand:**
    -   Create a new `hotline sendcmd` subcommand.
    -   This subcommand will read a JSON command from standard input, validate it against the defined protocol, and then send it to the UNIX socket.

4.  **Define JSON Command Protocol:**
    -   In a new module (e.g., `src/socket.rs`), define the command protocol using `serde` for serialization and deserialization.

    ```rust
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Command {
        StartTranscription(StartTranscriptionArgs),
        StopTranscription,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct StartTranscriptionArgs {
        // Transcription parameters
        pub model: Option<String>,
        pub language: Option<String>,
        pub prompt: Option<String>,

        // VAD parameters
        pub vad_config: Option<VadConfig>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum VadConfig {
        ServerVad(ServerVadConfig),
        SemanticVad(SemanticVadConfig),
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ServerVadConfig {
        pub threshold: Option<f32>,
        pub prefix_padding_ms: Option<u32>,
        pub silence_duration_ms: Option<u32>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SemanticVadConfig {
        pub eagerness: Option<String>,
    }
    ```

5.  **Implement `StartTranscription` Command:**
    -   When the `daemon` receives a `StartTranscription` command, it should start a new transcription session with the provided parameters.
    -   The `StartTranscriptionArgs` struct should allow for all the configurable parameters in the OpenAI Real-time Transcription and VAD APIs. Refer to the `openai_docs` for a complete list of parameters.

6.  **Implement `StopTranscription` Command:**
    -   When the `daemon` receives a `StopTranscription` command, it should gracefully stop the current transcription session. This replaces the old `SIGUSR2` functionality.

**Acceptance Criteria:**

-   The `hotline daemon` starts without errors and creates a UNIX socket.
-   The `hotline sendcmd` can read a JSON command from stdin and send it to the daemon.
-   The `daemon` correctly parses the commands and starts/stops transcription sessions with the specified parameters.
-   The old signal-based controls are completely removed.
