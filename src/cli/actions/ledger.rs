//! Ledger action handling.
//!
//! This module provides the ledger action enum and implementations
//! for managing ledger operations.

/// Available actions when viewing a ledger
#[derive(Debug)]
pub enum LedgerAction {
    ViewDetails,
    Delete,
    Return,
    GoBack,
}

impl LedgerAction {
    /// Get list of action variants as strings
    pub fn variants() -> Vec<&'static str> {
        vec![
            "See More Details",
            "Delete Ledger",
            "Return to Ledger List",
            "Go Back to Container Menu",
        ]
    }

    /// Convert a selection index to an action
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::ViewDetails),
            1 => Some(Self::Delete),
            2 => Some(Self::Return),
            3 => Some(Self::GoBack),
            _ => None,
        }
    }
}
