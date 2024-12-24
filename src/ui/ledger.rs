//! Ledger management UI components.

use console::style;

use crate::docker::{DockerManager, DockerOperations, LedgerInfo};
use crate::Result;

use super::UserInterface;

/// Available actions when viewing a ledger
#[derive(Debug)]
enum LedgerAction {
    ViewDetails,
    Delete,
    Return,
}

impl LedgerAction {
    fn variants() -> Vec<&'static str> {
        vec!["See More Details", "Delete Ledger", "Return to Ledger List"]
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::ViewDetails),
            1 => Some(Self::Delete),
            2 => Some(Self::Return),
            _ => None,
        }
    }
}

/// Ledger management UI
#[derive(Default)]
pub struct LedgerUI;

impl LedgerUI {
    /// Format ledger information for display
    fn format_ledger_info(&self, ledger: &LedgerInfo) -> String {
        format!(
            "{} (Last commit: {}, Commits: {}, Size: {} bytes)",
            style(&ledger.alias).cyan(),
            style(&ledger.last_commit_time).yellow(),
            style(&ledger.commit_count).green(),
            style(&ledger.size).blue()
        )
    }

    /// Handle ledger deletion confirmation
    async fn handle_ledger_deletion(
        &self,
        docker: &DockerManager,
        container_id: &str,
        ledger: &LedgerInfo,
    ) -> Result<bool> {
        self.display_warning("This will permanently delete the ledger and all its data!");

        let confirmation = self.get_string_input("Type 'delete' to confirm")?;
        if confirmation == "delete" {
            docker.delete_ledger(container_id, &ledger.path).await?;
            self.display_success("Ledger deleted successfully");
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Handle ledger details display
    async fn handle_ledger_details(
        &self,
        docker: &DockerManager,
        container_id: &str,
        ledger: &LedgerInfo,
    ) -> Result<()> {
        let details = docker
            .get_ledger_details(container_id, &ledger.path)
            .await?;
        println!("\n{}", style("Ledger Details:").cyan().bold());
        println!("{}", details);
        Ok(())
    }

    /// Handle ledger management for a container
    pub async fn manage_ledgers(&self, docker: &DockerManager, container_id: &str) -> Result<()> {
        loop {
            let ledgers = docker.list_ledgers(container_id).await?;

            if ledgers.is_empty() {
                self.display_warning("No ledgers found");
                return Ok(());
            }

            let ledger_strings: Vec<String> = ledgers
                .iter()
                .map(|ledger| self.format_ledger_info(ledger))
                .collect();

            let selection = self.get_selection("Select a ledger", &ledger_strings)?;
            let selected_ledger = &ledgers[selection];

            let action_selection =
                self.get_selection("What would you like to do?", &LedgerAction::variants())?;

            match LedgerAction::from_index(action_selection) {
                Some(LedgerAction::ViewDetails) => {
                    self.handle_ledger_details(docker, container_id, selected_ledger)
                        .await?;
                }
                Some(LedgerAction::Delete) => {
                    if self
                        .handle_ledger_deletion(docker, container_id, selected_ledger)
                        .await?
                    {
                        break;
                    }
                }
                Some(LedgerAction::Return) | None => {
                    break;
                }
            }
        }

        Ok(())
    }
}

impl UserInterface for LedgerUI {}
