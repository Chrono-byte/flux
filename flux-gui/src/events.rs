//! Event types for the Flux GUI application.

use crate::state::{AppView, DiffLine};

/// Application events that drive state changes
#[derive(Debug, Clone)]
pub enum AppEvent {
    // Navigation
    /// Switch to a different view
    SwitchView(AppView),

    // File operations
    /// Request to add a new file
    AddFile {
        /// Tool name
        tool: String,
        /// Source file path
        path: String,
        /// Destination path
        dest: String,
    },
    /// Request to remove a tracked file
    RemoveFile {
        /// Tool name
        tool: String,
        /// File name in repo
        file: String,
    },
    /// Refresh the file list
    RefreshFileList {
        /// Optional profile filter
        profile: Option<String>,
    },
    /// File operation succeeded
    FileOperationSuccess {
        /// Success message
        message: String,
    },

    // Profile operations
    /// Create a new profile
    CreateProfile {
        /// Profile name
        name: String,
    },
    /// Switch to a different profile
    SwitchProfile {
        /// Profile name
        name: String,
    },
    /// Refresh profile list
    RefreshProfiles,

    // Git operations
    /// Push to remote
    Push {
        /// Remote name (optional, uses default)
        remote: Option<String>,
        /// Dry run mode
        dry_run: bool,
    },
    /// Pull from remote
    Pull {
        /// Remote name (optional, uses default)
        remote: Option<String>,
        /// Dry run mode
        dry_run: bool,
    },
    /// Add a new remote
    AddRemote {
        /// Remote name
        name: String,
        /// Remote URL
        url: String,
    },
    /// Remove a remote
    RemoveRemote {
        /// Remote name
        name: String,
    },
    /// Refresh remotes list
    RefreshRemotes,

    // Config operations
    /// Apply configuration (sync files)
    ApplyConfig {
        /// Profile to apply (optional)
        profile: Option<String>,
        /// Dry run mode
        dry_run: bool,
    },
    /// Commit and sync changes
    CommitAndSync {
        /// Commit message
        message: String,
        /// Dry run mode
        dry_run: bool,
    },
    /// Load configuration for editing
    LoadConfigForEdit,
    /// Save edited configuration
    SaveConfig {
        /// New config content
        content: String,
    },
    /// Validate configuration
    ValidateConfig {
        /// Config content to validate
        content: String,
    },

    // Status operations
    /// Refresh sync status
    RefreshStatus {
        /// Optional profile filter
        profile: Option<String>,
    },

    // Error/notification handling
    /// Show error message
    ShowError {
        /// Error message
        message: String,
    },
    /// Clear error state
    ClearError,
    /// Show success message
    ShowSuccess {
        /// Success message
        message: String,
    },
    /// Clear notification
    ClearNotification,

    // Loading states
    /// Set loading state
    SetLoading {
        /// Is loading
        loading: bool,
    },

    // Diff preview
    /// Load diff for files
    LoadDiff {
        /// File paths to diff
        files: Vec<String>,
    },
    /// Diff loaded
    DiffLoaded {
        /// Diff lines
        lines: Vec<DiffLine>,
    },

    // Window management
    /// Save window geometry
    SaveWindowGeometry,
    /// Load window geometry
    LoadWindowGeometry,
}
