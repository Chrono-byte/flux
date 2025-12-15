//! Flux - A dotfiles manager with symlink-based sync
//!
//! This library provides the core functionality for managing dotfiles.
//! The binary entry point is in `main.rs`.

pub mod commands;
pub mod config;
pub mod file_manager;
pub mod services;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use utils::error::{DotfilesError, Result};
