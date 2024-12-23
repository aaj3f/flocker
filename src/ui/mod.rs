//! User interface components and interactions.
//!
//! This module provides the interactive UI components for:
//! - Container management
//! - Image selection
//! - Configuration
//! - Ledger management

mod container;
mod image;
mod ledger;

pub use container::ContainerUI;
pub use image::ImageUI;
pub use ledger::LedgerUI;

use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

/// Common UI functionality shared across components
pub trait UserInterface {
    /// Get a string input from the user
    fn get_string_input(&self, prompt: &str) -> crate::Result<String> {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    /// Get a string input from the user with a default value
    fn get_string_input_with_default(&self, prompt: &str, default: &str) -> crate::Result<String> {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default.into())
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    /// Get a boolean input from the user
    fn get_bool_input(&self, prompt: &str, default: bool) -> crate::Result<bool> {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    /// Get a selection from a list of options
    fn get_selection<T: ToString>(&self, prompt: &str, items: &[T]) -> crate::Result<usize> {
        Select::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .items(items)
            .default(0)
            .interact()
            .map_err(|e| crate::error::FlockerError::UserInput(e.to_string()))
    }

    /// Display a success message
    fn display_success(&self, message: &str) {
        println!("\n{}", style(message).green().bold());
    }

    /// Display a warning message
    fn display_warning(&self, message: &str) {
        println!("\n{}", style(message).yellow().bold());
    }

    /// Display an error message
    fn display_error(&self, message: &str) {
        println!("\n{}", style(message).red().bold());
    }

    /// Display an info message
    fn display_info(&self, message: &str) {
        println!("\n{}", style(message).cyan());
    }
}
