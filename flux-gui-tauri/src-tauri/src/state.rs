//! Application state for the Tauri backend.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// File status for the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    /// File is synced.
    Synced,
    /// Symlink is missing.
    MissingSymlink,
    /// Symlink is broken.
    BrokenSymlink,
    /// File is out of sync.
    OutOfSync,
    /// File is missing in repo.
    MissingRepo,
}

impl From<flux::commands::status::FileStatus> for FileStatus {
    fn from(status: flux::commands::status::FileStatus) -> Self {
        match status {
            flux::commands::status::FileStatus::Synced => Self::Synced,
            flux::commands::status::FileStatus::MissingSymlink => Self::MissingSymlink,
            flux::commands::status::FileStatus::BrokenSymlink => Self::BrokenSymlink,
            flux::commands::status::FileStatus::OutOfSync => Self::OutOfSync,
            flux::commands::status::FileStatus::MissingRepo => Self::MissingRepo,
        }
    }
}

/// Status report for a tracked file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiStatusReport {
    /// Tool name.
    pub tool: String,
    /// Path in repository.
    pub repo_path: String,
    /// Destination path.
    pub dest_path: String,
    /// Profile (if any).
    pub profile: Option<String>,
    /// Current status.
    pub status: FileStatus,
    /// Status message.
    pub message: String,
}

/// Information about a tracked file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedFileInfo {
    /// Tool name.
    pub tool: String,
    /// Path in repository.
    pub repo_path: String,
    /// Destination path.
    pub dest_path: String,
    /// Profile (if any).
    pub profile: Option<String>,
}

/// Information about a git remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiRemoteInfo {
    /// Remote name.
    pub name: String,
    /// Remote URL.
    pub url: String,
    /// Current branch.
    pub branch: String,
}

/// Git repository status information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusInfo {
    /// Current branch name.
    pub branch: String,
    /// Commits ahead of remote.
    pub ahead: usize,
    /// Commits behind remote.
    pub behind: usize,
    /// Whether a remote is configured.
    pub has_remote: bool,
    /// Whether this is a git repository.
    pub is_repo: bool,
}

/// Sync summary statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    /// Number of synced files.
    pub synced: usize,
    /// Number of out-of-sync files.
    pub out_of_sync: usize,
    /// Number of missing files.
    pub missing: usize,
    /// Number of broken/conflicted files.
    pub broken: usize,
    /// Total number of files.
    pub total: usize,
}

/// Snapshot of application state for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    /// Current profile.
    pub current_profile: String,
    /// Repository path.
    pub repo_path: String,
    /// Dry run mode enabled.
    pub dry_run_mode: bool,
    /// Whether config has been loaded.
    pub config_loaded: bool,
    /// Sync summary.
    pub sync_summary: SyncSummary,
    /// Git status.
    pub git_status: GitStatusInfo,
}

/// Main application state stored in Tauri.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Current active profile.
    pub current_profile: String,
    /// Repository path.
    pub repo_path: PathBuf,
    /// Dry run mode.
    pub dry_run_mode: bool,
    /// Whether config has been loaded.
    pub config_loaded: bool,
    /// Sync summary.
    pub sync_summary: SyncSummary,
    /// Git status.
    pub git_status: GitStatusInfo,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new() -> Self {
        let repo_path =
            dirs::home_dir().map_or_else(|| PathBuf::from(".dotfiles"), |h| h.join(".dotfiles"));

        Self {
            current_profile: "default".to_string(),
            repo_path,
            dry_run_mode: false,
            config_loaded: false,
            sync_summary: SyncSummary::default(),
            git_status: GitStatusInfo::default(),
        }
    }

    /// Create a snapshot for the frontend.
    #[must_use]
    pub fn snapshot(&self) -> AppStateSnapshot {
        AppStateSnapshot {
            current_profile: self.current_profile.clone(),
            repo_path: self.repo_path.display().to_string(),
            dry_run_mode: self.dry_run_mode,
            config_loaded: self.config_loaded,
            sync_summary: self.sync_summary.clone(),
            git_status: self.git_status.clone(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
