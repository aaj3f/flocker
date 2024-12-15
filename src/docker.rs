//! Docker operations for managing Fluree containers.
//!
//! This module provides functionality to interact with Docker, including:
//! - Listing and searching Fluree images
//! - Creating and managing containers
//! - Executing commands within containers

use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions,
    RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::image::{CreateImageOptions, ListImagesOptions};
use bollard::models::*;
use bollard::Docker;
use chrono::{DateTime, TimeZone, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cli::Tag;
use crate::error::FlockerError;
use crate::{ContainerStatus, Result};

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
}

impl FlureeImage {
    /// Format the image information with aligned columns
    pub fn display_string(&self, max_tag_length: usize) -> String {
        self.tag.pretty_print(Some(max_tag_length))
    }
}

/// Represents container configuration options
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub host_port: u16,
    pub container_port: u16,
    pub data_mount_path: Option<String>,
    pub detach: bool,
}

impl ContainerConfig {
    /// Convert a PathBuf to a Docker-compatible mount path string
    fn path_to_mount_string(path: &std::path::Path) -> String {
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
            detach: config.detached,
        }
    }
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            host_port: 8090,
            container_port: 8090,
            data_mount_path: None,
            detach: true,
        }
    }
}

/// Docker operations manager
pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    /// Create a new DockerManager instance
    pub async fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| FlockerError::Docker(format!("Failed to connect to Docker: {}", e)))?;
        Ok(Self { docker })
    }

    /// Execute a command in a container and return the output
    async fn exec_command(&self, container_id: &str, cmd: Vec<&str>) -> Result<String> {
        let exec = self
            .docker
            .create_exec(
                container_id,
                CreateExecOptions {
                    cmd: Some(cmd),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to create exec: {}", e)))?;

        let output = self
            .docker
            .start_exec(&exec.id, None::<StartExecOptions>)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to start exec: {}", e)))?;

        match output {
            bollard::exec::StartExecResults::Attached { mut output, .. } => {
                let mut result = String::new();
                while let Some(Ok(msg)) = output.next().await {
                    result.push_str(&msg.to_string());
                }
                Ok(result)
            }
            _ => Err(FlockerError::Docker("Unexpected exec output".to_string())),
        }
    }

    /// List all ledger files in the container
    pub async fn list_ledgers(&self, container_id: &str) -> Result<Vec<LedgerInfo>> {
        // First, find all .json files recursively (excluding commit directory)
        let find_cmd = vec![
            "find",
            "/opt/fluree-server/data",
            "-type",
            "f",
            "-name",
            "*.json",
            "-not",
            "-path",
            "*/commit/*",
        ];

        let output = self.exec_command(container_id, find_cmd).await?;
        let mut ledgers = Vec::new();

        for path in output.lines() {
            if path.trim().is_empty() {
                continue;
            }

            // Read the JSON file
            let cat_cmd = vec!["cat", path];
            let json_content = self.exec_command(container_id, cat_cmd).await?;

            // Parse the JSON content
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_content) {
                if let Some(ledger_alias) = json.get("ledgerAlias").and_then(|v| v.as_str()) {
                    let last_commit_time = json
                        .get("branches")
                        .and_then(|b| b.get(0))
                        .and_then(|b| b.get("commit"))
                        .and_then(|c| c.get("time"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");

                    let commit_count = json
                        .get("branches")
                        .and_then(|b| b.get(0))
                        .and_then(|b| b.get("commit"))
                        .and_then(|c| c.get("data"))
                        .and_then(|d| d.get("t"))
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0);

                    let size = json
                        .get("branches")
                        .and_then(|b| b.get(0))
                        .and_then(|b| b.get("commit"))
                        .and_then(|c| c.get("data"))
                        .and_then(|d| d.get("size"))
                        .and_then(|s| s.as_u64())
                        .unwrap_or(0);

                    ledgers.push(LedgerInfo {
                        alias: ledger_alias.to_string(),
                        last_commit_time: last_commit_time.to_string(),
                        commit_count,
                        size,
                        path: path.to_string(),
                    });
                }
            }
        }

        Ok(ledgers)
    }

    /// Get detailed information about a specific ledger
    pub async fn get_ledger_details(&self, container_id: &str, path: &str) -> Result<String> {
        let cat_cmd = vec!["cat", path];
        let json_content = self.exec_command(container_id, cat_cmd).await?;

        // Pretty print the JSON
        let json: serde_json::Value = serde_json::from_str(&json_content)
            .map_err(|e| FlockerError::Docker(format!("Failed to parse JSON: {}", e)))?;

        serde_json::to_string_pretty(&json)
            .map_err(|e| FlockerError::Docker(format!("Failed to format JSON: {}", e)))
    }

    /// Delete a ledger directory
    pub async fn delete_ledger(&self, container_id: &str, path: &str) -> Result<()> {
        // Extract the directory path from the JSON file path
        let dir_path = std::path::Path::new(path)
            .parent()
            .ok_or_else(|| FlockerError::Docker("Invalid ledger path".to_string()))?
            .to_str()
            .ok_or_else(|| FlockerError::Docker("Invalid path encoding".to_string()))?;

        // Remove the directory and all its contents
        let rm_cmd = vec!["rm", "-rf", dir_path];
        self.exec_command(container_id, rm_cmd).await?;

        Ok(())
    }

    /// Pull a Docker image with progress reporting
    pub async fn pull_image(&self, tag: &str) -> Result<()> {
        let options = Some(CreateImageOptions {
            from_image: "fluree/server",
            tag,
            ..Default::default()
        });

        let mut pull_stream = self.docker.create_image(options, None, None);

        while let Some(info) = pull_stream.next().await {
            match info {
                Ok(output) => {
                    if let Some(status) = output.status {
                        if let Some(progress) = output.progress {
                            println!("{}: {}", status, progress);
                        } else {
                            println!("{}", status);
                        }
                    }
                }
                Err(e) => {
                    return Err(FlockerError::Docker(format!("Failed to pull image: {}", e)));
                }
            }
        }

        Ok(())
    }

    /// Get the local Docker image by tag
    pub async fn get_image_by_tag(&self, tag_str: &str) -> Result<FlureeImage> {
        let mut filters = HashMap::new();
        filters.insert(
            String::from("reference"),
            vec![String::from("fluree/server")],
        );

        let tag_full_name = format!("fluree/server:{}", tag_str);

        let image = self
            .docker
            .inspect_image(&tag_full_name)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to get image: {}", e)))?;

        let created_string = image.created.unwrap_or("<unknown>".to_string());
        let created = DateTime::parse_from_rfc3339(&created_string)
            .map_err(|e| FlockerError::Docker(format!("Failed to parse created date: {}", e)))?
            .with_timezone(&Utc);
        let id = image.id.clone().ok_or(FlockerError::Docker(
            "Image ID not found on inspacted image".to_string(),
        ))?;
        let size = image.size.ok_or(FlockerError::Docker(
            "Image size not found on inspacted image".to_string(),
        ))? as u64;
        Ok(FlureeImage {
            tag: Tag::new(tag_full_name, created_string.to_string()),
            id,
            created,
            size,
        })
    }

    /// List local Fluree images
    pub async fn list_local_images(&self) -> Result<Vec<FlureeImage>> {
        let mut filters = HashMap::new();
        filters.insert(
            String::from("reference"),
            vec![String::from("fluree/server")],
        );

        let options = ListImagesOptions {
            filters,
            ..Default::default()
        };

        let images = self.docker.list_images(Some(options)).await.map_err(|e| {
            FlockerError::Docker(format!(
                "Failed to list images. Is the docker daemon running? ({})",
                e
            ))
        })?;

        let mut fluree_images = Vec::new();
        for image in images {
            for tag in image.repo_tags {
                if tag.starts_with("fluree/server:") {
                    let created_i64 = image.created;
                    let created = Utc
                        .timestamp_opt(created_i64, 0)
                        .single()
                        .unwrap_or_else(Utc::now);

                    fluree_images.push(FlureeImage {
                        tag: Tag::new(tag, created.to_rfc3339()),
                        id: image.id.clone(),
                        created,
                        size: image.size as u64,
                    });
                }
            }
        }

        Ok(fluree_images)
    }

    /// Check if a port is already in use by another container
    pub async fn is_port_in_use(&self, port: u16) -> Result<bool> {
        let mut filters = HashMap::new();
        filters.insert(String::from("status"), vec![String::from("running")]);

        let options = Some(ListContainersOptions {
            filters,
            ..Default::default()
        });

        let containers = self
            .docker
            .list_containers(options)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to list containers: {}", e)))?;

        for container in containers {
            if let Some(ports) = container.ports {
                for port_mapping in ports {
                    if let Some(public_port) = port_mapping.public_port {
                        if public_port == port {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Get the status of a container
    pub async fn get_container_status(&self, container_id: &str) -> Result<ContainerStatus> {
        match self
            .docker
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await
        {
            Ok(container) => {
                let state = container.state.unwrap_or_default();
                let running = state.running.unwrap_or(false);

                if running {
                    let host_config = container.host_config.unwrap_or_default();

                    // Extract port mapping
                    let port = host_config
                        .port_bindings
                        .and_then(|bindings| {
                            bindings
                                .get("8090/tcp")
                                .and_then(|binding| binding.as_ref())
                                .and_then(|binding| binding.first())
                                .and_then(|port| port.host_port.as_ref())
                                .and_then(|port| port.parse().ok())
                        })
                        .unwrap_or(8090);

                    // Extract data directory
                    let data_dir = host_config.binds.and_then(|binds| binds.first().cloned());

                    Ok(ContainerStatus::Running {
                        id: container_id.to_string(),
                        name: container.name.unwrap_or_default(),
                        port,
                        data_dir,
                    })
                } else {
                    Ok(ContainerStatus::Stopped {
                        id: container_id.to_string(),
                    })
                }
            }
            Err(_) => Ok(ContainerStatus::NotFound),
        }
    }

    /// Stop a running container
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .stop_container(container_id, None::<StopContainerOptions>)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to stop container: {}", e)))?;
        Ok(())
    }

    /// Remove a container
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        let options = Some(RemoveContainerOptions {
            force: true,
            ..Default::default()
        });

        self.docker
            .remove_container(container_id, options)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to remove container: {}", e)))?;
        Ok(())
    }

    /// Create and start a new Fluree container
    pub async fn create_and_start_container(
        &self,
        image_tag: &Tag,
        config: &ContainerConfig,
    ) -> Result<String> {
        // Check if port is already in use
        if self.is_port_in_use(config.host_port).await? {
            return Err(FlockerError::Docker(format!(
                "Port {} is already in use by another container",
                config.host_port
            )));
        }

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(format!("{}/tcp", config.container_port), HashMap::new());

        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", config.container_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(config.host_port.to_string()),
            }]),
        );

        // Convert path to Docker-compatible format
        let binds = config.data_mount_path.as_ref().map(|path| {
            let path = path.replace('\\', "/"); // Convert Windows paths to forward slashes
            vec![format!("{}:/opt/fluree-server/data:rw", path)]
        });

        let host_config = HostConfig {
            port_bindings: Some(port_bindings),
            binds,
            ..Default::default()
        };

        let container_config = Config {
            image: Some(image_tag.name().to_string()),
            exposed_ports: Some(exposed_ports),
            host_config: Some(host_config),
            ..Default::default()
        };

        let container = self
            .docker
            .create_container(None::<CreateContainerOptions<String>>, container_config)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to create container: {}", e)))?;

        self.docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to start container: {}", e)))?;

        Ok(container.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_container_config_default() {
        let config = ContainerConfig::default();
        assert_eq!(config.host_port, 8090);
        assert_eq!(config.container_port, 8090);
        assert!(config.data_mount_path.is_none());
        assert!(config.detach);
    }

    #[test]
    fn test_fluree_image_created_relative() {
        let now = Utc::now();
        let image = FlureeImage {
            tag: Tag::new(
                "fluree/server:latest".to_string(),
                (now - Duration::days(2)).to_rfc3339(),
            ),
            id: "test".to_string(),
            created: now - Duration::days(2),
            size: 1000,
        };
        assert!(image.display_string(10).contains("2 days"));
    }

    #[test]
    fn test_fluree_image_display_string() {
        let now = Utc::now();
        let image = FlureeImage {
            tag: Tag::new(
                "fluree/server:latest".to_string(),
                (now - Duration::days(1)).to_rfc3339(),
            ),
            id: "test".to_string(),
            created: now - Duration::days(1),
            size: 1000,
        };
        let display = image.display_string(10);
        assert!(display.contains("latest"));
        assert!(display.contains("1 day"));
    }
}
