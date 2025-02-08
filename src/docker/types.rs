use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::cli::hub::Tag;

/// Represents a Fluree Docker image
#[derive(Debug, Clone)]
pub struct FlureeImage {
    pub tag: Tag,
    pub id: String,
    pub created: DateTime<Utc>,
    pub size: u64,
}

/// Represents a Fluree ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerInfo {
    pub alias: String,
    pub last_commit_time: String,
    pub commit_count: u64,
    pub size: u64,
    pub path: String,
    pub flakes_count: String,
    pub last_index: Option<u64>,
}

/// Represents container configuration options
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub host_port: u16,
    pub container_port: u16,
    pub data_mount_path: Option<String>,
}

impl ContainerConfig {
    /// Convert a PathBuf to a Docker-compatible mount path string
    pub fn path_to_mount_string(path: &std::path::Path) -> String {
        // Convert path to string, replacing backslashes with forward slashes
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string()
    }
}

impl From<&crate::config::FlureeConfig> for ContainerConfig {
    fn from(config: &crate::config::FlureeConfig) -> Self {
        Self {
            host_port: config.host_port,
            container_port: 8090,
            data_mount_path: config
                .data_mount
                .as_ref()
                .map(|path| Self::path_to_mount_string(path)),
        }
    }
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            host_port: 8090,
            container_port: 8090,
            data_mount_path: None,
        }
    }
}
