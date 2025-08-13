### 2. Implement Subcommands for the CLI

**Description:**

The command-line interface currently takes all arguments directly. To prepare for future extensions, we need to refactor the CLI to use subcommands. The existing functionality will be moved under a `daemon` subcommand.

**Key Changes:**

1.  **Introduce a `Commands` Enum:**
    -   In `src/main.rs`, define a new enum `Commands` with a `Daemon` variant.
    -   The `Daemon` variant should contain the existing command-line arguments (`envfile` and `pipe_to`).

2.  **Update the `Args` Struct:**
    -   Modify the `Args` struct to include a `command` field of type `Commands`.

    ```rust
    #[derive(Parser)]
    #[command(name = "hotline")]
    struct Args {
        #[command(subcommand)]
        command: Commands,
    }

    #[derive(clap::Subcommand)]
    enum Commands {
        /// Run the HotLine daemon
        Daemon {
            /// Path to environment file
            #[arg(long)]
            envfile: Option<PathBuf>,

            /// Pipe transcribed text to the specified command
            #[arg(long, short = 'p', num_args = 1.., value_name = "COMMAND", allow_hyphen_values = true, trailing_var_arg = true)]
            pipe_to: Option<Vec<String>>,
        },
    }
    ```

3.  **Refactor `main` Function:**
    -   Update the `main` function to handle the new subcommand structure. The core logic will be executed when the `daemon` subcommand is used.

    ```rust
    fn main() -> Result<()> {
        let args = Args::parse();

        match args.command {
            Commands::Daemon { envfile, pipe_to } => {
                // All the existing logic goes here
            }
        }

        Ok(())
    }
    ```

**Acceptance Criteria:**

-   The application is now run using `hotline daemon`.
-   All existing command-line arguments (`--envfile` and `--pipe-to`) are now options for the `daemon` subcommand.
-   The application's core functionality remains unchanged when run with the `daemon` subcommand.
