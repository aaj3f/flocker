//! CLI interface for Flocker.
//!
//! This module provides the command-line interface components,
//! organized into submodules for different concerns:
//! - args: Command line argument parsing
//! - actions: Container and ledger action handling
//! - hub: Docker Hub interactions
//! - ui: User interface state and interactions

pub mod actions;
pub mod args;
pub mod hub;
pub mod pager;
pub mod terminal;
pub mod ui;

pub use terminal::{format_bytes, format_duration_since, Column, TableFormatter};

// Re-export commonly used types
pub use args::Cli;
pub use ui::CliState;
