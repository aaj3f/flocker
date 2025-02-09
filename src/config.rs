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
    /// Path to mount as container's config directory
    pub config_mount: Option<PathBuf>,
    /// Name of the config file to use
    pub config_file: Option<PathBuf>,
}

impl Default for FlureeConfig {
    fn default() -> Self {
        Self {
            host_port: 8090,
            data_mount: None,
            config_mount: None,
            config_file: None,
        }
    }
}

impl FlureeConfig {
    /// Create a new configuration with custom settings
    pub fn new(
        host_port: u16,
        data_mount: Option<PathBuf>,
        config_mount: Option<PathBuf>,
        config_file: Option<PathBuf>,
    ) -> Self {
        Self {
            host_port,
            data_mount,
            config_mount,
            config_file,
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

        // Helper function to validate a directory path
        let validate_dir = |path: &PathBuf, name: &str| -> Result<()> {
            if !path.exists() {
                return Err(FlockerError::Config(format!(
                    "{} path does not exist: {}",
                    name,
                    path.display()
                )));
            }
            if !path.is_dir() {
                return Err(FlockerError::Config(format!(
                    "{} path is not a directory: {}",
                    name,
                    path.display()
                )));
            }
            path.canonicalize().map_err(|e| FlockerError::ConfigFile {
                message: format!("Failed to resolve {} path", name),
                path: path.clone(),
                source: e.into(),
            })?;
            Ok(())
        };

        // Validate data mount path if specified
        if let Some(path) = &self.data_mount {
            validate_dir(path, "Data mount")?;
        }

        // Validate config mount path and file if specified
        if let Some(path) = &self.config_mount {
            validate_dir(path, "Config mount")?;

            // If config mount is specified, config file must also be specified
            if self.config_file.is_none() {
                return Err(FlockerError::Config(
                    "Config file must be specified when config mount is provided".to_string(),
                ));
            }

            // Validate that config file exists in the config mount directory
            if let Some(file) = &self.config_file {
                let full_path = path.join(file);
                if !full_path.exists() {
                    return Err(FlockerError::Config(format!(
                        "Config file does not exist: {}",
                        full_path.display()
                    )));
                }
            }
        } else if self.config_file.is_some() {
            return Err(FlockerError::Config(
                "Config mount must be specified when config file is provided".to_string(),
            ));
        }

        Ok(())
    }

    /// Convert the configuration into Docker-compatible settings
    pub fn into_docker_config(self) -> crate::docker::ContainerConfig {
        crate::docker::ContainerConfig {
            host_port: self.host_port,
            container_port: 8090,
            data_mount_path: self.data_mount,
            config_mount_path: self.config_mount,
            config_file: self.config_file,
        }
    }
}

#[cfg(test)]
mod tests {
    use serial_test::parallel;

    use super::*;

    #[test]
    #[parallel]
    fn test_config_file_without_mount() {
        let config = FlureeConfig::new(8090, None, None, Some(PathBuf::from("config.edn")));
        assert!(config.validate().is_err());
    }

    #[test]
    #[parallel]
    fn test_config_mount_without_file() {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let config = FlureeConfig::new(8090, None, Some(temp_dir.path().to_path_buf()), None);
        assert!(config.validate().is_err());
    }

    #[test]
    #[parallel]
    fn test_valid_config_mount_and_file() {
        // Create a temporary directory and config file
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir.path().join("config.edn");
        std::fs::write(&config_file, "test").unwrap();

        let config = FlureeConfig::new(
            8090,
            None,
            Some(temp_dir.path().to_path_buf()),
            Some(PathBuf::from("config.edn")),
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    #[parallel]
    fn test_invalid_config_file() {
        // Create a temporary directory without the config file
        let temp_dir = tempfile::tempdir().unwrap();
        let config = FlureeConfig::new(
            8090,
            None,
            Some(temp_dir.path().to_path_buf()),
            Some(PathBuf::from("nonexistent.edn")),
        );
        assert!(config.validate().is_err());
    }

    #[test]
    #[parallel]
    fn test_default_config() {
        let config = FlureeConfig::default();
        assert_eq!(config.host_port, 8090);
        assert!(config.data_mount.is_none());
        assert!(config.config_mount.is_none());
        assert!(config.config_file.is_none());
    }

    #[test]
    #[parallel]
    fn test_custom_config() {
        let config = FlureeConfig::new(9090, None, None, None);
        assert_eq!(config.host_port, 9090);
        assert!(config.data_mount.is_none());
        assert!(config.config_mount.is_none());
        assert!(config.config_file.is_none());
    }

    #[test]
    #[parallel]
    fn test_invalid_port() {
        let config = FlureeConfig::new(80, None, None, None);
        assert!(config.validate().is_err());
    }

    #[test]
    #[parallel]
    fn test_invalid_data_mount() {
        let config = FlureeConfig::new(8090, Some(PathBuf::from("/nonexistent/path")), None, None);
        assert!(config.validate().is_err());
    }

    #[test]
    #[parallel]
    fn test_valid_data_mount() {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let config = FlureeConfig::new(8090, Some(temp_dir.path().to_path_buf()), None, None);
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

        let config = FlureeConfig::new(8090, Some(relative_path), None, None);
        assert!(config.validate().is_ok());

        // Change back to the original directory
        std::env::set_current_dir(original_dir).unwrap();
    }
}
