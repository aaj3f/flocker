//! Container management UI components.

use console::style;
use std::path::PathBuf;

use crate::docker::DockerOperations;
use crate::state::{ContainerInfo, DataDirConfig, State};
use crate::{ContainerStatus, Result};

use super::UserInterface;

/// Default UI implementation using dialoguer
#[derive(Default)]
pub struct DefaultUI;

/// Container management UI
pub struct ContainerUI<UI = DefaultUI> {
    state: State,
    ui: UI,
}

impl ContainerUI<DefaultUI> {
    /// Create a new ContainerUI instance with default UI implementation
    pub fn new(state: State) -> Self {
        Self {
            state,
            ui: DefaultUI,
        }
    }
}

impl<UI: UserInterface> ContainerUI<UI> {
    /// Create a new ContainerUI instance with custom UI implementation
    pub fn with_ui(state: State, ui: UI) -> Self {
        Self { state, ui }
    }

    /// Remove a container from state
    pub fn remove_container(&mut self, container_id: &str) -> Result<()> {
        let mut state = self.state.clone();
        state.remove_container(container_id)?;
        state.save()?;
        self.state = state;
        Ok(())
    }

    /// Add a new container to state
    pub fn add_container(&self, info: ContainerInfo) -> Result<()> {
        let mut state = self.state.clone();
        state.add_container(info)?;
        state.save()?;
        Ok(())
    }

    /// Get container name from user
    pub fn get_container_name(&self) -> Result<String> {
        self.ui.get_string_input("Enter a name for this container")
    }

    /// Get port configuration from user
    pub fn get_port_config(&self, default_port: u16) -> Result<u16> {
        let port_str = self.ui.get_string_input_with_default(
            "Enter host port to map to container port 8090",
            &default_port.to_string(),
        )?;

        let port = port_str.parse::<u16>().map_err(|_| {
            crate::error::FlockerError::UserInput("Port must be a valid number".to_string())
        })?;

        if port < 1024 {
            return Err(crate::error::FlockerError::UserInput(
                "Port must be >= 1024".to_string(),
            ));
        }

        Ok(port)
    }

    /// Get data mount configuration from user
    pub fn get_data_mount_config(&self, default_path: &DataDirConfig) -> Result<Option<PathBuf>> {
        let use_mount = self
            .ui
            .get_bool_input("Mount a local directory for data persistence?", true)?;

        if !use_mount {
            return Ok(None);
        }

        let path_str = self.ui.get_string_input_with_default(
            "Enter path to mount (will be created if it doesn't exist)",
            &default_path.display_relative_path(),
        )?;

        let current_dir = std::env::current_dir()?;
        let absolute_path = if PathBuf::from(&path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            current_dir.join(&path_str)
        };

        // Create directory if it doesn't exist
        if !absolute_path.exists() {
            std::fs::create_dir_all(&absolute_path)?;
            self.ui
                .display_success(&format!("Created directory: {}", absolute_path.display()));
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
    pub fn get_detach_config(&self, default_detached: bool) -> Result<bool> {
        self.ui.get_bool_input(
            "Run container in background (detached mode)?",
            default_detached,
        )
    }

    /// Format container status for display
    fn format_container_status(
        &self,
        container: &ContainerInfo,
        status: ContainerStatus,
    ) -> String {
        let status_color = match status {
            ContainerStatus::Running { .. } => style("running").green(),
            ContainerStatus::Stopped { .. } => style("stopped").yellow(),
            ContainerStatus::NotFound => style("not found").red(),
        };

        format!(
            "{} [{}] (Image: {}, Port: {}, Last Start: {})",
            style(&container.name).cyan(),
            status_color,
            style(&container.image_tag).yellow(),
            style(&container.port).green(),
            container
                .last_start
                .as_ref()
                .map(|t| t.to_string())
                .unwrap_or_else(|| "Never".to_string())
        )
    }

    /// Display container details
    fn display_container_details(
        &self,
        name: &str,
        id: &str,
        port: u16,
        data_dir: Option<&str>,
        running: bool,
    ) {
        let status = if running { "running" } else { "stopped" };
        let status_style = if running {
            style(status).green()
        } else {
            style(status).yellow()
        };

        println!(
            "\n{} {} ({})",
            style("Found").cyan(),
            style(name).cyan().bold(),
            status_style
        );
        println!("Container ID: {}", style(&id[..id.len().min(12)]).dim());
        println!("Mapped port: {}", style(port).cyan());
        if let Some(dir) = data_dir {
            println!("Data directory: {}", style(dir).cyan());
        }
    }

    /// Handle container selection
    pub async fn select_container(&self, docker: &impl DockerOperations) -> Result<Option<String>> {
        let containers = self.state.get_containers();
        if containers.is_empty() {
            return Ok(None);
        }

        // Get status for all containers
        let mut container_strings = Vec::new();
        for container in &containers {
            let status = docker
                .get_container_status(&container.id)
                .await
                .unwrap_or(ContainerStatus::NotFound);
            container_strings.push(self.format_container_status(container, status));
        }

        // Add option for new container
        let mut options = vec!["Create new container".to_string()];
        options.extend(container_strings);

        let selection = self
            .ui
            .get_selection("Select a container or create a new one", &options)?;

        if selection == 0 {
            return Ok(None);
        }

        let selected_container = containers[selection - 1];
        let status = docker.get_container_status(&selected_container.id).await?;

        match status {
            ContainerStatus::NotFound => {
                self.ui.display_warning(&format!(
                    "Container no longer exists: {}",
                    selected_container.name
                ));
                Ok(None)
            }
            ContainerStatus::Running {
                id,
                name,
                port,
                data_dir,
                started_at,
            } => {
                self.display_container_details(&name, &id, port, data_dir.as_deref(), true);
                if let Some(time) = started_at {
                    println!("Started at: {}", style(time).yellow());
                }
                Ok(Some(id))
            }
            ContainerStatus::Stopped {
                id,
                name,
                last_start,
            } => {
                self.display_container_details(&name, &id, selected_container.port, None, false);
                if let Some(time) = last_start {
                    println!("Last started: {}", style(time).yellow());
                }
                Ok(Some(id))
            }
        }
    }

    /// Display container action menu
    pub fn display_action_menu(&self, running: bool) -> Result<usize> {
        let options = if running {
            vec![
                "View Container Stats",
                "View Container Logs",
                "List Ledgers",
                "Stop Container",
                "Stop and Destroy Container",
                "Exit Flocker",
            ]
        } else {
            vec!["Start this container", "Remove this container"]
        };

        self.ui
            .get_selection("What would you like to do?", &options)
    }

    /// Display success message for container creation
    pub fn display_container_success(
        &self,
        container_id: &str,
        port: u16,
        data_dir: Option<&PathBuf>,
    ) {
        self.ui.display_success("Container started successfully!");
        println!(
            "Container ID: {}",
            style(&container_id[..container_id.len().min(12)]).cyan()
        );
        println!("Mapped port: {}", style(port).cyan());

        if let Some(path) = data_dir {
            println!("Data directory: {}", style(path.display()).cyan());
        }

        println!("\nFluree will be available at:");
        println!(
            "{}",
            style(format!("http://localhost:{}", port))
                .cyan()
                .underlined()
        );
    }
}

impl UserInterface for DefaultUI {
    fn get_string_input(&self, prompt: &str) -> Result<String> {
        dialoguer::Input::new()
            .with_prompt(prompt)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    fn get_string_input_with_default(&self, prompt: &str, default: &str) -> Result<String> {
        dialoguer::Input::new()
            .with_prompt(prompt)
            .default(default.to_string())
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    fn get_bool_input(&self, prompt: &str, default: bool) -> Result<bool> {
        dialoguer::Confirm::new()
            .with_prompt(prompt)
            .default(default)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    fn get_selection<T: ToString>(&self, prompt: &str, items: &[T]) -> Result<usize> {
        dialoguer::Select::new()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::{parallel, serial};
    use tempfile::tempdir;

    // Mock UserInterface implementation for testing
    struct MockUserInterface;

    impl UserInterface for MockUserInterface {
        fn get_string_input(&self, _prompt: &str) -> Result<String> {
            Ok("test".to_string())
        }

        fn get_string_input_with_default(&self, _prompt: &str, default: &str) -> Result<String> {
            Ok(default.to_string())
        }

        fn get_bool_input(&self, _prompt: &str, default: bool) -> Result<bool> {
            Ok(default)
        }

        fn get_selection<T: ToString>(&self, _prompt: &str, _items: &[T]) -> Result<usize> {
            Ok(0)
        }

        fn display_success(&self, _message: &str) {}
        fn display_warning(&self, _message: &str) {}
    }

    // Test helper functions
    fn create_test_container(id: &str, name: &str, port: u16) -> ContainerInfo {
        ContainerInfo::new(
            id.to_string(),
            name.to_string(),
            port,
            None,
            true,
            "latest".to_string(),
        )
    }

    fn create_test_state() -> State {
        let mut state = State::default();
        let container = create_test_container("test1", "test-container", 8090);
        state.containers.insert(container.id.clone(), container);
        state
    }

    #[test]
    #[parallel]
    fn test_format_container_status() {
        let state = create_test_state();
        let ui = ContainerUI::with_ui(state, MockUserInterface);
        let container = create_test_container("test1", "test-container", 8090);

        // Test running status
        let running_status = ContainerStatus::Running {
            id: "test1".to_string(),
            name: "test-container".to_string(),
            port: 8090,
            data_dir: None,
            started_at: Some("2024-01-01T00:00:00Z".to_string()),
        };
        let status_str = ui.format_container_status(&container, running_status);
        assert!(status_str.contains("running"));
        assert!(status_str.contains("test-container"));
        assert!(status_str.contains("8090"));

        // Test stopped status
        let stopped_status = ContainerStatus::Stopped {
            id: "test1".to_string(),
            name: "test-container".to_string(),
            last_start: None,
        };
        let status_str = ui.format_container_status(&container, stopped_status);
        assert!(status_str.contains("stopped"));
        assert!(status_str.contains("Never"));
    }

    #[test]
    #[parallel]
    #[serial]
    fn test_add_container() {
        let temp_dir = tempdir().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let state = State::default();
        let ui = ContainerUI::with_ui(state, MockUserInterface);
        let container = create_test_container("test1", "test-container", 8090);

        assert!(ui.add_container(container).is_ok());
    }
}
