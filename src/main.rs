//! Flocker is a CLI tool for managing Fluree Docker containers.
//!
//! This is the main entry point that ties together the CLI interface
//! with Docker operations.

use clap::Parser;
use flocker::cli::{Cli, CliState};
use flocker::docker::DockerManager;
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

    // Create CLI interface
    let mut cli = CliState::new();

    debug!("Loading state");

    // Load state
    let state = cli.load_state()?;

    debug!("Checking for running container");

    // Check for existing container if we have one saved
    let container_id =
        if let Some(container_id) = cli.try_running_existing_container(&docker).await? {
            container_id.clone()
        } else {
            debug!("No running container found");

            // Get configuration from user
            let (image, config) = cli.get_config(&docker).await?;

            // Create and start container
            docker
                .create_and_start_container(&image.tag, &config.clone().into_docker_config())
                .await?
        };

    // Update state with new container
    cli.set_running_container(Some(container_id.clone()))?;

    // Display success message
    cli.display_success(&container_id);

    Ok(())
}
