//! Flocker is a CLI tool for managing Fluree Docker containers.
//!
//! This is the main entry point that ties together the CLI interface
//! with Docker operations.

use clap::Parser;
use flocker::{
    cli::Cli,
    docker::{DockerManager, DockerOperations},
    state::{ContainerInfo, DataDirConfig, State},
    ui::{ContainerUI, ImageUI},
};
use tracing::{debug, Level};

#[tokio::main]
async fn main() -> flocker::Result<()> {
    let cli_arg_state = Cli::parse();

    // Initialize logging with appropriate level
    let level = if cli_arg_state.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let level_string = match level {
        Level::INFO => "flocker=info",
        Level::DEBUG => "flocker=debug",
        _ => "flocker=debug",
    };

    tracing_subscriber::fmt()
        // .with_writer(File::create("logs.txt").expect("Failed to create log file"))
        // .with_ansi(false)
        .with_env_filter(level_string)
        // .with_max_level(level)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    debug!("Logging initialized");
    debug!("Initializing Docker manager");

    // Create Docker manager
    let docker = DockerManager::new().await?;

    // Load state
    let state = State::load().unwrap_or_default();
    debug!("State loaded: {:?}", state);

    // Create UI components
    let mut container_ui = ContainerUI::new(state);
    let image_ui = ImageUI;

    debug!("Checking for running container");

    // Main application loop
    loop {
        // Check for existing container if we have one saved
        if let Some(container_id) = container_ui.select_container(&docker).await? {
            debug!("Selected existing container: {}", container_id);

            // Get container status
            let status = docker.get_container_status(&container_id).await?;

            // Handle container actions
            match status {
                flocker::ContainerStatus::Running { .. } => {
                    let action = container_ui.display_action_menu(true)?;
                    match action {
                        0 => {
                            // View Container Stats
                            let stats = docker.get_container_stats(&container_id).await?;
                            println!("\n{}", stats);
                            continue;
                        }
                        1 => {
                            // View Container Logs
                            let logs = docker
                                .get_container_logs(&container_id, Some("100"))
                                .await?;
                            println!("\n{}", logs);
                            continue;
                        }
                        2 => {
                            // List Ledgers
                            let ledgers = docker.list_ledgers(&container_id).await?;
                            if ledgers.is_empty() {
                                println!("\nNo ledgers found");
                            } else {
                                println!("\nLedgers:");
                                for ledger in ledgers {
                                    println!(
                                        "\nAlias: {}\nLast Commit: {}\nCommit Count: {}\nSize: {} bytes",
                                        ledger.alias, ledger.last_commit_time, ledger.commit_count, ledger.size
                                    );
                                }
                            }
                            continue;
                        }
                        3 => {
                            // Stop Container
                            docker.stop_container(&container_id).await?;
                            continue;
                        }
                        4 => {
                            // Stop and Destroy Container
                            docker.remove_container(&container_id).await?;
                            container_ui.remove_container(&container_id)?;
                            continue;
                        }
                        5 => break,    // Exit
                        _ => continue, // Other actions not yet implemented
                    }
                }
                flocker::ContainerStatus::Stopped { .. } => {
                    let action = container_ui.display_action_menu(false)?;
                    match action {
                        0 => {
                            // Start this container
                            docker.start_container(&container_id).await?;
                            continue;
                        }
                        1 => {
                            // Destroy this container
                            docker.remove_container(&container_id).await?;
                            container_ui.remove_container(&container_id)?;
                            continue;
                        }
                        _ => continue,
                    }
                }
                flocker::ContainerStatus::NotFound => (),
            }
        }

        // Create new container
        debug!("Creating new container");

        // Select image
        let image = image_ui.select_image(&docker).await?;

        // Get container configuration
        let name = container_ui.get_container_name()?;
        let port = container_ui.get_port_config(8090)?;
        let data_mount = container_ui
            .get_data_mount_config(&DataDirConfig::from_current_dir(&std::env::current_dir()?))?;
        let detached = container_ui.get_detach_config(true)?;

        let config = flocker::FlureeConfig::new(port, data_mount.clone(), detached);
        config.validate()?;

        // Create and start container
        let container_id = docker
            .create_and_start_container(&image.tag, &config.clone().into_docker_config(), &name)
            .await?;

        // Create container info
        let data_dir = data_mount.as_ref().map(|path| {
            let current_dir = std::env::current_dir().expect("Failed to get current directory");
            let relative_path = if path.starts_with(&current_dir) {
                Some(pathdiff::diff_paths(path, &current_dir).unwrap_or(path.clone()))
            } else {
                None
            };
            DataDirConfig::new(path.clone(), relative_path)
        });

        let container_info = ContainerInfo::new(
            container_id.clone(),
            name,
            port,
            data_dir,
            detached,
            image.tag.name().to_string(),
        );

        // Update state with new container
        container_ui.add_container(container_info)?;

        // Display success message
        container_ui.display_container_success(&container_id, port, data_mount.as_ref());

        // Exit after container creation
        break;
    }

    Ok(())
}
