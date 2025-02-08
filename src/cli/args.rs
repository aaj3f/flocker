//! Command line argument parsing for Flocker.
//!
//! This module handles parsing and validation of command line arguments
//! using the clap crate.

use clap::Parser;

/// Command line arguments for Flocker
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Enable verbose output for detailed processing information
    #[arg(short, long)]
    pub verbose: bool,
}
