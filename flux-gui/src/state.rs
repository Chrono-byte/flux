//! Core state types for the Flux GUI application.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The current view being displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    /// Home/dashboard view
    Home,
    /// File list and management
    Files,
    /// File sync status detail
    Status,
    /// Profile management
    Profiles,
    /// Git remotes management
    Remotes,
    /// Configuration editor
    Settings,
    /// Operation history
    History,
}

/// Status of a tracked file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// File is properly synced
    Synced,
    /// File has diverged from repo
    OutOfSync,
    /// File has a conflict
    Conflict,
    /// Pending operation
    Pending,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Synced => write!(f, "✓ Synced"),
            Self::OutOfSync => write!(f, "↻ Out of Sync"),
            Self::Conflict => write!(f, "⚠ Conflict"),
            Self::Pending => write!(f, "⏳ Pending"),
        }
    }
}

/// Information about a git remote
#[derive(Debug, Clone)]
pub struct RemoteInfo {
    /// Remote name (e.g., "origin")
    pub name: String,
    /// Remote URL
    pub url: String,
}

/// Result of an operation
#[derive(Debug, Clone)]
pub struct OperationResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Result message
    pub message: String,
    /// Timestamp of operation
    pub timestamp: DateTime<Utc>,
}

/// Sync summary statistics
#[derive(Debug, Clone, Default)]
pub struct SyncSummary {
    /// Number of synced files
    pub synced: usize,
    /// Number of out-of-sync files
    pub out_of_sync: usize,
    /// Number of conflicted files
    pub conflicts: usize,
    /// Number of pending files
    pub pending: usize,
}

impl SyncSummary {
    /// Get total number of tracked files
    #[must_use]
    pub fn total(&self) -> usize {
        self.synced + self.out_of_sync + self.conflicts + self.pending
    }
}

/// Diff line for display
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Line number in original
    pub original_line: usize,
    /// Line number in modified
    pub modified_line: usize,
    /// Type of change
    pub change_type: DiffChangeType,
    /// Line content
    pub content: String,
}

/// Type of diff change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffChangeType {
    /// Line was added
    Added,
    /// Line was removed
    Removed,
    /// Line was modified
    Modified,
    /// Line is context (unchanged)
    Context,
}

/// Window geometry for state persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Window width
    pub width: i32,
    /// Window height
    pub height: i32,
    /// Window X position
    pub x: i32,
    /// Window Y position
    pub y: i32,
    /// Is window maximized
    pub maximized: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            width: 1400,
            height: 900,
            x: 100,
            y: 100,
            maximized: false,
        }
    }
}
