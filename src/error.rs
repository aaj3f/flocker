//! Error types for the Flocker application.
use std::path::PathBuf;
use thiserror::Error;

/// Custom error type for Flocker operations
#[derive(Debug, Error)]
pub enum FlockerError {
    /// Docker-related errors
    #[error("Docker error: {0}")]
    Docker(String),
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),
    /// Configuration file errors
    #[error("{message} (configuration file path: {path}): {source}")]
    ConfigFile {
        /// Error message
        message: String,
        /// Path to the configuration file
        path: PathBuf,
        /// Source of the error
        source: anyhow::Error,
    },
    /// IO operation errors
    #[error("IO error: {0}")]
    Io(std::io::Error),
    /// User interaction errors
    #[error("User input error: {0}")]
    UserInput(String),
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
