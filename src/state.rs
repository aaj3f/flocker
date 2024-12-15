//! State management for Flocker.
//!
//! This module handles persistent state including user preferences
//! and running container information.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::FlockerError;
use crate::Result;

/// Persistent state for the Flocker application
#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    /// Last used port mapping
    pub last_port: Option<u16>,
    /// Last used data directory
    pub last_data_dir: Option<PathBuf>,
    /// Whether to run in detached mode by default
    pub default_detached: bool,
    /// ID of the currently running container, if any
    pub running_container: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_port: Some(8090),
            last_data_dir: None,
            default_detached: true,
            running_container: None,
        }
    }
}

impl State {
    /// Load state from disk, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| FlockerError::Config(format!("Failed to read config: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| FlockerError::Config(format!("Failed to parse config: {}", e)))
    }

    /// Save current state to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                FlockerError::Config(format!("Failed to create config directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| FlockerError::Config(format!("Failed to serialize config: {}", e)))?;

        fs::write(&config_path, content)
            .map_err(|e| FlockerError::Config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Update the running container ID and save state
    pub fn set_running_container(&mut self, container_id: Option<String>) -> Result<()> {
        self.running_container = container_id;
        self.save()
    }

    /// Get the path to the config file
    fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "fluree", "flocker").ok_or_else(|| {
            FlockerError::Config("Failed to determine config directory".to_string())
        })?;

        Ok(proj_dirs.config_dir().join("config.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn test_state_default() {
        let state = State::default();
        assert_eq!(state.last_port, Some(8090));
        assert!(state.last_data_dir.is_none());
        assert!(state.default_detached);
        assert!(state.running_container.is_none());
    }

    #[test]
    fn test_state_save_load() {
        // Create a temporary directory for config
        let temp_dir = tempdir().unwrap();
        // let config_dir = temp_dir.path().join("flocker");

        // Override config directory for test
        env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let mut state = State::default();
        state.last_port = Some(9090);
        state.running_container = Some("test_container".to_string());

        // Save state
        state.save().unwrap();

        // Load state
        let loaded = State::load().unwrap();
        assert_eq!(loaded.last_port, Some(9090));
        assert_eq!(loaded.running_container, Some("test_container".to_string()));
    }
}
