//! CLI user interface state and interactions.
//!
//! This module provides the main CLI interface state and user interaction
//! handling, including prompts, configuration, and display formatting.

use console::style;
use crossterm::{
    cursor, execute,
    terminal::{Clear, ClearType},
};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

/// Common UI functionality shared across components
pub trait UserInterface {
    /// Get a string input from the user
    fn get_string_input(&self, prompt: &str) -> crate::Result<String>;

    /// Get a string input from the user with a default value
    fn get_string_input_with_default(&self, prompt: &str, default: &str) -> crate::Result<String>;

    /// Get a boolean input from the user
    fn get_bool_input(&self, prompt: &str, default: bool) -> crate::Result<bool>;

    /// Get a selection from a list of options
    fn get_selection<T: ToString>(&self, prompt: &str, items: &[T]) -> crate::Result<usize>;

    /// Display a success message
    fn display_success(&self, message: &str);

    /// Display a warning message
    fn display_warning(&self, message: &str);
}

/// Default UI implementation using dialoguer
#[derive(Default)]
pub struct DefaultUI;

impl UserInterface for DefaultUI {
    fn get_string_input(&self, prompt: &str) -> crate::Result<String> {
        Input::new()
            .with_prompt(prompt)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    fn get_string_input_with_default(&self, prompt: &str, default: &str) -> crate::Result<String> {
        Input::new()
            .with_prompt(prompt)
            .default(default.to_string())
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    fn get_bool_input(&self, prompt: &str, default: bool) -> crate::Result<bool> {
        Confirm::new()
            .with_prompt(prompt)
            .default(default)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    fn get_selection<T: ToString>(&self, prompt: &str, items: &[T]) -> crate::Result<usize> {
        Select::new()
            .with_prompt(prompt)
            .items(items)
            .default(0)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    fn display_success(&self, message: &str) {
        println!("\n{}", style(message).green().bold());
    }

    fn display_warning(&self, message: &str) {
        println!("\n{}", style(message).yellow().bold());
    }
}
use std::{io, path::PathBuf};
use tracing::debug;

use crate::{
    cli::{format_bytes, format_duration_since},
    config::FlureeConfig,
    docker::{DockerOperations, FlureeImage},
    state::{ContainerInfo, DataDirConfig, State},
    ContainerStatus, FlockerError, Result,
};

use super::{
    actions::{LedgerAction, RunningContainerAction},
    hub::HubClient,
};

/// Custom theme for container list formatting
struct ContainerTheme {
    base: ColorfulTheme,
}

impl ContainerTheme {
    fn new() -> Self {
        Self {
            base: ColorfulTheme::default(),
        }
    }
}

impl dialoguer::theme::Theme for ContainerTheme {
    fn format_prompt(&self, f: &mut dyn std::fmt::Write, prompt: &str) -> std::fmt::Result {
        self.base.format_prompt(f, prompt)
    }

    fn format_error(&self, f: &mut dyn std::fmt::Write, err: &str) -> std::fmt::Result {
        self.base.format_error(f, err)
    }

    fn format_confirm_prompt(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        default: Option<bool>,
    ) -> std::fmt::Result {
        self.base.format_confirm_prompt(f, prompt, default)
    }

    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        selection: Option<bool>,
    ) -> std::fmt::Result {
        self.base
            .format_confirm_prompt_selection(f, prompt, selection)
    }

    fn format_select_prompt(&self, f: &mut dyn std::fmt::Write, prompt: &str) -> std::fmt::Result {
        self.base.format_select_prompt(f, prompt)
    }

    fn format_select_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        sel: &str,
    ) -> std::fmt::Result {
        self.base.format_select_prompt_selection(f, prompt, sel)
    }

    fn format_select_prompt_item(
        &self,
        f: &mut dyn std::fmt::Write,
        text: &str,
        active: bool,
    ) -> std::fmt::Result {
        if text == "Create new container" {
            if active {
                write!(f, "{}", style(text).cyan().bold())
            } else {
                write!(f, "{}", text)
            }
        } else {
            // For container list items, add a prefix indicator
            if active {
                write!(f, "{} {}", style("❯").cyan().bold(), text)
            } else {
                write!(f, "  {}", text)
            }
        }
    }
}

/// CLI manager for handling user interaction
pub struct CliState {
    theme: ContainerTheme,
    state: State,
    config: Option<FlureeConfig>,
    hub_client: HubClient,
}

impl Default for CliState {
    fn default() -> Self {
        Self {
            theme: ContainerTheme::new(),
            state: State::load().unwrap_or_default(),
            config: None,
            hub_client: HubClient::new(),
        }
    }
}

impl CliState {
    /// Create a new CLI instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new container to state
    pub fn add_container(&mut self, info: ContainerInfo) -> Result<()> {
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

    pub fn get_state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    /// Get container name from user
    pub fn get_container_name(&self) -> Result<String> {
        Input::with_theme(&self.theme)
            .with_prompt("Enter a name for this container")
            .interact()
            .map_err(|e| FlockerError::UserInput(e.to_string()))
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
            .map_err(|e| FlockerError::UserInput(e.to_string()))
    }

    /// Get data mount configuration from user
    pub fn get_data_mount_config(&mut self) -> Result<Option<PathBuf>> {
        let use_mount = Confirm::with_theme(&self.theme)
            .with_prompt("Mount a local directory for data persistence?")
            .default(true)
            .interact()
            .map_err(|e| FlockerError::UserInput(e.to_string()))?;

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
            .map_err(|e| FlockerError::UserInput(e.to_string()))?;

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
                .map_err(|e| FlockerError::ConfigFile {
                    message: "Failed to resolve path".to_string(),
                    path: absolute_path.clone(),
                    source: e.into(),
                })?;

        Ok(Some(canonical_path))
    }

    /// Get complete configuration from user
    pub async fn get_config(
        &mut self,
        docker: &impl DockerOperations,
    ) -> Result<(FlureeImage, FlureeConfig, String)> {
        let image = self.select_image(docker).await?;
        let name = self.get_container_name()?;
        let host_port = self.get_port_config()?;
        let data_mount = self.get_data_mount_config()?;

        let config = FlureeConfig::new(host_port, data_mount.clone());
        config.validate()?;

        self.config = Some(config.clone());

        Ok((image, config, name))
    }

    /// Display available Fluree images and get user selection
    pub async fn select_image(&self, docker: &impl DockerOperations) -> Result<FlureeImage> {
        let remote_or_local_selection = Select::with_theme(&self.theme)
            .with_prompt("Do you want to list remote or local Fluree images?")
            .items(&["Remote (Docker Hub)", "Local"])
            .default(0)
            .interact()
            .map_err(|e| FlockerError::UserInput(e.to_string()))?;

        match remote_or_local_selection {
            0 => self.select_remote_image(docker).await,
            1 => self.select_local_image(docker).await,
            _ => unreachable!(),
        }
    }

    /// Select a remote image from Docker Hub
    async fn select_remote_image(&self, docker: &impl DockerOperations) -> Result<FlureeImage> {
        println!(
            "{}",
            style("Fetching available images from Docker Hub...").cyan()
        );

        let tags = self.hub_client.fetch_tags().await?;

        // Find the longest tag name for alignment
        let max_tag_length = tags
            .iter()
            .map(|tag| tag.name.len())
            .max()
            .unwrap_or_default();

        let tag_strings_to_display = tags
            .iter()
            .map(|tag| tag.pretty_print(Some(max_tag_length)))
            .collect::<Vec<String>>();

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Select a Fluree image")
            .items(tag_strings_to_display.as_slice())
            .default(0)
            .interact()
            .map_err(|e| FlockerError::UserInput(e.to_string()))?;

        let selected_tag = &tags[selection].name;

        self.pull_remote_image(docker, selected_tag).await?;

        docker.get_image_by_tag(selected_tag).await
    }

    /// Pull a remote image from Docker Hub
    async fn pull_remote_image(&self, docker: &impl DockerOperations, tag: &str) -> Result<()> {
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

    /// Select a local image
    pub async fn select_local_image(&self, docker: &impl DockerOperations) -> Result<FlureeImage> {
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
            .map_err(|e| FlockerError::UserInput(e.to_string()))?;

        Ok(images[selection].clone())
    }

    /// Try to run an existing container if one is saved in the state
    pub async fn try_running_existing_container(
        &mut self,
        docker: &impl DockerOperations,
    ) -> Result<Option<String>> {
        let containers = self.state.get_containers().to_vec(); // Clone to avoid borrow issues
        if containers.is_empty() {
            return Ok(None);
        }

        use crate::cli::terminal::get_terminal_width;

        // Get terminal width and calculate column widths
        let term_width = get_terminal_width() as usize;
        let max_name_width = (term_width * 10) / 100; // 15% of width
        let max_status_width = (term_width * 15) / 100; // 10% of width
        let max_image_width = (term_width * 30) / 100; // 30% of width
        let max_port_width = (term_width * 8) / 100; // 8% of width
        let max_time_width = (term_width * 30) / 100; // 30% of width

        let mut name_width = 0;
        let mut status_width = 0;
        let mut image_width = 0;
        let mut port_width = 0;
        let mut time_width = 0;

        // Helper function to truncate strings
        fn truncate(s: &str, width: usize) -> String {
            if s.len() > width {
                format!("{}...", &s[..width.saturating_sub(3)])
            } else {
                format!("{:<width$}", s)
            }
        }

        // Create container info strings
        let mut raw_items = vec![];
        let mut items = vec![];
        for c in &containers {
            let status = docker
                .get_container_status(&c.id)
                .await
                .unwrap_or(ContainerStatus::NotFound);

            let last_start = c
                .last_start
                .as_ref()
                .map(|t| match format_duration_since(t) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::debug!("Failed to parse time: {}", t);
                        tracing::debug!("Error: {}", e);
                        "Unknown".to_string()
                    }
                })
                .unwrap_or_else(|| "Never".to_string());

            let status_str = match status {
                ContainerStatus::Running { .. } => "running",
                ContainerStatus::Stopped { .. } => "stopped",
                ContainerStatus::NotFound => "not found",
            };

            // let item = format!(
            //     "{:<name_width$} {:<status_width$} {:<image_width$} {:<port_width$} {:<time_width$}",
            //     truncate(&c.name, name_width),
            //     truncate(format!("STATE: {}", status_str).as_str(), status_width),
            //     truncate(&c.image_tag, image_width),
            //     truncate(format!("PORT: {}", c.port).as_str(), port_width),
            //     truncate(format!("LAST STARTED: {}", last_start).as_str(), time_width),
            //     name_width = name_width,
            //     status_width = status_width,
            //     image_width = image_width,
            //     port_width = port_width,
            //     time_width = time_width
            // );

            if c.name.len() > name_width {
                if c.name.len() > max_name_width {
                    name_width = max_name_width;
                } else {
                    name_width = c.name.len();
                }
            }

            if status_str.len() > status_width {
                if status_str.len() > max_status_width {
                    status_width = max_status_width;
                } else {
                    status_width = status_str.len();
                }
            }

            if c.image_tag.len() > image_width {
                if c.image_tag.len() > max_image_width {
                    image_width = max_image_width;
                } else {
                    image_width = c.image_tag.len();
                }
            }

            if c.port.to_string().len() > port_width {
                if c.port.to_string().len() > max_port_width {
                    port_width = max_port_width;
                } else {
                    port_width = c.port.to_string().len();
                }
            }

            if last_start.len() > time_width {
                if last_start.len() > max_time_width {
                    time_width = max_time_width;
                } else {
                    time_width = last_start.len();
                }
            }

            raw_items.push((&c.name, status_str, &c.image_tag, c.port, last_start));
        }

        for (name, status, image, port, time) in raw_items {
            let item = format!(
                "{:<name_width$} {:<status_width$} {:<image_width$} {:<port_width$} {:<time_width$}",
                style(truncate(name, name_width)).blue().bold(),
                // truncate(format!("STATE: {}", status).as_str(), status_width),
                match status {
                    "running" => style("running").green(),
                    "stopped" => style("stopped").yellow(),
                    "not found" => style("not found").red(),
                    _ => style(status).cyan(),
                },
                truncate(image, image_width),
                // truncate(format!("PORT: {}", port).as_str(), port_width),
                style(truncate(&port.to_string(), port_width)).green(),
                // truncate(format!("LAST STARTED: {}", time).as_str(), time_width),
                truncate(&time.to_string(), time_width),
                name_width = name_width,
                status_width = status_width,
                image_width = image_width,
                port_width = port_width,
                time_width = time_width
            );

            items.push(item);
        }

        items.push("Create new container".to_string());
        println!(
            "{} Select a container or create a new one:",
            style("?").yellow(),
        );
        println!(
            "  {:<name_width$} {:<status_width$} {:<image_width$} {:<port_width$} {:<time_width$}",
            style("Container").bold(),
            style("Status").bold(),
            style("Image").bold(),
            style("Port").bold(),
            style("Last Started").bold()
        );
        let selection = Select::with_theme(&self.theme)
            // .with_prompt("Select a container or create a new one")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| FlockerError::UserInput(e.to_string()))?;

        let stdout = io::stdout();
        let mut handle = stdout.lock();

        for _ in 0..2 {
            execute!(handle, cursor::MoveUp(1), Clear(ClearType::CurrentLine)).unwrap();
        }

        if selection == items.len() - 1 {
            return Ok(None);
        }

        let selected_container = containers[selection].clone(); // Clone to avoid borrow issues
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

    /// Handle running container actions
    pub async fn handle_running_container(
        &mut self,
        docker: &impl DockerOperations,
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
                    .map_err(|e| FlockerError::UserInput(e.to_string()))?;

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
                        let stats = docker.get_container_stats(&id).await?;
                        println!("\n{}", stats);
                    }
                    Some(RunningContainerAction::ViewLogs) => {
                        // Get the last 1000 lines of logs
                        let logs = docker.get_container_logs(&id, Some("1000")).await?;
                        if let Ok(mut pager) = super::pager::Pager::new(&logs) {
                            pager.display()?;
                        }
                    }
                    Some(RunningContainerAction::ListLedgers) => {
                        self.handle_ledger_management(docker, &id).await?;
                    }
                    Some(RunningContainerAction::GoBack) => {
                        return Ok(());
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
                        let now_time = chrono::Utc::now();
                        let now_time_string = now_time.to_rfc3339();
                        self.state
                            .update_container_start_time(&id, now_time_string)?;
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

    /// Handle ledger management for a container
    async fn handle_ledger_management(
        &self,
        docker: &impl DockerOperations,
        container_id: &str,
    ) -> Result<()> {
        loop {
            // Get list of ledgers
            let mut ledgers = docker.list_ledgers(container_id).await?;

            if ledgers.is_empty() {
                println!("\n{}", style("No ledgers found").yellow());
                return Ok(());
            }

            ledgers.sort_by(|a, b| b.last_commit_time.cmp(&a.last_commit_time));

            let raw_values: Vec<(String, String, String, Option<String>, String, String)> = ledgers
                .iter()
                .map(|ledger| {
                    let duration = format_duration_since(&ledger.last_commit_time)
                        .unwrap_or_else(|_| "unknown time ago".to_string());
                    let size = format_bytes(ledger.size);
                    let commit_count = ledger.commit_count.to_string();
                    let last_index = ledger.last_index.map(|i| i.to_string());
                    let flakes_count = ledger.flakes_count.to_string();
                    let alias = ledger.alias.clone();

                    (
                        alias,
                        duration,
                        commit_count,
                        last_index,
                        size,
                        flakes_count,
                    )
                })
                .collect();

            // Step 2: Determine the max width for each field
            let max_widths = raw_values.iter().fold(
                (0, 0, 0, 0, 0, 0),
                |(max_alias, max_duration, max_commits, max_index, max_size, max_flakes),
                 (alias, duration, commits, index, size, flakes)| {
                    (
                        max_alias.max(alias.len()),
                        max_duration.max(duration.len()),
                        max_commits.max(commits.len()),
                        max_index.max(index.as_ref().unwrap_or(&"None".to_string()).len()),
                        max_size.max(size.len()),
                        max_flakes.max(flakes.len()),
                    )
                },
            );

            let (alias_w, duration_w, commits_w, index_w, size_w, flakes_w) = max_widths;

            // Format ledger information for display
            let mut ledger_strings: Vec<String> = raw_values
                .into_iter()
                .map(|(alias, duration, commit_count, last_index, size, flakes_count)| {
                    format!(
                        "{:<alias_w$} Last commit: {:<duration_w$}  Commits: {:<commits_w$}  Last Indexed Commit: {:<index_w$}  Size: {:<size_w$}  Flakes: {:<flakes_w$}",
                        style(alias).cyan(),
                        style(duration).yellow(),
                        style(commit_count.clone()).green(),
                        match last_index {
                            Some(i) => if i == commit_count {
                                style(i).green()
                            } else {
                                style(i).yellow()
                            },
                            None => style("None".to_string()).red(),
                        },
                        style(size).blue(),
                        style(flakes_count).blue(),
                        alias_w = alias_w,
                        duration_w = duration_w,
                        commits_w = commits_w,
                        index_w = index_w,
                        size_w = size_w,
                        flakes_w = flakes_w,
                    )
                })
                .collect();

            ledger_strings.push("Go Back to Container Menu".to_string());

            // Let user select a ledger
            let selection = Select::with_theme(&self.theme)
                .with_prompt("Select a ledger")
                .items(&ledger_strings)
                .default(0)
                .interact()
                .map_err(|e| FlockerError::UserInput(e.to_string()))?;

            let selected_ledger = if selection < ledgers.len() {
                &ledgers[selection]
            } else {
                break;
            };

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
                Some(LedgerAction::GoBack) => {
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Display success message for container creation
    pub fn display_success(&self, container: &ContainerInfo) {
        println!(
            "\n{}",
            style("Container started successfully!").green().bold()
        );
        println!("Container ID: {}", style(&container.id[..12]).cyan());
        println!("Mapped port: {}", style(container.port).cyan());

        if let Some(data_dir) = &container.data_dir {
            println!(
                "Data directory: {}",
                style(data_dir.absolute_path.display()).cyan()
            );
        }

        println!("\nFluree will be available at:");
        println!(
            "{}",
            style(format!("http://localhost:{}", container.port))
                .cyan()
                .underlined()
        );

        if container.detached {
            println!("\nTo view logs:");
            println!(
                "{}",
                style(format!("docker logs {}", &container.id[..12])).cyan()
            );
        }
    }
}
