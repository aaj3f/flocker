//! Docker Hub interactions.
//!
//! This module provides functionality for:
//! - Fetching and managing Docker image tags
//! - Interacting with Docker Hub API
//! - Formatting tag information

mod api;
mod tag;

pub use api::{HubClient, TagResponse};
pub use tag::Tag;
