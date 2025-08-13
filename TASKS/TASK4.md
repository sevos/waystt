### 4. Implement a TOML Configuration File and `config` Subcommand

**Description:**

To provide a more robust and flexible configuration system, we will introduce a TOML configuration file. This file will serve as a central place for setting default values and other options that are not easily managed through environment variables or command-line arguments. We will also add a `config` subcommand to validate and display the current configuration.

**Key Changes:**

1.  **Configuration File:**
    -   The application should look for a configuration file at `$XDG_CONFIG_DIR/hotline/hotline.toml`.
    -   The configuration file should be in TOML format.
    -   The structure of the configuration file should mirror the subcommand structure. For example:

        ```toml
        [daemon]
        openai_api_key = "your-api-key"
        whisper_model = "whisper-1"
        # ... other daemon-specific settings

        [sendcmd]
        # ... settings for the sendcmd subcommand
        ```

2.  **Configuration Loading:**
    -   Update the `Config` struct in `src/config.rs` to load settings from the TOML file.
    -   The order of precedence for configuration should be: command-line arguments > environment variables > TOML configuration file > default values.

3.  **Implement `config` Subcommand:**
    -   Create a new `hotline config` subcommand.
    -   This subcommand should read and validate the `hotline.toml` configuration file.
    -   Upon successful validation, it should print the entire configuration to standard output in a human-readable format.
    -   **Important:** The `config` subcommand must **never** write to the configuration file. It is a read-only operation.

4.  **Map Existing Configuration:**
    -   Ensure that all configuration parameters currently loaded from environment variables (as defined in `src/config.rs`) can also be specified in the `hotline.toml` file under the `[daemon]` section.

**Acceptance Criteria:**

-   The application correctly loads and applies settings from the `hotline.toml` file.
-   The `hotline config` subcommand successfully validates and prints the configuration.
-   The order of configuration precedence is correctly implemented.
-   All existing configuration options are supported in the new TOML file.
