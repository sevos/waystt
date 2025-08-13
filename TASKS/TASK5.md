### 5. Integrate Command Execution into the `StartTranscription` Command

**Description:**

The current method of executing a command for each transcription using a global `--pipe-to` argument is inflexible. To allow for more advanced command execution strategies in the future, we will integrate this behavior into the `StartTranscription` command itself.

**Key Changes:**

1.  **Update `StartTranscriptionArgs`:**
    -   In `src/socket.rs`, add a new optional `command` field to the `StartTranscriptionArgs` struct.

2.  **Create `CommandExecution` Enum:**
    -   Define a new `CommandExecution` enum that will represent the different ways a command can be executed. For now, it will have a single variant.

    ```rust
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub enum CommandExecution {
        SpawnForEachTranscription {
            command: Vec<String>,
        },
    }

    // In StartTranscriptionArgs
    #[derive(Serialize, Deserialize, Debug)]
    pub struct StartTranscriptionArgs {
        // ... existing fields
        pub command: Option<CommandExecution>,
    }
    ```

3.  **Refactor Command Execution Logic:**
    -   Move the command execution logic from `src/main.rs` into the `daemon`'s handler for the `StartTranscription` command.
    -   When a `StartTranscription` command is received with the `command` field set, the daemon should execute the specified command for each transcription received.

4.  **Remove `--pipe-to` Argument:**
    -   Remove the `--pipe-to` command-line argument from the `daemon` subcommand in `src/main.rs`.

**Acceptance Criteria:**

-   The `--pipe-to` argument is removed from the CLI.
-   The `StartTranscription` command now accepts an optional `command` field.
-   When the `command` field is present, the daemon executes the specified command for each transcription, writing the transcription text to the command's standard input.
-   If the `command` field is not present, the transcription is simply printed to standard output as before.
