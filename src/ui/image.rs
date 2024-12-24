//! Image selection and management UI components.

use console::style;
use pad::PadStr;
use reqwest::Client;
use serde::Deserialize;

use crate::docker::{DockerManager, DockerOperations, FlureeImage};
use crate::Result;

use super::UserInterface;

#[derive(Deserialize, Debug, Clone)]
struct Tag {
    name: String,
    last_updated: String,
}

#[derive(Deserialize)]
struct TagResponse {
    results: Vec<Tag>,
    next: Option<String>,
}

impl Tag {
    fn pretty_print(&self, max_tag_length: Option<usize>) -> String {
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

    fn name(&self) -> &str {
        &self.name
    }

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

/// Image selection UI
#[derive(Default)]
pub struct ImageUI;

impl ImageUI {
    /// Select a Fluree image
    pub async fn select_image(&self, docker: &DockerManager) -> Result<FlureeImage> {
        let options = ["Remote (Docker Hub)", "Local"];
        let selection = self.get_selection(
            "Do you want to list remote or local Fluree images?",
            &options,
        )?;

        match selection {
            0 => self.select_remote_image(docker).await,
            1 => self.select_local_image(docker).await,
            _ => unreachable!(),
        }
    }

    /// Select a remote image from Docker Hub
    async fn select_remote_image(&self, docker: &DockerManager) -> Result<FlureeImage> {
        self.display_info("Fetching available images from Docker Hub...");

        let tags = self.fetch_remote_tags().await?;
        let max_tag_length = tags
            .iter()
            .map(|tag| tag.name.len())
            .max()
            .unwrap_or_default();

        let tag_strings: Vec<String> = tags
            .iter()
            .map(|tag| tag.pretty_print(Some(max_tag_length)))
            .collect();

        let selection = self.get_selection("Select a Fluree image", &tag_strings)?;
        let selected_tag = &tags[selection];

        self.pull_remote_image(docker, selected_tag.name()).await?;
        docker.get_image_by_tag(selected_tag.name()).await
    }

    /// Select a local image
    async fn select_local_image(&self, docker: &DockerManager) -> Result<FlureeImage> {
        let images = docker.list_local_images().await?;

        if images.is_empty() {
            self.display_warning("No local Fluree images found.");
            println!("Please pull an image first using:");
            println!("{}", style("docker pull fluree/server:latest").cyan());
            std::process::exit(1);
        }

        let max_tag_length = images
            .iter()
            .map(|img| img.tag.name().len())
            .max()
            .unwrap_or_default();

        let image_strings: Vec<String> = images
            .iter()
            .map(|img| img.tag.pretty_print(Some(max_tag_length)))
            .collect();

        let selection = self.get_selection("Select a Fluree image", &image_strings)?;
        Ok(images[selection].clone())
    }

    /// Fetch available tags from Docker Hub
    async fn fetch_remote_tags(&self) -> Result<Vec<Tag>> {
        let client = Client::new();
        let mut url = "https://hub.docker.com/v2/repositories/fluree/server/tags".to_string();
        let mut tags = Vec::new();

        loop {
            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| {
                    crate::error::FlockerError::Docker(format!("Failed to fetch tags: {}", e))
                })
                .and_then(|res| {
                    if res.status().is_success() {
                        Ok(res)
                    } else {
                        Err(crate::error::FlockerError::Docker(format!(
                            "Failed to fetch tags: {}",
                            res.status()
                        )))
                    }
                })?;

            let response: TagResponse = response.json().await.map_err(|e| {
                crate::error::FlockerError::Docker(format!("Failed to parse tags response: {}", e))
            })?;

            tags.extend(response.results);

            if let Some(next_url) = response.next {
                url = next_url;
            } else {
                break;
            }
        }

        Ok(tags)
    }

    /// Pull a remote image
    async fn pull_remote_image(&self, docker: &DockerManager, tag: &str) -> Result<()> {
        self.display_info(&format!("Pulling image fluree/server:{}", tag));
        docker.pull_image(tag).await?;
        self.display_success(&format!("Successfully pulled fluree/server:{}", tag));
        Ok(())
    }
}

impl UserInterface for ImageUI {
    fn display_info(&self, message: &str) {
        println!("\n{}", style(message).cyan());
    }
}
