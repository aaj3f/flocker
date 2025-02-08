//! Docker image tag handling.
//!
//! This module provides functionality for managing and formatting
//! Docker image tags.

use crate::Result;
use pad::PadStr;
use serde::Deserialize;

/// Docker image tag information
#[derive(Deserialize, Debug, Clone)]
pub struct Tag {
    /// Tag name (e.g., "latest", "v1.0.0")
    pub name: String,
    /// Last update timestamp in RFC3339 format
    pub last_updated: String,
}

impl Tag {
    /// Create a new tag
    pub fn new(name: String, last_updated: String) -> Self {
        Tag { name, last_updated }
    }

    /// Format tag for display with optional padding
    pub fn pretty_print(&self, max_tag_length: Option<usize>) -> String {
        let name = if let Some(max_tag_length) = max_tag_length {
            self.name
                .pad_to_width_with_alignment(max_tag_length, pad::Alignment::Left)
        } else {
            self.name.clone()
        };
        format!(
            "fluree/server:{} (updated {})",
            name,
            self.pretty_print_time()
                .unwrap_or_else(|_| "unknown time ago".to_string())
        )
    }

    /// Get the tag name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Format the last update time as a human-readable string
    fn pretty_print_time(&self) -> Result<String> {
        let now_time = chrono::Utc::now();
        let last_updated_time =
            chrono::DateTime::parse_from_rfc3339(&self.last_updated).map_err(|e| {
                crate::error::FlockerError::Docker(format!("Failed to parse date: {}", e))
            })?;
        let duration = now_time.signed_duration_since(last_updated_time);
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
        } else {
            format!("{} days ago", days)
        })
    }
}
