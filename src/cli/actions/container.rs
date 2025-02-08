//! Container action handling.
//!
//! This module provides the container action enum and implementations
//! for managing container lifecycle and operations.

/// Available actions when a container is running
#[derive(Debug)]
pub enum RunningContainerAction {
    ViewStats,
    ViewLogs,
    ListLedgers,
    Stop,
    StopAndDestroy,
    GoBack,
}

impl RunningContainerAction {
    /// Get list of action variants as strings
    pub fn variants() -> Vec<&'static str> {
        vec![
            "View Container Stats",
            "View Container Logs",
            "List Ledgers",
            "Stop Container",
            "Stop and Destroy Container",
            "Go Back to Container List",
        ]
    }

    /// Convert a selection index to an action
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::ViewStats),
            1 => Some(Self::ViewLogs),
            2 => Some(Self::ListLedgers),
            3 => Some(Self::Stop),
            4 => Some(Self::StopAndDestroy),
            5 => Some(Self::GoBack),
            _ => None,
        }
    }
}
