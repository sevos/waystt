use std::fmt;

pub mod realtime;

#[derive(Debug)]
pub struct NetworkErrorDetails {
    pub provider: String,
    pub error_type: String,
    pub error_message: String,
}

#[derive(Debug)]
pub enum TranscriptionError {
    NetworkError(NetworkErrorDetails),
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranscriptionError::NetworkError(details) => {
                write!(
                    f,
                    "Network error with {}: {} - {}",
                    details.provider, details.error_type, details.error_message
                )
            }
        }
    }
}

impl std::error::Error for TranscriptionError {}
