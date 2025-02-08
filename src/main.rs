//! Flocker is a CLI tool for managing Fluree Docker containers.
//!
//! This is the main entry point that ties together the CLI interface
//! with Docker operations.

use clap::Parser;
use flocker::{
    cli::{Cli, CliState},
    docker::{DockerManager, DockerOperations},
};
use tracing::debug;

#[tokio::main]
async fn main() -> flocker::Result<()> {
    let cli_arg_state = Cli::parse();

    // Initialize logging with appropriate level
    let env_filter = if cli_arg_state.verbose {
        "flocker=debug"
    } else {
        "flocker=info"
    };

    // Allow RUST_LOG to override our default
    let env_filter = std::env::var("RUST_LOG").unwrap_or(env_filter.to_string());

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(true)
        .init();

    debug!("Logging initialized");
    debug!("Initializing Docker manager");

    // Create Docker manager
    let docker = DockerManager::new().await?;

    // Create CLI state
    let mut cli = CliState::new();
    debug!("CLI state initialized");

    // Main application loop
    loop {
        // Try to handle existing container or create new one
        if let Some(container_id) = cli.try_running_existing_container(&docker).await? {
            debug!("Handled existing container: {}", container_id);
            continue;
        }

        // Create new container
        debug!("Creating new container");
        let (image, config, name) = cli.get_config(&docker).await?;
        let container = docker
            .create_and_start_container(&image.tag, &config.clone().into_docker_config(), &name)
            .await?;

        // Add container to state and display success
        cli.add_container(container.clone())?;
        cli.display_success(&container);
    }
}
