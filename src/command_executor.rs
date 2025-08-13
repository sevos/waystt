use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;

/// Type alias for async command functions
type CommandFn = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// Request to execute a command
struct CommandRequest {
    name: String,
    command: CommandFn,
}

/// Centralized command executor that ensures sequential execution of hooks and commands
#[derive(Clone)]
pub struct CommandExecutor {
    request_tx: mpsc::UnboundedSender<CommandRequest>,
}

impl CommandExecutor {
    /// Create a new command executor
    pub fn new() -> Self {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<CommandRequest>();

        // Spawn the command processing task
        tokio::spawn(async move {
            // Process commands sequentially
            while let Some(request) = request_rx.recv().await {
                eprintln!("Executing command: {}", request.name);

                // Execute the command and log any errors
                if let Err(e) = request.command.await {
                    eprintln!("Command '{}' failed: {}", request.name, e);
                }
            }
        });

        Self { request_tx }
    }

    /// Execute a command sequentially (will wait for any currently executing command)
    pub fn execute<F>(&self, name: impl Into<String>, command: F)
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        let request = CommandRequest {
            name: name.into(),
            command: Box::pin(command),
        };

        // Send the command to be executed (ignore send errors if receiver is dropped)
        let _ = self.request_tx.send(request);
    }

    /// Execute a hook command with the given name and command string
    pub fn execute_hook(&self, hook_name: &str, command: &[String], input: String) {
        let hook_name = hook_name.to_string();
        let command = command.to_vec();

        self.execute(hook_name.clone(), async move {
            crate::command::execute_with_input(&command, &input)
                .await
                .map(|_| ()) // Convert Result<i32> to Result<()>
                .map_err(|e| anyhow::anyhow!("Hook '{}' failed: {}", hook_name, e))
        });
    }
}
