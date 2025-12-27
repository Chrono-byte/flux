/// Dry-run operation tracking.
pub mod dry_run;
/// Error types and result handling.
pub mod error;
/// Error utility functions.
pub mod error_utils;
/// Logging configuration and utilities.
pub mod logging;
/// Path manipulation utilities.
pub mod path_utils;
/// User prompt utilities.
pub mod prompt;
/// Security and permission utilities.
pub mod security;

pub use dry_run::DryRun;
pub use error::{DotfilesError, Result};
