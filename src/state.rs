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

/// Configuration for a data directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDirConfig {
    /// Relative path to the data directory
    pub relative_path: Option<PathBuf>,
    /// Absolute path to the data directory
    pub absolute_path: PathBuf,
}

impl DataDirConfig {
    pub fn new(absolute_path: PathBuf, relative_path: Option<PathBuf>) -> Self {
        DataDirConfig {
            relative_path,
            absolute_path,
        }
    }

    pub fn from_current_dir(current_dir: &PathBuf) -> Self {
        let relative_path = "data";
        let absolute_path = current_dir.join(relative_path);
        DataDirConfig {
            relative_path: Some(PathBuf::from("./data")),
            absolute_path,
        }
    }

    pub fn display_relative_path(&self) -> String {
        if let Some(ref relative_path) = self.relative_path {
            relative_path.to_string_lossy().to_string()
        } else {
            self.absolute_path.to_string_lossy().to_string()
        }
    }
}

/// Information about a Fluree container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    /// Container ID
    pub id: String,
    /// User-given name for the container
    pub name: String,
    /// Port mapping
    pub port: u16,
    /// Data directory configuration
    pub data_dir: Option<DataDirConfig>,
    /// Whether container is running in detached mode
    pub detached: bool,
    /// Image tag used for this container
    pub image_tag: String,
    /// Last start time
    pub last_start: Option<String>,
}

impl ContainerInfo {
    pub fn new(
        id: String,
        name: String,
        port: u16,
        data_dir: Option<DataDirConfig>,
        detached: bool,
        image_tag: String,
    ) -> Self {
        Self {
            id,
            name,
            port,
            data_dir,
            detached,
            image_tag,
            last_start: None,
        }
    }
}

/// Persistent state for the Flocker application
#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    /// Known containers, mapped by ID
    pub containers: std::collections::HashMap<String, ContainerInfo>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            containers: std::collections::HashMap::new(),
        }
    }
}

impl State {
    /// Load state from disk, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        tracing::debug!("Loading state from: {:?}", config_path);

        if !config_path.exists() {
            tracing::debug!("No config file found, creating default state");
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path).map_err(|e| FlockerError::ConfigFile {
            message: "Failed to read config file".to_string(),
            path: config_path.clone(),
            source: e.into(),
        })?;

        serde_json::from_str(&content).map_err(|e| FlockerError::ConfigFile {
            message: "Failed to parse config file".to_string(),
            path: config_path.clone(),
            source: e.into(),
        })
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

    /// Add or update a container in the state
    pub fn add_container(&mut self, info: ContainerInfo) -> Result<()> {
        self.containers.insert(info.id.clone(), info);
        self.save()
    }

    /// Remove a container from the state
    pub fn remove_container(&mut self, container_id: &str) -> Result<()> {
        self.containers.remove(container_id);
        self.save()
    }

    /// Get a container by ID
    pub fn get_container(&self, container_id: &str) -> Option<&ContainerInfo> {
        self.containers.get(container_id)
    }

    /// Get all known containers
    pub fn get_containers(&self) -> Vec<&ContainerInfo> {
        self.containers.values().collect()
    }

    /// Update container status
    pub fn update_container_status(
        &mut self,
        container_id: &str,
        is_running: bool,
        start_time: Option<String>,
    ) -> Result<()> {
        if let Some(container) = self.containers.get_mut(container_id) {
            if is_running {
                container.last_start = start_time;
            }
        }
        self.save()
    }

    /// Get the most recently used container's settings as defaults for a new container
    pub fn get_default_settings(&self) -> (u16, Option<DataDirConfig>, bool) {
        self.containers
            .values()
            .max_by_key(|c| c.last_start.as_ref())
            .map(|c| (c.port, c.data_dir.clone(), c.detached))
            .unwrap_or((8090, None, true))
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
        assert!(state.containers.is_empty());
    }

    #[test]
    fn test_state_save_load() {
        // Create a temporary directory for config
        let temp_dir = tempdir().unwrap();
        env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let mut state = State::default();
        let container = ContainerInfo::new(
            "test_container".to_string(),
            "test".to_string(),
            9090,
            None,
            true,
            "latest".to_string(),
        );
        state.add_container(container).unwrap();

        // Save state
        state.save().unwrap();

        // Load state
        let loaded = State::load().unwrap();
        assert_eq!(loaded.containers.len(), 1);
        let loaded_container = loaded.containers.get("test_container").unwrap();
        assert_eq!(loaded_container.port, 9090);
        assert_eq!(loaded_container.name, "test");
    }

    #[test]
    fn test_container_management() {
        let mut state = State::default();

        // Add container
        let container = ContainerInfo::new(
            "test1".to_string(),
            "test-1".to_string(),
            8090,
            None,
            true,
            "latest".to_string(),
        );
        state.add_container(container).unwrap();
        assert_eq!(state.containers.len(), 1);

        // Get container
        let container = state.get_container("test1").unwrap();
        assert_eq!(container.name, "test-1");

        // Update status
        state
            .update_container_status("test1", true, Some("2024-01-01T00:00:00Z".to_string()))
            .unwrap();
        let container = state.get_container("test1").unwrap();
        assert_eq!(
            container.last_start,
            Some("2024-01-01T00:00:00Z".to_string())
        );

        // Remove container
        state.remove_container("test1").unwrap();
        assert!(state.containers.is_empty());
    }
}
