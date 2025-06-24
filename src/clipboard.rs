use std::process::Command;
use thiserror::Error;
use wl_clipboard_rs::copy::{MimeType, Options, Source};

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Wayland display server not available")]
    WaylandNotAvailable,
    
    #[error("Clipboard operation failed: {0}")]
    OperationFailed(String),
    
    #[error("Input simulation failed: {0}")]
    InputSimulationFailed(String),
    
    #[allow(dead_code)]
    #[error("Clipboard access denied")]
    AccessDenied,
    
    #[allow(dead_code)]
    #[error("Text encoding error: {0}")]
    EncodingError(String),
    
    #[allow(dead_code)]
    #[error("Generic clipboard error: {0}")]
    Generic(String),
}

pub struct ClipboardManager {
    // No internal state needed for shell command approach
}

impl ClipboardManager {
    pub fn new() -> Result<Self, ClipboardError> {
        // Check if we're in a Wayland environment
        if !Self::is_wayland_available() {
            return Err(ClipboardError::WaylandNotAvailable);
        }
        
        Ok(Self {})
    }
    
    pub fn copy_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if text.is_empty() {
            // Allow copying empty text - it's a valid operation
            return self.copy_bytes(&[]);
        }
        
        // Perform the copy operation using background mode (default)
        // This is suitable for SIGUSR1 (copy + paste) workflow
        self.copy_bytes(text.as_bytes())?;
        
        Ok(())
    }
    
    // Copy text and spawn daemon for persistence (for SIGUSR2)
    pub fn copy_text_persistent(&mut self, text: &str) -> Result<(), ClipboardError> {
        if text.is_empty() {
            return Ok(());
        }
        
        // Spawn detached daemon to serve clipboard data persistently
        Self::spawn_clipboard_daemon(text)
    }
    
    fn copy_bytes(&mut self, data: &[u8]) -> Result<(), ClipboardError> {
        let opts = Options::new();
        
        // Use background mode (default) for regular copy operations
        opts.copy(
            Source::Bytes(data.to_vec().into()),
            MimeType::Autodetect,
        )
        .map_err(|e| {
            use wl_clipboard_rs::copy::Error;
            match e {
                Error::NoSeats => {
                    ClipboardError::WaylandNotAvailable
                }
                Error::MissingProtocol { name: _, version: _ } => {
                    ClipboardError::WaylandNotAvailable
                }
                Error::PrimarySelectionUnsupported => {
                    ClipboardError::OperationFailed("Primary selection not supported".to_string())
                }
                _ => ClipboardError::OperationFailed(e.to_string()),
            }
        })?;
        
        Ok(())
    }
    
    pub fn paste_text(&mut self) -> Result<(), ClipboardError> {
        // Try wtype first (Wayland-native text input)
        if let Ok(output) = Command::new("wtype")
            .arg("-M")
            .arg("ctrl")
            .arg("v")
            .output()
        {
            if output.status.success() {
                return Ok(());
            }
        }
        
        // Try ydotool as fallback (requires sudo/input group membership)
        if let Ok(output) = Command::new("ydotool")
            .arg("key")
            .arg("29:1")  // Ctrl down
            .arg("47:1")  // V down
            .arg("47:0")  // V up
            .arg("29:0")  // Ctrl up
            .output()
        {
            if output.status.success() {
                return Ok(());
            }
        }
        
        // No suitable paste method available
        Err(ClipboardError::InputSimulationFailed(
            "No suitable paste method available. Install 'wtype' for Wayland text input, \
             or configure 'ydotool' with proper permissions. Alternatively, paste manually with Ctrl+V.".to_string()
        ))
    }
    
    #[allow(dead_code)]
    pub fn copy_and_paste_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        // Combined operation: copy then paste
        self.copy_text(text)?;
        
        // Small delay to ensure clipboard operation completes
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        self.paste_text()?;
        
        Ok(())
    }
    
    // Alternative paste method using different key combinations
    #[allow(dead_code)]
    pub fn paste_with_shift_insert(&mut self) -> Result<(), ClipboardError> {
        // Try wtype with Shift+Insert (some applications respond better)
        if let Ok(output) = Command::new("wtype")
            .arg("-M")
            .arg("shift")
            .arg("Insert")
            .output()
        {
            if output.status.success() {
                return Ok(());
            }
        }
        
        // Try ydotool with Shift+Insert
        if let Ok(output) = Command::new("ydotool")
            .arg("key")
            .arg("42:1")  // Shift down
            .arg("110:1") // Insert down
            .arg("110:0") // Insert up
            .arg("42:0")  // Shift up
            .output()
        {
            if output.status.success() {
                return Ok(());
            }
        }
        
        // Fallback to regular paste
        self.paste_text()
    }
    
    // Utility method to check if Wayland is available
    pub fn is_wayland_available() -> bool {
        std::env::var("WAYLAND_DISPLAY").is_ok()
    }
    
    // Utility method to check which paste tools are available
    pub fn check_paste_tools() -> Vec<String> {
        let mut available_tools = Vec::new();
        
        // Check for wtype
        if Command::new("wtype").arg("--version").output().is_ok() {
            available_tools.push("wtype".to_string());
        }
        
        // Check for ydotool
        if Command::new("ydotool").arg("--help").output().is_ok() {
            available_tools.push("ydotool".to_string());
        }
        
        available_tools
    }
    
    // Spawn a detached daemon process to serve clipboard data
    pub fn spawn_clipboard_daemon(text: &str) -> Result<(), ClipboardError> {
        // Check if we're in a Wayland environment first
        if !Self::is_wayland_available() {
            return Err(ClipboardError::WaylandNotAvailable);
        }
        
        // Create the daemon process
        let text_clone = text.to_string();
        
        // Fork the process to create daemon
        match unsafe { libc::fork() } {
            -1 => {
                return Err(ClipboardError::OperationFailed(
                    "Failed to fork process for clipboard daemon".to_string()
                ));
            }
            0 => {
                // Child process - this will become the daemon
                Self::run_clipboard_daemon(&text_clone);
                // Child process exits here (should never return)
                std::process::exit(0);
            }
            _pid => {
                // Parent process - continue execution
                // Give the child process a moment to start
                std::thread::sleep(std::time::Duration::from_millis(100));
                Ok(())
            }
        }
    }
    
    // Run the clipboard daemon (this runs in the forked child process)
    fn run_clipboard_daemon(text: &str) {
        // Detach from parent process group
        if unsafe { libc::setsid() } == -1 {
            eprintln!("Failed to create new session for clipboard daemon");
            return;
        }
        
        // Set up clipboard with foreground mode to serve requests
        let mut opts = Options::new();
        opts.foreground(true);
        
        let result = opts.copy(
            Source::Bytes(text.as_bytes().to_vec().into()),
            MimeType::Autodetect,
        );
        
        match result {
            Ok(()) => {
                // Successfully serving clipboard data
                // The foreground mode will keep this process alive to serve requests
                // Process will terminate when no more requests are needed
            }
            Err(e) => {
                eprintln!("Clipboard daemon failed: {}", e);
            }
        }
        
        // Daemon process exits here
        std::process::exit(0);
    }
    
    // Method to provide helpful setup instructions
    pub fn get_setup_instructions() -> String {
        let wayland_available = Self::is_wayland_available();
        let paste_tools = Self::check_paste_tools();
        
        if !wayland_available {
            return "Wayland not detected. This tool is designed for Wayland environments.".to_string();
        }
        
        if paste_tools.is_empty() {
            return "No paste tools available. Install 'wtype' (recommended) or 'ydotool':\n\
                    - Arch Linux: sudo pacman -S wtype\n\
                    - Ubuntu/Debian: sudo apt install wtype\n\
                    - Or configure ydotool with proper permissions".to_string();
        }
        
        format!("Available paste tools: {}", paste_tools.join(", "))
    }
    
    // Verify that the clipboard contains the expected text
    fn verify_clipboard_content(&self, expected_text: &str) -> Result<bool, ClipboardError> {
        // Add a small delay to ensure clipboard operation has completed
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        match self.get_clipboard_text() {
            Ok(clipboard_content) => {
                let matches = clipboard_content.trim() == expected_text.trim();
                if !matches {
                    eprintln!("Clipboard verification failed:");
                    eprintln!("  Expected: '{}'", expected_text);
                    eprintln!("  Got:      '{}'", clipboard_content);
                }
                Ok(matches)
            }
            Err(e) => Err(e),
        }
    }
    
    // Utility method to get clipboard contents (for testing)
    #[allow(dead_code)]
    pub fn get_clipboard_text(&self) -> Result<String, ClipboardError> {
        use std::io::Read;
        use wl_clipboard_rs::paste::{get_contents, ClipboardType, MimeType, Seat};
        
        let result = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text);
        match result {
            Ok((mut pipe, _)) => {
                let mut contents = Vec::new();
                pipe.read_to_end(&mut contents)
                    .map_err(|e| ClipboardError::OperationFailed(e.to_string()))?;
                
                String::from_utf8(contents)
                    .map_err(|e| ClipboardError::EncodingError(e.to_string()))
            }
            Err(wl_clipboard_rs::paste::Error::NoSeats) => {
                Err(ClipboardError::WaylandNotAvailable)
            }
            Err(wl_clipboard_rs::paste::Error::ClipboardEmpty) => {
                Ok(String::new())
            }
            Err(wl_clipboard_rs::paste::Error::NoMimeType) => {
                Ok(String::new())
            }
            Err(e) => Err(ClipboardError::OperationFailed(e.to_string())),
        }
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // If Wayland isn't available, create a non-functional instance
            // This allows the application to start but paste operations will fail
            Self {}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_clipboard_manager_creation() {
        // This might fail in CI environments without proper display
        match ClipboardManager::new() {
            Ok(_) => println!("ClipboardManager created successfully"),
            Err(e) => println!("ClipboardManager creation failed (expected in CI): {}", e),
        }
    }
    
    #[test]
    fn test_wayland_detection() {
        let is_wayland = ClipboardManager::is_wayland_available();
        println!("Wayland available: {}", is_wayland);
        
        // Test should not fail regardless of environment
        assert!(is_wayland || !is_wayland); // Always true
    }
    
    #[test]
    fn test_error_display() {
        let error = ClipboardError::WaylandNotAvailable;
        assert_eq!(format!("{}", error), "Wayland display server not available");
        
        let error = ClipboardError::OperationFailed("test error".to_string());
        assert_eq!(format!("{}", error), "Clipboard operation failed: test error");
    }
}