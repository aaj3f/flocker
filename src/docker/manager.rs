use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions,
    RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use chrono::TimeZone;
#[allow(unused_imports)]
use futures_util::stream::StreamExt;
use std::collections::HashMap;

use crate::cli::hub::Tag;
use crate::error::FlockerError;
use crate::state::ContainerInfo;
use crate::{ContainerStatus, Result};

use super::types::*;

/// Docker operations trait
#[async_trait]
pub trait DockerOperations {
    /// Get the status of a container
    async fn get_container_status(&self, container_id: &str) -> Result<ContainerStatus>;

    /// Start a stopped container
    async fn start_container(&self, container_id: &str) -> Result<()>;

    /// Stop a running container
    async fn stop_container(&self, container_id: &str) -> Result<()>;

    /// Remove a container
    async fn remove_container(&self, container_id: &str) -> Result<()>;

    /// Create and start a new container
    async fn create_and_start_container(
        &self,
        image_tag: &Tag,
        config: &ContainerConfig,
        name: &str,
    ) -> Result<ContainerInfo>;

    /// List ledgers in a container
    async fn list_ledgers(&self, container_id: &str) -> Result<Vec<LedgerInfo>>;

    /// Get ledger details
    async fn get_ledger_details(&self, container_id: &str, path: &str) -> Result<String>;

    /// Delete a ledger
    async fn delete_ledger(&self, container_id: &str, path: &str) -> Result<()>;

    /// Get container stats
    async fn get_container_stats(&self, container_id: &str) -> Result<String>;

    /// Get container logs
    async fn get_container_logs(&self, container_id: &str, tail: Option<&str>) -> Result<String>;

    /// Pull a Docker image
    async fn pull_image(&self, tag: &str) -> Result<()>;

    /// Get image by tag
    async fn get_image_by_tag(&self, tag_str: &str) -> Result<FlureeImage>;

    /// List local images
    async fn list_local_images(&self) -> Result<Vec<FlureeImage>>;
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
}

#[async_trait]
impl DockerOperations for DockerManager {
    async fn get_container_status(&self, container_id: &str) -> Result<ContainerStatus> {
        match self
            .docker
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await
        {
            Ok(container) => {
                let state = container.state.unwrap_or_default();
                let running = state.running.unwrap_or(false);

                let name = container
                    .name
                    .unwrap_or_default()
                    .trim_start_matches('/')
                    .to_string();
                let started_at = state.started_at;

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
                        name,
                        port,
                        data_dir,
                        started_at,
                    })
                } else {
                    Ok(ContainerStatus::Stopped {
                        id: container_id.to_string(),
                        name,
                        last_start: started_at,
                    })
                }
            }
            Err(_) => Ok(ContainerStatus::NotFound),
        }
    }

    async fn start_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to start container: {}", e)))?;
        Ok(())
    }

    async fn stop_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .stop_container(container_id, None::<StopContainerOptions>)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to stop container: {}", e)))?;
        Ok(())
    }

    async fn remove_container(&self, container_id: &str) -> Result<()> {
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

    async fn create_and_start_container(
        &self,
        image_tag: &Tag,
        config: &ContainerConfig,
        name: &str,
    ) -> Result<ContainerInfo> {
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
            Some(vec![bollard::models::PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(config.host_port.to_string()),
            }]),
        );

        // Convert path to Docker-compatible format
        let binds = config.data_mount_path.as_ref().map(|path| {
            let path = path.replace('\\', "/"); // Convert Windows paths to forward slashes
            vec![format!("{}:/opt/fluree-server/data:rw", path)]
        });

        let host_config = bollard::models::HostConfig {
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

        let options = CreateContainerOptions {
            name,
            platform: None,
        };

        let container = self
            .docker
            .create_container(Some(options), container_config)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to create container: {}", e)))?;

        self.docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to start container: {}", e)))?;

        let container_id = container.id;

        // Create container info
        let data_dir = config
            .data_mount_path
            .as_ref()
            .map(|path| crate::state::DataDirConfig::from_path_str(path));

        let info = ContainerInfo::new(
            container_id,
            name.to_string(),
            config.host_port,
            data_dir,
            true, // detached mode is handled in ContainerConfig
            image_tag.name().to_string(),
        );

        Ok(info)
    }

    async fn get_container_stats(&self, container_id: &str) -> Result<String> {
        use crate::cli::{format_bytes, Column, TableFormatter};

        let options = bollard::container::StatsOptions {
            stream: false,
            ..Default::default()
        };

        let mut stats = self.docker.stats(container_id, Some(options));

        if let Some(result) = futures_util::StreamExt::next(&mut stats).await {
            match result {
                Ok(stats) => {
                    // Calculate CPU percentage
                    let cpu_percent = if stats.cpu_stats.system_cpu_usage.is_some()
                        && stats.precpu_stats.system_cpu_usage.is_some()
                    {
                        let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64
                            - stats.precpu_stats.cpu_usage.total_usage as f64;
                        let system_delta = stats.cpu_stats.system_cpu_usage.unwrap() as f64
                            - stats.precpu_stats.system_cpu_usage.unwrap() as f64;
                        if system_delta > 0.0 && cpu_delta > 0.0 {
                            (cpu_delta / system_delta)
                                * 100.0
                                * stats.cpu_stats.online_cpus.unwrap_or(1) as f64
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    // Get memory stats
                    let mem_usage = stats.memory_stats.usage.unwrap_or(0);
                    let mem_limit = stats.memory_stats.limit.unwrap_or(1);
                    let mem_percent = (mem_usage as f64 / mem_limit as f64) * 100.0;

                    use crate::cli::terminal::get_terminal_width;

                    // Get terminal width and calculate column widths
                    let term_width = get_terminal_width() as usize;
                    let id_width = (term_width * 25) / 100; // 25% of width
                    let cpu_width = (term_width * 15) / 100; // 15% of width
                    let usage_width = (term_width * 25) / 100; // 25% of width
                    let limit_width = (term_width * 20) / 100; // 20% of width
                    let percent_width = (term_width * 15) / 100; // 15% of width

                    // Helper function to truncate strings
                    fn truncate(s: &str, width: usize) -> String {
                        if s.len() > width {
                            format!("{}...", &s[..width.saturating_sub(3)])
                        } else {
                            s.to_string()
                        }
                    }

                    // Create table formatter with dynamic widths
                    let formatter = TableFormatter::new(vec![
                        Column::new("CONTAINER ID", id_width),
                        Column::new("CPU %", cpu_width),
                        Column::new("MEM USAGE", usage_width),
                        Column::new("MEM LIMIT", limit_width),
                        Column::new("MEM %", percent_width),
                    ]);

                    // Format output with truncation
                    let mut output = String::new();
                    formatter.print_header();
                    formatter.print_row(&[
                        truncate(&container_id[..12], id_width),
                        truncate(&format!("{:.2}%", cpu_percent), cpu_width),
                        truncate(&format_bytes(mem_usage), usage_width),
                        truncate(&format_bytes(mem_limit), limit_width),
                        truncate(&format!("{:.1}%", mem_percent), percent_width),
                    ]);

                    output.push('\n'); // Add extra line for spacing

                    Ok(output)
                }
                Err(e) => Err(FlockerError::Docker(format!(
                    "Failed to get container stats: {}",
                    e
                ))),
            }
        } else {
            Err(FlockerError::Docker("No stats received".to_string()))
        }
    }

    async fn get_container_logs(&self, container_id: &str, tail: Option<&str>) -> Result<String> {
        let options = Some(bollard::container::LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: tail
                .map(|t| t.to_string())
                .unwrap_or_else(|| "1000".to_string()),
            timestamps: true,
            ..Default::default()
        });

        let mut logs = self.docker.logs(container_id, options);
        let mut log_lines = Vec::new();

        while let Some(log) = futures_util::StreamExt::next(&mut logs).await {
            match log {
                Ok(log) => {
                    let log_str = log.to_string();
                    // Docker timestamps are in format "2024-02-08T21:56:23.123456789Z"
                    if let Some(timestamp_end) = log_str.find(' ') {
                        if let Ok(timestamp) =
                            chrono::DateTime::parse_from_rfc3339(&log_str[..timestamp_end])
                        {
                            let formatted_time = timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
                            let message = &log_str[timestamp_end + 1..];
                            log_lines.push(format!("[{}] {}", formatted_time, message));
                        } else {
                            log_lines.push(log_str);
                        }
                    } else {
                        log_lines.push(log_str);
                    }
                }
                Err(e) => {
                    return Err(FlockerError::Docker(format!(
                        "Failed to get container logs: {}",
                        e
                    )));
                }
            }
        }

        // Reverse the lines so newest are at the bottom
        Ok(log_lines.join(""))
    }

    async fn list_ledgers(&self, container_id: &str) -> Result<Vec<LedgerInfo>> {
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

    async fn get_ledger_details(&self, container_id: &str, path: &str) -> Result<String> {
        let cat_cmd = vec!["cat", path];
        let json_content = self.exec_command(container_id, cat_cmd).await?;

        // Pretty print the JSON
        let json: serde_json::Value = serde_json::from_str(&json_content)
            .map_err(|e| FlockerError::Docker(format!("Failed to parse JSON: {}", e)))?;

        serde_json::to_string_pretty(&json)
            .map_err(|e| FlockerError::Docker(format!("Failed to format JSON: {}", e)))
    }

    async fn delete_ledger(&self, container_id: &str, path: &str) -> Result<()> {
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

    async fn pull_image(&self, tag: &str) -> Result<()> {
        let options = Some(bollard::image::CreateImageOptions {
            from_image: "fluree/server",
            tag,
            ..Default::default()
        });

        let mut pull_stream = self.docker.create_image(options, None, None);

        while let Some(info) = futures_util::StreamExt::next(&mut pull_stream).await {
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

    async fn get_image_by_tag(&self, tag_str: &str) -> Result<FlureeImage> {
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
        let created = chrono::DateTime::parse_from_rfc3339(&created_string)
            .map_err(|e| FlockerError::Docker(format!("Failed to parse created date: {}", e)))?
            .with_timezone(&chrono::Utc);
        let id = image.id.clone().ok_or(FlockerError::Docker(
            "Image ID not found on inspected image".to_string(),
        ))?;
        let size = image.size.ok_or(FlockerError::Docker(
            "Image size not found on inspected image".to_string(),
        ))? as u64;

        Ok(FlureeImage {
            tag: Tag::new(tag_full_name, created_string),
            id,
            created,
            size,
        })
    }

    async fn list_local_images(&self) -> Result<Vec<FlureeImage>> {
        let mut filters = HashMap::new();
        filters.insert(
            String::from("reference"),
            vec![String::from("fluree/server")],
        );

        let options = bollard::image::ListImagesOptions {
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
                    let created = chrono::Utc
                        .timestamp_opt(created_i64, 0)
                        .single()
                        .unwrap_or_else(chrono::Utc::now);

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
}

impl DockerManager {
    /// Execute a command in a container and return the output
    async fn exec_command(&self, container_id: &str, cmd: Vec<&str>) -> Result<String> {
        let exec = self
            .docker
            .create_exec(
                container_id,
                bollard::exec::CreateExecOptions {
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
            .start_exec(&exec.id, None::<bollard::exec::StartExecOptions>)
            .await
            .map_err(|e| FlockerError::Docker(format!("Failed to start exec: {}", e)))?;

        match output {
            bollard::exec::StartExecResults::Attached { mut output, .. } => {
                let mut result = String::new();
                while let Some(Ok(msg)) = futures_util::StreamExt::next(&mut output).await {
                    result.push_str(&msg.to_string());
                }
                Ok(result)
            }
            _ => Err(FlockerError::Docker("Unexpected exec output".to_string())),
        }
    }
}
