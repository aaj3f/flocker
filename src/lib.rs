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

// Re-export commonly used types
pub use cli::{
    ui::{DefaultUI, UserInterface},
    Cli, CliState,
};
pub use config::FlureeConfig;
use console::{style, StyledObject};
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

type TruncateFunctionType = Box<dyn Fn(&str) -> String>;

impl ContainerStatus {
    pub fn style(&self, truncate_fn: Option<TruncateFunctionType>) -> String {
        let truncate_fn = truncate_fn.unwrap_or_else(|| Box::new(|s: &str| s.to_string()));
        // match self {
        //     ContainerStatus::Running { .. } => style(truncate_fn("running")).green(),
        //     ContainerStatus::Stopped { .. } => style(truncate_fn("stopped")).yellow(),
        //     ContainerStatus::NotFound => style(truncate_fn("not found")).red(),
        // }
        match self {
            ContainerStatus::Running { .. } => truncate_fn("running"),
            ContainerStatus::Stopped { .. } => truncate_fn("stopped"),
            ContainerStatus::NotFound => truncate_fn("not found"),
        }
    }
}
