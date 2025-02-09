//! State management for Flocker.
//!
//! This module handles persistent state including user preferences
//! and running container information.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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

    /// Create a new DataDirConfig from a string path
    pub fn from_path_str(path_str: &str) -> Self {
        Self::from_path(&std::path::PathBuf::from(path_str))
    }

    /// Create a new DataDirConfig from a PathBuf
    pub fn from_path(path: &PathBuf) -> Self {
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let relative_path = if path.starts_with(&current_dir) {
            pathdiff::diff_paths(path, &current_dir)
        } else {
            None
        };
        DataDirConfig {
            relative_path,
            absolute_path: path.clone(),
        }
    }

    pub fn from_current_dir(current_dir: &Path) -> Self {
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
    /// Config directory configuration
    pub config_dir: Option<DataDirConfig>,
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
        config_dir: Option<DataDirConfig>,
        image_tag: String,
    ) -> Self {
        let last_start =
            Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        Self {
            id,
            name,
            port,
            data_dir,
            config_dir,
            image_tag,
            last_start,
        }
    }
}

/// Persistent state for the Flocker application
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    /// Known containers, mapped by ID
    pub containers: std::collections::HashMap<String, ContainerInfo>,
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

    pub fn clear() -> Result<()> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            fs::remove_file(&config_path).map_err(|e| FlockerError::ConfigFile {
                path: config_path.clone(),
                message: "Failed to remove config file".to_string(),
                source: e.into(),
            })?;
        }
        Ok(())
    }

    /// Save current state to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                FlockerError::Config(format!("Failed to create config directory: {}", e))
            })?;
        }

        // Serialize state to JSON with pretty printing
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| FlockerError::Config(format!("Failed to serialize config: {}", e)))?;

        // Write content to file
        fs::write(&config_path, content)
            .map_err(|e| FlockerError::Config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Add or update a container in the state
    pub fn add_container(&mut self, info: ContainerInfo) -> Result<()> {
        // Check if name is already in use by a different container
        if let Some(existing) = self
            .containers
            .values()
            .find(|c| c.name == info.name && c.id != info.id)
        {
            return Err(FlockerError::Config(format!(
                "Container name '{}' is already in use by container {}",
                info.name, existing.id
            )));
        }

        // Save first to ensure directory exists
        self.save()?;
        self.containers.insert(info.id.clone(), info);
        self.save()
    }

    /// Remove a container from the state
    pub fn remove_container(&mut self, container_id: &str) -> Result<()> {
        if !self.containers.contains_key(container_id) {
            return Err(FlockerError::Config(format!(
                "Container {} not found in state",
                container_id
            )));
        }
        self.containers.remove(container_id);
        self.save()
    }

    /// Find containers by name
    pub fn find_containers_by_name(&self, name: &str) -> Vec<&ContainerInfo> {
        self.containers
            .values()
            .filter(|c| c.name.contains(name))
            .collect()
    }

    /// Get a container by ID
    pub fn get_container(&self, container_id: &str) -> Option<&ContainerInfo> {
        self.containers.get(container_id)
    }

    /// Get all known containers
    pub fn get_containers(&self) -> Vec<&ContainerInfo> {
        let mut containers: Vec<&ContainerInfo> = self.containers.values().collect();
        containers.sort_by(|a, b| b.last_start.cmp(&a.last_start));
        containers
    }

    pub fn update_container_start_time(
        &mut self,
        container_id: &str,
        start_time: String,
    ) -> Result<()> {
        if let Some(container) = self.containers.get_mut(container_id) {
            container.last_start = Some(start_time);
        }
        self.save()
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
    pub fn get_default_settings(&self) -> (u16, Option<DataDirConfig>) {
        self.containers
            .values()
            .max_by_key(|c| c.last_start.as_ref())
            .map(|c| (c.port, c.data_dir.clone()))
            .unwrap_or((8090, None))
    }

    /// Get the path to the config file
    fn config_path() -> Result<PathBuf> {
        // Check for test environment variable first
        if let Ok(test_config_dir) = std::env::var("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(test_config_dir).join("config.json"));
        }

        // Use default config path for normal operation
        let proj_dirs = ProjectDirs::from("com", "fluree", "flocker").ok_or_else(|| {
            FlockerError::Config("Failed to determine config directory".to_string())
        })?;

        Ok(proj_dirs.config_dir().join("config.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::{parallel, serial};
    use std::env;
    use tempfile::tempdir;

    #[test]
    #[parallel]
    fn test_state_default() {
        let state = State::default();
        assert!(state.containers.is_empty());
    }

    #[test]
    #[serial]
    fn test_state_save_load() {
        // Create a temporary directory for config
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_owned();
        env::set_var("XDG_CONFIG_HOME", &temp_path);
        State::clear().unwrap();

        // Create initial state
        let mut state = State::default();
        let container = ContainerInfo::new(
            "test1".to_string(),
            "test".to_string(),
            8090,
            None,
            None,
            "latest".to_string(),
        );
        state.containers.insert(container.id.clone(), container);

        // Save state
        let config_path = State::config_path().unwrap();
        println!("Config path: {:?}", config_path);
        state.save().unwrap();

        // Verify file contents
        let content = fs::read_to_string(&config_path).unwrap();
        println!("File contents:\n{}", content);

        // Load state and verify
        let loaded = State::load().unwrap();
        assert_eq!(loaded.containers.len(), 1);
        let loaded_container = loaded.containers.get("test1").unwrap();
        assert_eq!(loaded_container.port, 8090);
        assert_eq!(loaded_container.name, "test");

        // Keep temp_dir alive until end of test
        drop(temp_dir);
    }

    #[test]
    #[parallel]
    fn test_container_management() {
        let mut state = State::default();

        // Add container
        let container = ContainerInfo::new(
            "test1".to_string(),
            "test-1".to_string(),
            8090,
            None,
            None,
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

        // Test removing non-existent container
        assert!(state.remove_container("test1").is_err());
    }

    #[test]
    #[parallel]
    fn test_container_name_uniqueness() {
        let mut state = State::default();

        // Add first container
        let container1 = ContainerInfo::new(
            "test1".to_string(),
            "test".to_string(),
            8090,
            None,
            None,
            "latest".to_string(),
        );
        state.add_container(container1).unwrap();

        // Try to add second container with same name
        let container2 = ContainerInfo::new(
            "test2".to_string(),
            "test".to_string(),
            8091,
            None,
            None,
            "latest".to_string(),
        );
        assert!(state.add_container(container2).is_err());
    }

    #[test]
    #[parallel]
    fn test_find_containers() {
        let mut state = State::default();

        // Add containers with different states
        let container1 = ContainerInfo::new(
            "test1".to_string(),
            "test-1".to_string(),
            8090,
            None,
            None,
            "latest".to_string(),
        );
        state.add_container(container1).unwrap();

        let mut container2 = ContainerInfo::new(
            "test2".to_string(),
            "test-2".to_string(),
            8091,
            None,
            None,
            "latest".to_string(),
        );
        container2.last_start = Some("2024-01-01T00:00:00Z".to_string());
        state.add_container(container2).unwrap();

        // Test finding by name
        let found = state.find_containers_by_name("test-1");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "test1");
    }
}
