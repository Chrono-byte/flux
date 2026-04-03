//! CLI argument parsing for the Flux dotfiles manager.

use clap::{Parser, Subcommand};

/// Command line interface for the Flux dotfiles manager.
#[derive(Parser)]
#[command(name = "flux")]
#[command(about = "A tool to manage dotfiles with symlink-based sync")]
#[command(version)]
pub struct Cli {
    /// Path to configuration file (overrides `DOTFILES_CONFIG` env var)
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Verbose output (show detailed progress)
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Quiet output (suppress non-error messages)
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// Set log level (trace, debug, info, warn, error)
    #[arg(long, global = true)]
    pub log_level: Option<String>,

    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Commands for the Flux dotfiles manager.
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize dotfiles repository
    Init {
        /// Repository path (default: ~/.dotfiles)
        #[arg(long)]
        repo_path: Option<String>,
    },
    /// Add a file to tracking
    Add {
        /// Tool name (e.g., sway, waybar, cursor)
        tool: String,
        /// Source file path
        file: String,
        /// Destination path in home directory
        #[arg(long)]
        dest: Option<String>,
        /// Profile name (optional)
        #[arg(long)]
        profile: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
        /// File already exists in repo - just register it, don't copy
        #[arg(long)]
        from_repo: bool,
    },
    /// Sync tracked files (create symlinks) and commit changes to repository
    Commit {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Commit message (optional, will prompt if not provided)
        #[arg(long)]
        message: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
        /// Verbose output (show detailed progress for each file)
        #[arg(long, short = 'v')]
        verbose: bool,
    },
    /// Remove a file from tracking
    Rm {
        /// Tool name
        tool: String,
        /// File name in repository
        file: String,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// List all tracked files
    #[command(visible_alias = "list")]
    LsFiles {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Apply configuration declaratively
    Apply {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Dry run mode (preview changes without applying)
        #[arg(long)]
        dry_run: bool,
        /// Show unified diffs for files that would change (dry-run only)
        #[arg(long)]
        diff: bool,
        /// Skip confirmation prompts
        #[arg(long)]
        yes: bool,
        /// Description for this generation
        #[arg(long)]
        description: Option<String>,
        /// Force sync: replace all files that aren't correct symlinks (no backups, uses repo version)
        #[arg(long)]
        force: bool,
    },
    /// View operation history
    History {
        /// Limit number of events to show
        #[arg(long, short = 'n')]
        limit: Option<usize>,
        /// Filter by profile name
        #[arg(long)]
        profile: Option<String>,
        /// Filter by event kind (apply, rollback, doctor, import, etc.)
        #[arg(long)]
        kind: Option<String>,
        /// Show events since this time (e.g., "1 hour ago", "2024-01-01")
        #[arg(long)]
        since: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Profile management
    Profile {
        /// Command to execute
        #[command(subcommand)]
        command: ProfileCommands,
    },
    /// Manage remote repositories
    Remote {
        /// Command to execute
        #[command(subcommand)]
        command: RemoteCommands,
    },
    /// Push changes to remote repository
    Push {
        /// Remote name (default: origin or config `default_remote`)
        #[arg(long)]
        remote: Option<String>,
        /// Branch name (default: current HEAD or config `default_branch`)
        #[arg(long)]
        branch: Option<String>,
        /// Set upstream after push
        #[arg(long)]
        set_upstream: bool,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
        /// Timeout in seconds (default: 60 or config `push_timeout`)
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Pull changes from remote repository
    Pull {
        /// Remote name (default: origin or config `default_remote`)
        #[arg(long)]
        remote: Option<String>,
        /// Branch name (default: current HEAD or config `default_branch`)
        #[arg(long)]
        branch: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
        /// Timeout in seconds (default: 60 or config `push_timeout`)
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Show repository and file sync status
    Status {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Show only conflicted files (git merge conflicts)
        #[arg(long)]
        conflicts: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show git-style diffs between repository files and their destinations
    Diff {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Force diff evaluation even if symlinks are already correct
        #[arg(long)]
        force: bool,
        /// Print only file paths of tracked files with differences
        #[arg(long)]
        name_only: bool,
    },
    /// Maintenance and repair operations
    Maintain {
        /// Command to execute
        #[command(subcommand)]
        command: MaintainCommands,
    },
    /// Generate shell completions
    Completion {
        /// Shell type (zsh supported only)
        shell: String,
    },
    /// Health check and diagnostics (includes validation and state diff)
    Doctor {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
    },
}

/// Commands for maintenance and repair operations.
#[derive(Subcommand)]
pub enum MaintainCommands {
    /// Migrate files with discrepancies: copy current files to repo and create symlinks
    Migrate {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
        /// Skip backup and copy - just remove existing files and create symlinks to repo
        #[arg(long)]
        no_backup: bool,
    },
    /// Generate a .gitignore file for the repository
    Gitignore,
}

#[derive(Subcommand)]
/// Commands for managing profiles.
pub enum ProfileCommands {
    /// List all profiles
    List,
    /// Create a new profile
    Create {
        /// Profile name
        name: String,
    },
    /// Switch to a profile
    Switch {
        /// Profile name
        name: String,
    },
}

#[derive(Subcommand)]
/// Commands for managing remotes.
pub enum RemoteCommands {
    /// List all remotes
    List,
    /// Add a remote repository
    Add {
        /// Remote name (e.g., origin)
        name: String,
        /// Remote URL (git@... or https://...)
        url: String,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Set or change remote URL
    SetUrl {
        /// Remote name
        name: String,
        /// New remote URL
        url: String,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove a remote repository
    Remove {
        /// Remote name
        name: String,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
}
