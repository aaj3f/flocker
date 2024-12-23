//! CLI interface for Flocker.
//!
//! This module provides the interactive command-line interface,
//! including prompts, styling, and user input handling.

use clap::Parser;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use pad::PadStr;
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::debug;

use crate::config::FlureeConfig;
use crate::docker::{DockerManager, DockerOperations, FlureeImage};
use crate::state::{DataDirConfig, State};
use crate::{ContainerStatus, FlockerError, Result};

/// Available actions when a container is running
#[derive(Debug)]
enum RunningContainerAction {
    ViewStats,
    ViewLogs,
    ListLedgers,
    Stop,
    StopAndDestroy,
    Exit,
}

/// Available actions when viewing a ledger
#[derive(Debug)]
enum LedgerAction {
    ViewDetails,
    Delete,
    Return,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tag {
    name: String,
    last_updated: String,
}

impl Tag {
    pub fn new(name: String, last_updated: String) -> Self {
        Tag { name, last_updated }
    }

    pub fn pretty_print(&self, max_tag_length: Option<usize>) -> String {
        let name = if let Some(max_tag_length) = max_tag_length {
            self.name
                .pad_to_width_with_alignment(max_tag_length, pad::Alignment::Left)
        } else {
            self.name.clone()
        };
        format!(
            "fluree/server:{} (updated {})",
            name,
            self.pretty_print_time()
                .unwrap_or_else(|_| "unknown time ago".to_string())
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn pretty_print_time(&self) -> Result<String> {
        let now_time = chrono::Utc::now();
        let last_updated_time =
            chrono::DateTime::parse_from_rfc3339(&self.last_updated).map_err(|e| {
                crate::error::FlockerError::Docker(format!("Failed to parse date: {}", e))
            })?;
        let duration = now_time.signed_duration_since(last_updated_time);
        let days = duration.num_days();
        let weeks = days / 7;
        let months = days / 30;
        let years = days / 365;
        Ok(if years > 0 {
            format!("{} years ago", years)
        } else if months > 0 {
            format!("{} months ago", months)
        } else if weeks > 0 {
            format!("{} weeks ago", weeks)
        } else {
            format!("{} days ago", days)
        })
    }
}

#[derive(Deserialize)]
struct TagResponse {
    results: Vec<Tag>,
    next: Option<String>,
}

impl RunningContainerAction {
    fn variants() -> Vec<&'static str> {
        vec![
            "View Container Stats",
            "View Container Logs",
            "List Ledgers",
            "Stop Container",
            "Stop and Destroy Container",
            "Exit Flocker",
        ]
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::ViewStats),
            1 => Some(Self::ViewLogs),
            2 => Some(Self::ListLedgers),
            3 => Some(Self::Stop),
            4 => Some(Self::StopAndDestroy),
            5 => Some(Self::Exit),
            _ => None,
        }
    }
}

impl LedgerAction {
    fn variants() -> Vec<&'static str> {
        vec!["See More Details", "Delete Ledger", "Return to Ledger List"]
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::ViewDetails),
            1 => Some(Self::Delete),
            2 => Some(Self::Return),
            _ => None,
        }
    }
}

/// CLI manager for handling user interaction
pub struct CliState {
    theme: ColorfulTheme,
    state: State,
    config: Option<FlureeConfig>,
}

impl Default for CliState {
    fn default() -> Self {
        Self {
            theme: ColorfulTheme::default(),
            state: State::load().unwrap_or_default(),
            config: None,
        }
    }
}

impl CliState {
    /// Create a new CLI instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new container to state
    pub fn add_container(&mut self, info: crate::state::ContainerInfo) -> Result<()> {
        self.state.add_container(info)
    }

    /// Load state from disk
    pub fn load_state(&mut self) -> Result<&State> {
        self.state = match State::load() {
            Ok(state) => {
                debug!("State loaded: {:?}", state);
                state
            }
            Err(e) => {
                println!(
                    "{}\n{}",
                    style("Failed to load state").red().bold(),
                    style(e).red()
                );
                State::default()
            }
        };
        Ok(&self.state)
    }

    /// Get container name from user
    pub fn get_container_name(&self) -> Result<String> {
        Input::with_theme(&self.theme)
            .with_prompt("Enter a name for this container")
            .interact()
            .map_err(|e| FlockerError::UserInput(e.to_string()))
    }

    /// Try to run an existing container if one is saved in the state
    pub async fn try_running_existing_container(
        &mut self,
        docker: &DockerManager,
    ) -> Result<Option<String>> {
        let containers = self.state.get_containers().to_vec(); // Clone to avoid borrow issues
        if containers.is_empty() {
            return Ok(None);
        }

        // Get status for all containers
        let mut container_strings = Vec::new();
        for c in &containers {
            let status = docker
                .get_container_status(&c.id)
                .await
                .unwrap_or(ContainerStatus::NotFound);
            let status_color = match status {
                ContainerStatus::Running { .. } => style("running").green(),
                ContainerStatus::Stopped { .. } => style("stopped").yellow(),
                ContainerStatus::NotFound => style("not found").red(),
            };

            container_strings.push(format!(
                "{} [{}] (Image: {}, Port: {}, Last Start: {})",
                style(&c.name).cyan(),
                status_color,
                style(&c.image_tag).yellow(),
                style(&c.port).green(),
                c.last_start
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "Never".to_string())
            ));
        }

        // Add option for new container
        let mut options = vec!["Create new container".to_string()];
        options.extend(container_strings);

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Select a container or create a new one")
            .items(&options)
            .default(0)
            .interact()
            .map_err(|e| FlockerError::UserInput(e.to_string()))?;

        if selection == 0 {
            return Ok(None);
        }

        let selected_container = containers[selection - 1].clone(); // Clone to avoid borrow issues
        let status = docker.get_container_status(&selected_container.id).await?;

        if matches!(status, ContainerStatus::NotFound) {
            println!(
                "\n{} {}",
                style("Container no longer exists:").yellow(),
                style(&selected_container.name).cyan()
            );
            self.state.remove_container(&selected_container.id)?;
            return Ok(None);
        }

        self.handle_running_container(docker, status).await?;
        Ok(Some(selected_container.id))
    }

    /// Handle ledger management for a container
    async fn handle_ledger_management(
        &self,
        docker: &DockerManager,
        container_id: &str,
    ) -> Result<()> {
        loop {
            // Get list of ledgers
            let ledgers = docker.list_ledgers(container_id).await?;

            if ledgers.is_empty() {
                println!("\n{}", style("No ledgers found").yellow());
                return Ok(());
            }

            // Format ledger information for display
            let ledger_strings: Vec<String> = ledgers
                .iter()
                .map(|ledger| {
                    format!(
                        "{} (Last commit: {}, Commits: {}, Size: {} bytes)",
                        style(&ledger.alias).cyan(),
                        style(&ledger.last_commit_time).yellow(),
                        style(&ledger.commit_count).green(),
                        style(&ledger.size).blue()
                    )
                })
                .collect();

            // Let user select a ledger
            let selection = Select::with_theme(&self.theme)
                .with_prompt("Select a ledger")
                .items(&ledger_strings)
                .default(0)
                .interact()
                .map_err(|e| FlockerError::UserInput(e.to_string()))?;

            let selected_ledger = &ledgers[selection];

            // Show ledger actions
            let action_selection = Select::with_theme(&self.theme)
                .with_prompt("What would you like to do?")
                .items(&LedgerAction::variants())
                .default(0)
                .interact()
                .map_err(|e| FlockerError::UserInput(e.to_string()))?;

            match LedgerAction::from_index(action_selection) {
                Some(LedgerAction::ViewDetails) => {
                    let details = docker
                        .get_ledger_details(container_id, &selected_ledger.path)
                        .await?;
                    println!("\n{}", style("Ledger Details:").cyan().bold());
                    println!("{}", details);
                }
                Some(LedgerAction::Delete) => {
                    println!(
                        "\n{} {}",
                        style("WARNING:").red().bold(),
                        style("This will permanently delete the ledger and all its data!").red()
                    );

                    let confirmation: String = Input::with_theme(&self.theme)
                        .with_prompt("Type 'delete' to confirm")
                        .validate_with(|input: &String| -> Result<()> {
                            if input == "delete" {
                                Ok(())
                            } else {
                                Err(FlockerError::UserInput(
                                    "Type 'delete' to confirm".to_string(),
                                ))
                            }
                        })
                        .interact()
                        .map_err(|e| FlockerError::UserInput(e.to_string()))?;

                    if confirmation == "delete" {
                        docker
                            .delete_ledger(container_id, &selected_ledger.path)
                            .await?;
                        println!("\n{}", style("Ledger deleted successfully").green().bold());
                        // Break the loop to refresh ledger list
                        break;
                    }
                }
                Some(LedgerAction::Return) | None => {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Display available Fluree images and get user selection
    pub async fn select_image(&self, docker: &DockerManager) -> Result<FlureeImage> {
        let remote_or_local_selection = Select::with_theme(&self.theme)
            .with_prompt("Do you want to list remote or local Fluree images?")
            .items(&["Remote (Docker Hub)", "Local"])
            .default(0)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))?;

        match remote_or_local_selection {
            0 => self.select_remote_image(docker).await,
            1 => self.select_local_image(docker).await,
            _ => unreachable!(),
        }
    }

    pub async fn select_remote_image(&self, docker: &DockerManager) -> Result<FlureeImage> {
        println!(
            "{}",
            style("Fetching available images from Docker Hub...").cyan()
        );

        let client = Client::new();
        let mut url = "https://hub.docker.com/v2/repositories/fluree/server/tags".to_string();
        let mut tags = Vec::new();

        loop {
            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| {
                    crate::error::FlockerError::Docker(format!("Failed to fetch tags: {}", e))
                })
                .and_then(|res| {
                    if res.status().is_success() {
                        Ok(res)
                    } else {
                        Err(crate::error::FlockerError::Docker(format!(
                            "Failed to fetch tags: {}",
                            res.status()
                        )))
                    }
                })?;

            let response: TagResponse = response.json().await.map_err(|e| {
                crate::error::FlockerError::Docker(format!("Failed to parse tags response: {}", e))
            })?;

            tags.extend(response.results.into_iter());

            if let Some(next_url) = response.next {
                url = next_url;
            } else {
                break;
            }
        }

        // Find the longest tag name for alignment
        let max_tag_length = tags
            .iter()
            .map(|tag| tag.name.len())
            .max()
            .unwrap_or_default();

        let tag_strings_to_display = tags
            .iter()
            .map(|tag| {
                let time = tag
                    .pretty_print_time()
                    .unwrap_or_else(|_| "unknown time ago".to_string());
                format!(
                    "fluree/server:{} (updated {})",
                    tag.name
                        .pad_to_width_with_alignment(max_tag_length, pad::Alignment::Left),
                    style(time).cyan()
                )
            })
            .collect::<Vec<String>>();

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Select a Fluree image")
            .items(tag_strings_to_display.as_slice())
            .default(0)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))?;

        let selected_tag = &tags[selection].name;

        self.pull_remote_image(docker, selected_tag).await?;

        let image = docker.get_image_by_tag(selected_tag).await?;

        Ok(image)
    }

    async fn pull_remote_image(&self, docker: &DockerManager, tag: &str) -> Result<()> {
        println!(
            "\n{} {}",
            style("Pulling image").cyan(),
            style(format!("fluree/server:{}", tag)).cyan().bold()
        );

        docker.pull_image(tag).await?;

        println!(
            "\n{} {}",
            style("Successfully pulled").green(),
            style(format!("fluree/server:{}", tag)).green().bold()
        );

        Ok(())
    }

    pub async fn select_local_image(&self, docker: &DockerManager) -> Result<FlureeImage> {
        let images = docker.list_local_images().await?;

        if images.is_empty() {
            println!("{}", style("No local Fluree images found.").yellow());
            println!("Please pull an image first using:");
            println!("{}", style("docker pull fluree/server:latest").cyan());
            std::process::exit(1);
        }

        // Find the longest tag for alignment
        let max_tag_length = images
            .iter()
            .map(|img| img.tag.name.len())
            .max()
            .unwrap_or_default();

        let image_strings: Vec<String> = images
            .iter()
            .map(|img| img.tag.pretty_print(Some(max_tag_length)))
            .collect();

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Select a Fluree image")
            .items(&image_strings)
            .default(0)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))?;

        Ok(images[selection].clone())
    }

    /// Get port configuration from user
    pub fn get_port_config(&mut self) -> Result<u16> {
        let (default_port, _, _) = self.state.get_default_settings();

        Input::with_theme(&self.theme)
            .with_prompt("Enter host port to map to container port 8090")
            .default(default_port)
            .validate_with(|input: &u16| {
                if *input < 1024 {
                    Err("Port must be >= 1024")
                } else {
                    Ok(())
                }
            })
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    /// Get data mount configuration from user
    pub fn get_data_mount_config(&mut self) -> Result<Option<PathBuf>> {
        let use_mount = Confirm::with_theme(&self.theme)
            .with_prompt("Mount a local directory for data persistence?")
            .default(true)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))?;

        if !use_mount {
            return Ok(None);
        }

        let current_dir = std::env::current_dir()?;
        let (_, default_data_dir, _) = self.state.get_default_settings();
        let default_path =
            default_data_dir.unwrap_or_else(|| DataDirConfig::from_current_dir(&current_dir));

        let path_str: String = Input::with_theme(&self.theme)
            .with_prompt("Enter path to mount (will be created if it doesn't exist)")
            .default(default_path.display_relative_path())
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))?;

        // Convert relative path to absolute path
        let absolute_path = if PathBuf::from(&path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            current_dir.join(&path_str)
        };

        // Create directory if it doesn't exist
        if !absolute_path.exists() {
            std::fs::create_dir_all(&absolute_path)?;
            println!("{}", style("Created directory: ").green().bold());
            println!("{}", style(absolute_path.display()).cyan());
        }

        // Get the absolute path with all symlinks resolved
        let canonical_path =
            absolute_path
                .canonicalize()
                .map_err(|e| crate::error::FlockerError::ConfigFile {
                    message: "Failed to resolve path".to_string(),
                    path: absolute_path.clone(),
                    source: e.into(),
                })?;

        Ok(Some(canonical_path))
    }

    /// Get detach mode configuration from user
    pub fn get_detach_config(&mut self) -> Result<bool> {
        let (_, _, default_detached) = self.state.get_default_settings();

        Confirm::with_theme(&self.theme)
            .with_prompt("Run container in background (detached mode)?")
            .default(default_detached)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    /// Get complete configuration from user
    pub async fn get_config(
        &mut self,
        docker: &DockerManager,
    ) -> Result<(FlureeImage, FlureeConfig, String)> {
        let image = self.select_image(docker).await?;
        let name = self.get_container_name()?;
        let host_port = self.get_port_config()?;
        let data_mount = self.get_data_mount_config()?;
        let detached = self.get_detach_config()?;

        let config = FlureeConfig::new(host_port, data_mount.clone(), detached);
        config.validate()?;

        self.config = Some(config.clone());

        Ok((image, config, name))
    }

    /// Handle running container actions
    pub async fn handle_running_container(
        &mut self,
        docker: &DockerManager,
        status: ContainerStatus,
    ) -> Result<()> {
        match status {
            ContainerStatus::Running {
                id,
                name,
                port,
                data_dir,
                started_at: _,
            } => {
                println!(
                    "\n{} {}",
                    style("Found running Fluree container:").green(),
                    style(&name).cyan()
                );
                println!("Container ID: {}", style(&id[..12]).cyan());
                println!("Mapped port: {}", style(port).cyan());
                if let Some(dir) = data_dir {
                    println!("Data directory: {}", style(dir).cyan());
                }

                let selection = Select::with_theme(&self.theme)
                    .with_prompt("What would you like to do?")
                    .items(&RunningContainerAction::variants())
                    .default(0)
                    .interact()
                    .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))?;

                match RunningContainerAction::from_index(selection) {
                    Some(RunningContainerAction::Stop) => {
                        docker.stop_container(&id).await?;
                        println!("\n{}", style("Container stopped successfully").green());
                    }
                    Some(RunningContainerAction::StopAndDestroy) => {
                        docker.remove_container(&id).await?;
                        println!("\n{}", style("Container removed successfully").green());
                        self.state.remove_container(&id)?;
                    }
                    Some(RunningContainerAction::ViewStats) => {
                        println!(
                            "\n{}",
                            style("Container stats not yet implemented").yellow()
                        );
                    }
                    Some(RunningContainerAction::ViewLogs) => {
                        println!("\n{}", style("Container logs not yet implemented").yellow());
                    }
                    Some(RunningContainerAction::ListLedgers) => {
                        self.handle_ledger_management(docker, &id).await?;
                    }
                    Some(RunningContainerAction::Exit) => {
                        println!("\n{}", style("Exiting...").yellow());
                        std::process::exit(0);
                    }
                    None => unreachable!(),
                }
            }
            ContainerStatus::Stopped {
                id,
                name,
                last_start,
            } => {
                println!(
                    "\n{} {} ({})",
                    style("Found stopped container:").yellow(),
                    style(&name).cyan(),
                    style(&id[..12]).dim()
                );
                if let Some(time) = last_start {
                    println!("Last started: {}", style(time).yellow());
                }

                let options = vec!["Start this container", "Destroy this container"];
                let selection = Select::with_theme(&self.theme)
                    .with_prompt("What would you like to do?")
                    .items(&options)
                    .default(0)
                    .interact()
                    .map_err(|e| FlockerError::UserInput(e.to_string()))?;

                match selection {
                    0 => {
                        // Start the container
                        docker.start_container(&id).await?;
                        println!("\n{}", style("Container started successfully").green());
                    }
                    1 => {
                        docker.remove_container(&id).await?;
                        println!("\n{}", style("Container removed successfully").green());
                        self.state.remove_container(&id)?;
                    }
                    _ => unreachable!(),
                }
            }
            ContainerStatus::NotFound => {
                // Container not found, proceed with normal flow
            }
        }

        Ok(())
    }

    /// Display success message for container creation
    pub fn display_success(&self, container_id: &str) {
        let config = if let Some(config) = self.config.as_ref() {
            config
        } else {
            return;
        };

        println!(
            "\n{}",
            style("Container started successfully!").green().bold()
        );
        println!("Container ID: {}", style(&container_id[..12]).cyan());
        println!("Mapped port: {}", style(config.host_port).cyan());

        if let Some(path) = &config.data_mount {
            println!("Data directory: {}", style(path.display()).cyan());
        }

        println!("\nFluree will be available at:");
        println!(
            "{}",
            style(format!("http://localhost:{}", config.host_port))
                .cyan()
                .underlined()
        );

        if config.detached {
            println!("\nTo view logs:");
            println!(
                "{}",
                style(format!("docker logs {}", &container_id[..12])).cyan()
            );
        }
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Enable verbose output for detailed processing information
    #[arg(short, long)]
    pub verbose: bool,
}

#[cfg(test)]
mod tests {
    use serial_test::parallel;

    use super::*;

    #[test]
    #[parallel]
    fn test_cli_creation() {
        let _cli = CliState::new();
        // Simply verify we can create a CLI instance
    }

    #[test]
    #[parallel]
    fn test_running_container_action_variants() {
        let variants = RunningContainerAction::variants();
        assert_eq!(variants.len(), 6);
        assert!(variants.contains(&"Stop Container"));
    }

    #[test]
    #[parallel]
    fn test_running_container_action_from_index() {
        assert!(matches!(
            RunningContainerAction::from_index(3),
            Some(RunningContainerAction::Stop)
        ));
        assert!(RunningContainerAction::from_index(10).is_none());
    }

    #[test]
    #[parallel]
    fn test_ledger_action_variants() {
        let variants = LedgerAction::variants();
        assert_eq!(variants.len(), 3);
        assert!(variants.contains(&"See More Details"));
    }

    #[test]
    #[parallel]
    fn test_ledger_action_from_index() {
        assert!(matches!(
            LedgerAction::from_index(0),
            Some(LedgerAction::ViewDetails)
        ));
        assert!(LedgerAction::from_index(10).is_none());
    }
}
