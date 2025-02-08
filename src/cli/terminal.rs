//! Terminal utilities for CLI formatting and interaction.
//!
//! This module provides functionality for:
//! - Getting terminal dimensions
//! - Formatting tables with proper alignment
//! - Truncating strings to fit terminal width

use std::io::Write;
use termion::terminal_size;

/// Get the current terminal width, defaulting to 80 if it can't be determined
pub fn get_terminal_width() -> u16 {
    terminal_size().map(|(w, _)| w).unwrap_or(80)
}

/// A table column definition
pub struct Column {
    /// Header text for the column
    header: String,
    /// Width of the column in characters
    width: usize,
}

impl Column {
    /// Create a new column with header and width
    pub fn new(header: impl Into<String>, width: usize) -> Self {
        Self {
            header: header.into(),
            width,
        }
    }
}

/// A table formatter for aligned columnar output
pub struct TableFormatter {
    columns: Vec<Column>,
}

impl TableFormatter {
    /// Create a new table formatter with the given columns
    pub fn new(columns: Vec<Column>) -> Self {
        Self { columns }
    }

    /// Print the table header
    pub fn print_header(&self) {
        for col in &self.columns {
            print!("{:width$} ", col.header, width = col.width);
        }
        println!();
    }

    /// Print a row of data
    pub fn print_row(&self, values: &[String]) {
        for (i, value) in values.iter().enumerate() {
            if let Some(col) = self.columns.get(i) {
                let truncated = if value.len() > col.width {
                    format!("{}...", &value[..col.width.saturating_sub(1)])
                } else {
                    value.clone()
                };
                print!("{:width$} ", truncated, width = col.width);
            }
        }
        println!();
        std::io::stdout().flush().unwrap();
    }
}

/// Format a byte size into a human readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Format a duration since now into a human readable string
pub fn format_duration_since(timestamp: &str) -> Result<String, chrono::ParseError> {
    let now = chrono::Utc::now();
    let then = chrono::DateTime::parse_from_rfc3339(timestamp)?;
    tracing::debug!("Timestamp: {}", then);
    let duration = now.signed_duration_since(then);

    let days = duration.num_days();
    let weeks = days / 7;
    let months = days / 30;
    let years = days / 365;

    Ok(if years > 0 {
        format!("{} years ago", years)
    } else if months > 0 {
        format!("{} months ago", months)
    } else if weeks > 0 {
        format!("{} weeks ago", weeks)
    } else if days > 0 {
        format!("{} days ago", days)
    } else {
        let hours = duration.num_hours();
        if hours > 0 {
            format!("{} hours ago", hours)
        } else {
            let minutes = duration.num_minutes();
            if minutes > 0 {
                format!("{} minutes ago", minutes)
            } else {
                "Seconds Ago".to_string()
            }
        }
    })
}
