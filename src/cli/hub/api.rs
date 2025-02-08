//! Docker Hub API client.
//!
//! This module provides functionality for interacting with
//! the Docker Hub API to fetch image tags and metadata.

use reqwest::Client;
use serde::Deserialize;

use super::Tag;
use crate::{FlockerError, Result};

/// Response from Docker Hub API tag listing endpoint
#[derive(Deserialize)]
pub struct TagResponse {
    /// List of tags returned
    pub results: Vec<Tag>,
    /// URL for next page of results, if any
    pub next: Option<String>,
}

/// Docker Hub API client
pub struct HubClient {
    client: Client,
}

impl HubClient {
    /// Create a new Docker Hub API client
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Fetch all tags for the Fluree server image
    pub async fn fetch_tags(&self) -> Result<Vec<Tag>> {
        let mut url = "https://hub.docker.com/v2/repositories/fluree/server/tags".to_string();
        let mut tags = Vec::new();

        loop {
            let response = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| FlockerError::Docker(format!("Failed to fetch tags: {}", e)))
                .and_then(|res| {
                    if res.status().is_success() {
                        Ok(res)
                    } else {
                        Err(FlockerError::Docker(format!(
                            "Failed to fetch tags: {}",
                            res.status()
                        )))
                    }
                })?;

            let response: TagResponse = response.json().await.map_err(|e| {
                FlockerError::Docker(format!("Failed to parse tags response: {}", e))
            })?;

            tags.extend(response.results.into_iter());

            if let Some(next_url) = response.next {
                url = next_url;
            } else {
                break;
            }
        }

        Ok(tags)
    }
}

impl Default for HubClient {
    fn default() -> Self {
        Self::new()
    }
}
