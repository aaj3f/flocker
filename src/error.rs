//! Error types for the Flocker application.

use std::fmt;

/// Custom error type for Flocker operations
#[derive(Debug)]
pub enum FlockerError {
    /// Docker-related errors
    Docker(String),
    /// Configuration errors
    Config(String),
    /// IO operation errors
    Io(std::io::Error),
    /// User interaction errors
    UserInput(String),
}

impl std::error::Error for FlockerError {}

impl fmt::Display for FlockerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlockerError::Docker(msg) => write!(f, "Docker error: {}", msg),
            FlockerError::Config(msg) => write!(f, "Configuration error: {}", msg),
            FlockerError::Io(err) => write!(f, "IO error: {}", err),
            FlockerError::UserInput(msg) => write!(f, "User input error: {}", msg),
        }
    }
}

impl From<std::io::Error> for FlockerError {
    fn from(err: std::io::Error) -> Self {
        FlockerError::Io(err)
    }
}

impl From<bollard::errors::Error> for FlockerError {
    fn from(err: bollard::errors::Error) -> Self {
        FlockerError::Docker(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let docker_err = FlockerError::Docker("connection failed".to_string());
        assert_eq!(docker_err.to_string(), "Docker error: connection failed");

        let config_err = FlockerError::Config("invalid port".to_string());
        assert_eq!(config_err.to_string(), "Configuration error: invalid port");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let flocker_err: FlockerError = io_err.into();
        match flocker_err {
            FlockerError::Io(_) => (),
            _ => panic!("Expected Io variant"),
        }
    }
}
