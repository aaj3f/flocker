//! Configuration management for Flocker.
//!
//! This module handles configuration settings for Fluree containers,
//! including port mappings and volume mounts.

use crate::error::FlockerError;
use crate::Result;
use std::path::PathBuf;

/// Configuration for a Fluree container instance
#[derive(Debug, Clone)]
pub struct FlureeConfig {
    /// Host port to map to container's port 8090
    pub host_port: u16,
    /// Path to mount as container's data directory
    pub data_mount: Option<PathBuf>,
}

impl Default for FlureeConfig {
    fn default() -> Self {
        Self {
            host_port: 8090,
            data_mount: None,
        }
    }
}

impl FlureeConfig {
    /// Create a new configuration with custom settings
    pub fn new(host_port: u16, data_mount: Option<PathBuf>) -> Self {
        Self {
            host_port,
            data_mount,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate port number
        if self.host_port < 1024 {
            return Err(FlockerError::Config(
                "Host port must be greater than 1023".to_string(),
            ));
        }

        // Validate data mount path if specified
        if let Some(path) = &self.data_mount {
            if !path.exists() {
                return Err(FlockerError::Config(format!(
                    "Data mount path does not exist: {}",
                    path.display()
                )));
            }
            if !path.is_dir() {
                return Err(FlockerError::Config(format!(
                    "Data mount path is not a directory: {}",
                    path.display()
                )));
            }

            // Try to canonicalize the path to ensure it's absolute and all symlinks are resolved
            path.canonicalize().map_err(|e| {
                // FlockerError::Config(format!(
                //     "Failed to resolve data mount path {}: {}",
                //     path.display(),
                //     e
                // ))
                FlockerError::ConfigFile {
                    message: "Failed to resolve data mount path".to_string(),
                    path: path.clone(),
                    source: e.into(),
                }
            })?;
        }

        Ok(())
    }

    /// Convert the configuration into Docker-compatible settings
    pub fn into_docker_config(self) -> crate::docker::ContainerConfig {
        let data_mount_path = self.data_mount.and_then(|path| {
            // Convert path to absolute and resolve symlinks
            path.canonicalize()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        });

        crate::docker::ContainerConfig {
            host_port: self.host_port,
            container_port: 8090,
            data_mount_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use serial_test::parallel;

    use super::*;

    #[test]
    #[parallel]
    fn test_default_config() {
        let config = FlureeConfig::default();
        assert_eq!(config.host_port, 8090);
        assert!(config.data_mount.is_none());
    }

    #[test]
    #[parallel]
    fn test_custom_config() {
        let config = FlureeConfig::new(9090, None);
        assert_eq!(config.host_port, 9090);
        assert!(config.data_mount.is_none());
    }

    #[test]
    #[parallel]
    fn test_invalid_port() {
        let config = FlureeConfig::new(80, None);
        assert!(config.validate().is_err());
    }

    #[test]
    #[parallel]
    fn test_invalid_data_mount() {
        let config = FlureeConfig::new(8090, Some(PathBuf::from("/nonexistent/path")));
        assert!(config.validate().is_err());
    }

    #[test]
    #[parallel]
    fn test_valid_data_mount() {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let config = FlureeConfig::new(8090, Some(temp_dir.path().to_path_buf()));
        assert!(config.validate().is_ok());
    }

    #[test]
    #[parallel]
    fn test_relative_data_mount() {
        // Create a temporary directory and a relative path within it
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp_dir.path().join("data")).unwrap();
        let relative_path = PathBuf::from("data");

        // Change to the temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let config = FlureeConfig::new(8090, Some(relative_path));
        assert!(config.validate().is_ok());

        // Change back to the original directory
        std::env::set_current_dir(original_dir).unwrap();
    }
}
