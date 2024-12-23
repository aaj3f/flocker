//! Flocker is a CLI tool for managing Fluree Docker containers.
//!
//! This library provides functionality to:
//! - List and select Fluree Docker images
//! - Configure and run Fluree containers
//! - Monitor container status and statistics
//! - Manage container lifecycle

pub mod cli;
pub mod config;
pub mod docker;
pub mod error;
pub mod state;
pub mod ui;

/// Re-export commonly used types
pub use cli::Cli;
pub use config::FlureeConfig;
pub use docker::{
    manager::{DockerManager, DockerOperations},
    types::{ContainerConfig, FlureeImage, LedgerInfo},
};
pub use error::FlockerError;
pub use state::State;
pub type Result<T> = std::result::Result<T, FlockerError>;

/// Container status information
#[derive(Debug, Clone)]
pub enum ContainerStatus {
    /// Container is running
    Running {
        /// Container ID
        id: String,
        /// Container name
        name: String,
        /// Mapped port
        port: u16,
        /// Data directory if mounted
        data_dir: Option<String>,
        /// Last start time
        started_at: Option<String>,
    },
    /// Container exists but is not running
    Stopped {
        /// Container ID
        id: String,
        /// Container name
        name: String,
        /// Last start time before stopping
        last_start: Option<String>,
    },
    /// No container found
    NotFound,
}
