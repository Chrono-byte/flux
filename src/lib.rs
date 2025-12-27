//! Flux - A dotfiles manager with symlink-based sync
//!
//! This library provides the core functionality for managing dotfiles.
//! The binary entry point is in `main.rs`.

/// Command implementations for CLI subcommands.
pub mod commands;
/// Configuration management and profile handling.
pub mod config;
/// File operations and symlink management.
pub mod file_manager;
/// Service layer for git, hooks, and other operations.
pub mod services;
/// Type definitions and data structures.
pub mod types;
/// Utility functions for error handling, logging, and prompts.
pub mod utils;

// Re-export commonly used types
pub use utils::error::{DotfilesError, Result};
