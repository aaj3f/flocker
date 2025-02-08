//! Action handling for containers and ledgers.
//!
//! This module provides enums and implementations for:
//! - Container actions (start, stop, view stats, etc.)
//! - Ledger actions (view details, delete, etc.)

mod container;
mod ledger;

pub use container::RunningContainerAction;
pub use ledger::LedgerAction;
