//! flux - A dotfiles manager with symlink-based sync
//!
//! This is the binary entry point for the Flux dotfiles manager.
//! The library functionality is provided in `lib.rs`.

/// Command implementations for CLI subcommands.
mod commands;
/// Configuration management and profile handling.
mod config;
/// File operations and symlink management.
mod file_manager;
/// Service layer for git, hooks, and other operations.
mod services;
/// Type definitions and data structures.
mod types;
/// Utility functions for error handling, logging, and prompts.
mod utils;

use clap::{CommandFactory, Parser, Subcommand};
use colored::{Colorize, control};
use commands::{
    apply_config, check_status, compare_states, display_discrepancies, display_health_report,
    display_preview, display_status, display_validation, find_discrepancies, migrate_files,
    run_health_checks, validate_config,
};
use config::profile::{create_profile, get_profile_files, list_profiles, switch_profile};
use config::{Config, EnvironmentConfig};
use file_manager::{add_file, remove_file, sync_files};
use services::git;
use services::{
    add_remote, commit_changes, detect_changes, init_repo, list_remotes, pull_from_remote,
    push_to_remote, read_events, remove_remote, set_remote_url, show_git_status, stage_changes,
};
use utils::prompt::prompt_commit_message;
use utils::{DotfilesError, DryRun, Result, logging};

/// Command line interface for the Flux dotfiles manager.
#[derive(Parser)]
#[command(name = "flux")]
#[command(about = "A tool to manage dotfiles with symlink-based sync")]
#[command(version)]
struct Cli {
    /// Path to configuration file (overrides `DOTFILES_CONFIG` env var)
    #[arg(long, global = true)]
    config: Option<String>,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Verbose output (show detailed progress)
    #[arg(short = 'v', long, global = true)]
    verbose: bool,

    /// Quiet output (suppress non-error messages)
    #[arg(short = 'q', long, global = true)]
    quiet: bool,

    /// Set log level (trace, debug, info, warn, error)
    #[arg(long, global = true)]
    log_level: Option<String>,

    /// Command to execute
    #[command(subcommand)]
    command: Commands,
}

/// Commands for the Flux dotfiles manager.
#[derive(Subcommand)]
enum Commands {
    /// Initialize dotfiles repository
    Init {
        /// Repository path (default: ~/.dotfiles)
        #[arg(long)]
        repo_path: Option<String>,
    },
    /// Add a file to tracking
    Add {
        /// Tool name (e.g., sway, waybar, cursor, firefox, zen)
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
    /// Configuration management operations
    Config {
        /// Command to execute
        #[command(subcommand)]
        command: ConfigCommands,
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
    /// Maintenance and repair operations
    Maintain {
        /// Command to execute
        #[command(subcommand)]
        command: MaintainCommands,
    },
    /// Generate shell completions
    Completion {
        /// Shell type (zsh, bash, fish, etc.)
        shell: String,
    },
    /// CI-safe validation (read-only, no writes, no prompts, no hooks)
    Check {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Health check and diagnostics
    Doctor,
}

/// Commands for maintenance and repair operations.
#[derive(Subcommand)]
enum MaintainCommands {
    /// Check for discrepancies in tracked files (missing, wrong target, content differs, etc.)
    Check {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Validate configuration integrity
    Validate,
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
enum ProfileCommands {
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
enum RemoteCommands {
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

#[derive(Subcommand)]
enum ConfigCommands {
    /// Sync XDG config to repo (overwrite repo config with XDG config)
    Sync {
        /// Dry run mode (show what would be done without actually doing it)
        #[arg(long)]
        dry_run: bool,
    },
    /// Format and organize XDG config file
    Format {
        /// Dry run mode (show what would be done without actually doing it)
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    // Handle --no-color flag (disable colored output programmatically)
    if cli.no_color {
        control::set_override(false);
    }

    // Handle --config flag (set static override for Config::load() to use)
    if let Some(config_path) = &cli.config {
        Config::set_path_override(config_path.as_ref());
    }

    // Load and validate environment configuration early
    let env_config = match EnvironmentConfig::load() {
        Ok(config) => {
            if !cli.quiet {
                config.display_summary();
            }
            config
        }
        Err(err) => {
            eprintln!(
                "{} Failed to load environment configuration: {}",
                "Error:".red().bold(),
                err
            );
            eprintln!(
                "{} Please check your environment variables",
                "Help:".yellow().bold()
            );
            std::process::exit(1);
        }
    };

    // Initialize logging system with CLI log level
    logging::init_logging(cli.log_level.as_deref());

    if let Err(err) = run(cli, env_config) {
        eprintln!("{} {}", "Error:".red().bold(), err);
        std::process::exit(1);
    }
}

/// Handle the maintain command.
///
/// # Parameters
///
/// * `command`: The maintain command to handle.
///
/// # Returns
///
/// Returns the result of the maintain command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the maintain command cannot be executed.
fn handle_maintain_command(command: MaintainCommands) -> Result<()> {
    match command {
        MaintainCommands::Check { profile } => {
            let config = Config::load()?;
            let discrepancies = find_discrepancies(&config, profile.as_deref())?;
            display_discrepancies(&discrepancies);

            if !discrepancies.is_empty() {
                std::process::exit(1);
            }
        }
        MaintainCommands::Validate => {
            let config = Config::load()?;
            let report = validate_config(&config)?;
            display_validation(&report);
            if !report.is_valid {
                std::process::exit(1);
            }
        }
        MaintainCommands::Migrate {
            profile,
            dry_run,
            no_backup,
        } => {
            let config = Config::load()?;
            let mut dry_run_tracker = DryRun::default();

            migrate_files(
                &config,
                profile.as_deref(),
                &mut dry_run_tracker,
                dry_run,
                no_backup,
            )?;

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        MaintainCommands::Gitignore => {
            let config = Config::load()?;
            let repo_path = config.get_repo_path();
            let gitignore_path = repo_path.join(".gitignore");

            let gitignore_content = r"# OS files
.DS_Store
Thumbs.db
*.swp
*.swo
*~
.#*

# Editor files
.idea/
.vscode/
*.swp
*.swo
*~
.vim/
.swapfile

# Temporary files
*.tmp
*.temp
*.log

# Backup files
*.bak
*.backup
*~

# Node modules (if configs contain web stuff)
node_modules/

# Python cache
__pycache__/
*.pyc

# System files
.directory
desktop.ini
";

            std::fs::write(&gitignore_path, gitignore_content)?;
            println!(
                "{} Created {} with standard ignore patterns",
                "✓".green(),
                gitignore_path.display()
            );
        }
    }
    Ok(())
}

/// Handle the init command.
///
/// # Parameters
///
/// * `repo_path`: Optional repository path to initialize.
///
/// # Returns
///
/// Returns the result of the init command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the repository cannot be initialized.
fn handle_init_command(repo_path: Option<String>) -> Result<()> {
    let mut config = Config::load()?;
    if let Some(path) = repo_path {
        config.general.repo_path = path;
    }
    config.save(false)?;

    let repo_path = config.get_repo_path();
    std::fs::create_dir_all(&repo_path)?;

    let repo = init_repo(&repo_path)?;
    println!(
        "{} Initialized repository at {}",
        "✓".green(),
        repo_path.display()
    );
    println!(
        "   Git repository: {}",
        if repo.path().exists() {
            "initialized"
        } else {
            "not initialized"
        }
    );
    Ok(())
}

/// Handle the add command.
///
/// # Parameters
///
/// * `tool`: The tool name.
/// * `file`: The source file path.
/// * `dest`: Optional destination path.
/// * `profile`: Optional profile name.
/// * `dry_run`: Whether to perform a dry run.
/// * `from_repo`: Whether the file already exists in the repository.
///
/// # Returns
///
/// Returns the result of the add command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the file cannot be added.
fn handle_add_command(
    tool: &str,
    file: &str,
    dest: Option<&str>,
    profile: Option<&str>,
    dry_run: bool,
    from_repo: bool,
) -> Result<()> {
    let mut config = Config::load()?;
    let mut dry_run_tracker = DryRun::default();
    let mut fs_manager = file_manager::FileSystemManager::new(&mut dry_run_tracker, dry_run);

    if from_repo {
        // File already exists in repo - register it without copying
        let repo_path = config.get_repo_path();
        let repo_file = repo_path.join(tool).join(file);

        if !repo_file.exists() {
            return Err(DotfilesError::Path(format!(
                "File does not exist in repo: {}",
                repo_file.display()
            )));
        }

        let dest_path = dest.map_or_else(
            || std::path::Path::new(file).to_path_buf(),
            |dest| std::path::Path::new(dest).to_path_buf(),
        );

        // Register file in config without copying (file already in repo)
        let repo_relative = repo_file
            .strip_prefix(&repo_path)
            .map_err(|_| DotfilesError::Path("Could not compute repo relative path".to_string()))?
            .to_string_lossy()
            .to_string();

        config.add_file_to_tool(tool, &repo_relative, &dest_path, profile);

        if dry_run {
            println!(
                "  [DRY RUN] Would register {} from repo",
                repo_file.display()
            );
        } else {
            config.save(false)?;
            // Git-like output: "add 'file'"
            println!("add '{}'", dest_path.display());
        }
    } else {
        // Standard flow: copy source file to repository
        let source_path = std::path::Path::new(file);
        if !source_path.exists() {
            return Err(DotfilesError::Path(format!(
                "Source file does not exist: {file}"
            )));
        }

        let dest_path = if let Some(dest) = dest {
            std::path::Path::new(dest).to_path_buf()
        } else {
            // Use source path relative to home directory
            let home = dirs::home_dir().ok_or_else(|| {
                DotfilesError::Config("Could not find home directory".to_string())
            })?;
            source_path
                .strip_prefix(&home)
                .map_or_else(|_| source_path.to_path_buf(), std::path::Path::to_path_buf)
        };

        add_file(
            &mut config,
            tool,
            source_path,
            &dest_path,
            profile,
            &mut fs_manager,
        )?;
    }

    if dry_run {
        dry_run_tracker.display_summary();
    }
    Ok(())
}

/// Handle the commit command.
///
/// # Parameters
///
/// * `profile`: Optional profile name.
/// * `message`: Optional commit message.
/// * `dry_run`: Whether to perform a dry run.
/// * `verbose`: Whether to show verbose output.
///
/// # Returns
///
/// Returns the result of the commit command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the commit cannot be performed.
fn handle_commit_command(
    profile: Option<&str>,
    message: Option<&str>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let config = Config::load()?;
    let mut dry_run_tracker = DryRun::default();

    // Run pre-commit hooks (if not dry-run)
    if !dry_run
        && let Some(hooks_config) = &config.hooks
        && let Some(pre_hooks) = &hooks_config.pre_commit
    {
        use crate::services::hooks::execute_hooks;
        println!("{} Running pre-commit hooks...", "→".cyan());
        let hook_results = execute_hooks(pre_hooks, &config, None, true)?;
        for result in hook_results {
            if !result.success {
                return Err(DotfilesError::Config(format!(
                    "Pre-commit hook failed: {} (exit code: {:?})",
                    result.command, result.exit_code
                )));
            }
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() && !result.stderr.contains("Hook skipped") {
                eprint!("{}", result.stderr);
            }
        }
    }

    // Sync files first (quietly unless verbose)
    sync_files(&config, profile, &mut dry_run_tracker, dry_run, verbose)?;

    if dry_run {
        dry_run_tracker.display_summary();
    } else {
        // Automatically commit any changes to the repository
        let repo_path = config.get_repo_path();
        let repo = init_repo(&repo_path)?;
        let changes = detect_changes(&repo)?;

        if !changes.is_empty() {
            let commit_message = if let Some(msg) = message {
                msg.to_string()
            } else {
                prompt_commit_message(&changes)?
            };
            stage_changes(&repo, &changes, &mut dry_run_tracker, dry_run)?;
            commit_changes(&repo, &commit_message, &mut dry_run_tracker, dry_run)?;

            // Run post-commit hooks
            if let Some(hooks_config) = &config.hooks
                && let Some(post_hooks) = &hooks_config.post_commit
            {
                use crate::services::hooks::execute_hooks;
                println!("{} Running post-commit hooks...", "→".cyan());
                let hook_results = execute_hooks(post_hooks, &config, None, false)?; // Don't fail-fast on post hooks
                for result in hook_results {
                    if !result.success {
                        eprintln!(
                            "{} Post-commit hook failed (non-fatal): {} (exit code: {:?})",
                            "⚠".yellow(),
                            result.command,
                            result.exit_code
                        );
                    }
                    if !result.stdout.is_empty() {
                        print!("{}", result.stdout);
                    }
                    if !result.stderr.is_empty() && !result.stderr.contains("Hook skipped") {
                        eprint!("{}", result.stderr);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Handle the remove command.
///
/// # Parameters
///
/// * `tool`: The tool name.
/// * `file`: The file name to remove.
/// * `dry_run`: Whether to perform a dry run.
///
/// # Returns
///
/// Returns the result of the remove command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the file cannot be removed.
fn handle_rm_command(tool: &str, file: &str, dry_run: bool) -> Result<()> {
    let mut config = Config::load()?;
    let mut dry_run_tracker = DryRun::default();
    let mut fs_manager = file_manager::FileSystemManager::new(&mut dry_run_tracker, dry_run);
    remove_file(&mut config, tool, file, &mut fs_manager)?;

    if dry_run {
        dry_run_tracker.display_summary();
    }
    Ok(())
}

/// Handle the list files command.
///
/// # Parameters
///
/// * `profile`: Optional profile name.
///
/// # Returns
///
/// Returns the result of the list files command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the files cannot be listed.
fn handle_ls_files_command(profile: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let files = config.get_tracked_files(profile)?;

    // Git-like output: one file per line, no header, just paths
    for file in files {
        // Show repo-relative path (like git ls-files shows paths relative to repo root)
        let repo_path = config.get_repo_path();
        if let Ok(rel_path) = file.repo_path.strip_prefix(&repo_path) {
            println!("{}", rel_path.display());
        } else {
            // Fallback to full path if we can't compute relative
            println!("{}", file.repo_path.display());
        }
    }
    Ok(())
}

/// Handle the apply command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be applied or if the configuration cannot be loaded.
#[allow(clippy::fn_params_excessive_bools)]
fn handle_apply_command(
    profile: Option<&str>,
    dry_run: bool,
    diff: bool,
    yes: bool,
    description: Option<&str>,
    force: bool,
) -> Result<()> {
    let config = Config::load()?;

    // Normal apply command
    if dry_run {
        // In dry-run mode, only show preview without applying changes
        let state_diff = compare_states(&config, profile, force)?;
        display_preview(&state_diff);
        if diff && !state_diff.files_to_sync.is_empty() {
            commands::display_diffs(&state_diff);
        }
    } else {
        use crate::commands::ApplyOptions;
        // apply_config now handles history recording internally
        apply_config(&ApplyOptions {
            config: &config,
            profile,
            dry_run,
            yes,
            description,
            force,
        })?;
    }
    Ok(())
}

/// Parse a since time string into a `DateTime`.
///
/// # Parameters
///
/// * `since_str`: The since time string to parse.
///
/// # Returns
///
/// Returns the parsed `DateTime`.
fn parse_since_time(since_str: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    since_str.parse::<i64>().map_or_else(
        |_| {
            chrono::DateTime::parse_from_rfc3339(since_str).map_or_else(
                |_| {
                    chrono::NaiveDateTime::parse_from_str(since_str, "%Y-%m-%d %H:%M:%S")
                        .map_or_else(
                            |_| {
                                // Try just date
                                chrono::NaiveDate::parse_from_str(since_str, "%Y-%m-%d")
                                    .map_or_else(
                                        |_| {
                                            Err(DotfilesError::Config(format!(
                                                "Invalid 'since' format: {since_str}. Use ISO 8601, 'YYYY-MM-DD', or hours as number"
                                            )))
                                        },
                                        |date| {
                                            date.and_hms_opt(0, 0, 0)
                                                .ok_or_else(|| {
                                                    DotfilesError::Config(format!(
                                                        "Invalid date: {since_str}. Could not create datetime from date"
                                                    ))
                                                })
                                                .map(|dt| dt.and_utc())
                                        },
                                    )
                            },
                            |dt| Ok(dt.and_utc()),
                        )
                },
                |dt| Ok(dt.with_timezone(&chrono::Utc)),
            )
        },
        |hours| {
            // Interpret as hours ago
            Ok(chrono::Utc::now() - chrono::Duration::hours(hours))
        },
    )
}

/// Handle the history command.
///
/// # Parameters
///
/// * `limit`: Optional limit on number of events to show.
/// * `profile`: Optional profile name to filter by.
/// * `kind`: Optional event kind to filter by.
/// * `since`: Optional time string to filter events since.
/// * `json`: Whether to output as JSON.
///
/// # Returns
///
/// Returns the result of the history command.
///
/// # Errors
///
/// Returns an error if the history cannot be read or if the filters cannot be applied.
fn handle_history_command(
    limit: Option<usize>,
    profile: Option<&str>,
    kind: Option<&str>,
    since: Option<&str>,
    json: bool,
) -> Result<()> {
    let mut events = read_events(None)?; // Read all, filter manually

    // Apply filters
    if let Some(profile_filter) = &profile {
        events.retain(|event| event.profile.as_deref() == Some(profile_filter));
    }

    if let Some(kind_filter) = &kind {
        events.retain(|event| event.command.starts_with(kind_filter));
    }

    if let Some(since_string) = &since {
        let since_time = parse_since_time(since_string)?;
        events.retain(|event| event.timestamp >= since_time);
    }

    // Apply limit after filtering
    if let Some(max_events) = limit {
        events.truncate(max_events);
    }

    if events.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("{} No history found", "⊘".yellow());
        }
        return Ok(());
    }

    if json {
        // Output as JSON
        let json_str = serde_json::to_string_pretty(&events).map_err(|err| {
            DotfilesError::Config(format!("Failed to serialize history to JSON: {err}"))
        })?;
        println!("{json_str}");
    } else {
        println!("\n{} Operation History:", "→".cyan().bold());
        for event in &events {
            let profile_str = event.profile.as_deref().map_or_else(String::new, |profile_name| {
                format!(" [profile: {profile_name}]")
            });
            let dry_run_str = if event.dry_run { " [DRY RUN]" } else { "" };
            let summary = event.summary.as_deref().unwrap_or("(no summary)");
            println!(
                "  {} {} - {}{}{}",
                "•".cyan(),
                event.timestamp.format("%Y-%m-%d %H:%M:%S"),
                event.command,
                profile_str,
                dry_run_str
            );
            if !summary.is_empty() && summary != "(no summary)" {
                println!("    {summary}");
            }
        }
    }
    Ok(())
}

/// Handle the profile command.
///
/// # Parameters
///
/// * `command`: The profile command to handle.
///
/// # Returns
///
/// Returns the result of the profile command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the profile cannot be created or switched.
fn handle_profile_command(command: ProfileCommands) -> Result<()> {
    let mut config = Config::load()?;
    match command {
        ProfileCommands::List => {
            let profiles = list_profiles(&config)?;
            println!("\n{}", "Profiles:".bold().cyan());
            for profile in &profiles {
                let marker = if profile == &config.general.current_profile {
                    "→"
                } else {
                    " "
                };
                let profile_files = get_profile_files(&config, profile)?;
                let file_count = profile_files.len();
                println!(
                    "  {} {} ({} file(s))",
                    marker.green(),
                    profile,
                    if file_count > 0 {
                        file_count.to_string().cyan()
                    } else {
                        "0".yellow()
                    }
                );
            }
        }
        ProfileCommands::Create { name } => {
            create_profile(&config, &name)?;
        }
        ProfileCommands::Switch { name } => {
            switch_profile(&mut config, &name)?;
        }
    }
    Ok(())
}

/// Handle the config command.
///
/// # Parameters
///
/// * `command`: The config command to handle.
///
/// # Returns
///
/// Returns the result of the config command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be synced or formatted.
/// Handle the config command.
///
/// # Parameters
///
/// * `command`: The config command to handle.
///
/// # Returns
///
/// Returns the result of the config command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be synced or formatted.
fn handle_config_command(command: &ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Sync { dry_run } => {
            Config::sync_repo_to_xdg(*dry_run)?;
        }
        ConfigCommands::Format { dry_run } => {
            let xdg_config = Config::get_xdg_config_path()?;

            if !xdg_config.exists() {
                return Err(DotfilesError::Config(
                    "XDG config does not exist. Nothing to format.".to_string(),
                ));
            }

            if *dry_run {
                println!(
                    "{} [DRY RUN] Would format and organize {}",
                    "⊘".yellow(),
                    xdg_config.display()
                );
                return Ok(());
            }

            // Load config (prefers XDG location if it exists)
            let config = Config::load()?;

            // Save formatted config back to XDG location
            config.save(true)?;

            println!(
                "{} Formatted and organized config: {}",
                "✓".green(),
                xdg_config.display()
            );
        }
    }
    Ok(())
}

/// Handle the remote command.
///
/// # Parameters
///
/// * `command`: The remote command to handle.
///
/// # Returns
///
/// Returns the result of the remote command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the remote cannot be added, set, or removed.
fn handle_remote_command(command: RemoteCommands) -> Result<()> {
    let config = Config::load()?;
    let repo_path = config.get_repo_path();
    let repo = init_repo(&repo_path)?;
    let mut dry_run_tracker = DryRun::default();

    match command {
        RemoteCommands::List => {
            list_remotes(&repo)?;
        }
        RemoteCommands::Add { name, url, dry_run } => {
            add_remote(&repo, &name, &url, &mut dry_run_tracker, dry_run)?;
            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        RemoteCommands::SetUrl { name, url, dry_run } => {
            set_remote_url(&repo, &name, &url, &mut dry_run_tracker, dry_run)?;
            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        RemoteCommands::Remove { name, dry_run } => {
            remove_remote(&repo, &name, &mut dry_run_tracker, dry_run)?;
            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
    }
    Ok(())
}

/// Handle the push command.
///
/// # Parameters
///
/// * `remote`: The remote name to push to.
/// * `branch`: The branch to push to.
/// * `set_upstream`: Whether to set the upstream after pushing.
/// * `dry_run`: Whether to perform a dry run.
/// * `timeout`: The timeout in seconds.
///
/// # Returns
///
/// Returns the result of the push command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the push cannot be performed.
async fn handle_push_command(
    remote: Option<String>,
    branch: Option<String>,
    set_upstream: bool,
    dry_run: bool,
    timeout: Option<u64>,
) -> Result<()> {
    let config = Config::load()?;
    let repo_path = config.get_repo_path();
    let repo = init_repo(&repo_path)?;
    let mut dry_run_tracker = DryRun::default();

    // Resolve remote with precedence: CLI flag > config default_remote > "origin"
    let resolved_remote = remote
        .or_else(|| config.general.default_remote.clone())
        .unwrap_or_else(|| "origin".to_string());

    // Resolve branch with precedence: CLI flag > current HEAD > config default_branch > "main"
    let resolved_branch = branch
        .or_else(|| git::get_current_branch(&repo).ok())
        .or_else(|| config.general.default_branch.clone())
        .unwrap_or_else(|| "main".to_string());

    // Resolve timeout with precedence: CLI flag > config push_timeout > 60 seconds
    let resolved_timeout = timeout.or(config.general.push_timeout).unwrap_or(60);

    push_to_remote(
        &repo,
        &resolved_remote,
        &resolved_branch,
        set_upstream,
        resolved_timeout,
        &mut dry_run_tracker,
        dry_run,
    )
    .await?;

    if dry_run {
        dry_run_tracker.display_summary();
    }
    Ok(())
}

/// Handle the pull command.
///
/// # Parameters
///
/// * `remote`: The remote name to pull from.
/// * `branch`: The branch to pull from.
/// * `dry_run`: Whether to perform a dry run.
/// * `timeout`: The timeout in seconds.
///
/// # Returns
///
/// Returns the result of the pull command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the pull cannot be performed.
async fn handle_pull_command(
    remote: Option<String>,
    branch: Option<String>,
    dry_run: bool,
    timeout: Option<u64>,
) -> Result<()> {
    let config = Config::load()?;
    let repo_path = config.get_repo_path();
    let repo = init_repo(&repo_path)?;
    let mut dry_run_tracker = DryRun::default();

    // Resolve remote with precedence: CLI flag > config default_remote > "origin"
    let resolved_remote = remote
        .or_else(|| config.general.default_remote.clone())
        .unwrap_or_else(|| "origin".to_string());

    // Resolve branch with precedence: CLI flag > current HEAD > config default_branch > "main"
    let resolved_branch = branch
        .or_else(|| git::get_current_branch(&repo).ok())
        .or_else(|| config.general.default_branch.clone())
        .unwrap_or_else(|| "main".to_string());

    // Resolve timeout with precedence: CLI flag > config push_timeout > 60 seconds
    let resolved_timeout = timeout.or(config.general.push_timeout).unwrap_or(60);

    pull_from_remote(
        &repo,
        &resolved_remote,
        &resolved_branch,
        resolved_timeout,
        &mut dry_run_tracker,
        dry_run,
    )
    .await?;

    if dry_run {
        dry_run_tracker.display_summary();
    }
    Ok(())
}

/// Handle the status command.
///
/// # Parameters
///
/// * `profile`: Optional profile name.
/// * `conflicts`: Whether to show only conflicted files.
/// * `json`: Whether to output as JSON.
///
/// # Returns
///
/// Returns the result of the status command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if the status cannot be checked.
fn handle_status_command(profile: Option<&str>, conflicts: bool, json: bool) -> Result<()> {
    let config = Config::load()?;
    let repo_path = config.get_repo_path();

    if conflicts {
        // Show only conflicted files
        use commands::status::check_conflicts;
        let conflicted = check_conflicts(&config, profile)?;

        if json {
            use commands::status::JsonStatusReport;
            let json_reports: Vec<JsonStatusReport> = conflicted.iter().map(Into::into).collect();
            println!("{}", serde_json::to_string_pretty(&json_reports)?);
        } else if conflicted.is_empty() {
            println!("{} No conflicted files found", "✓".green());
        } else {
            println!("\n{} Conflicted files:", "⚠".yellow().bold());
            for report in &conflicted {
                println!("  {}", report.message);
            }
            println!(
                "\n{} Found {} conflicted file(s)",
                "Summary:".bold(),
                conflicted.len()
            );
        }
    } else if json {
        // JSON output - skip git status, just show file status
        use commands::status::JsonStatusReport;
        let reports = check_status(&config, profile)?;
        let json_reports: Vec<JsonStatusReport> = reports.iter().map(Into::into).collect();
        println!("{}", serde_json::to_string_pretty(&json_reports)?);
    } else {
        // Display git repository status
        if let Ok(repo) = init_repo(&repo_path) {
            show_git_status(&repo)?;
        }

        // Display file synchronization status
        let reports = check_status(&config, profile)?;
        display_status(&reports);
    }
    Ok(())
}

/// Handle the check command.
///
/// # Parameters
///
/// * `profile`: Optional profile name.
/// * `quiet`: Whether to suppress non-error output.
///
/// # Returns
///
/// Returns the result of the check command (exits with appropriate code).
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if validation fails.
fn handle_check_command(profile: Option<&str>, quiet: bool) -> Result<()> {
    // CI-safe validation: read-only, no writes, no prompts, no hooks, no secrets
    let config = Config::load()?;
    let report = validate_config(&config)?;

    // Also check state diff (read-only)
    let diff = compare_states(&config, profile, false)?;

    if report.is_valid && diff.is_empty() {
        // All good - exit 0
        if !quiet {
            println!(
                "{} Configuration is valid and system is in sync",
                "✓".green()
            );
        }
        std::process::exit(0);
    }

    // Issues found - exit with appropriate code
    if !report.is_valid {
        display_validation(&report);
    }
    if !diff.is_empty() && !quiet {
        println!(
            "\n{} System is not in sync with configuration",
            "⚠".yellow()
        );
        display_preview(&diff);
    }

    // Exit code: 1 for validation issues, 2 for sync issues, 3 for both
    let exit_code = if !report.is_valid && !diff.is_empty() {
        3
    } else if !report.is_valid {
        1
    } else {
        2
    };
    std::process::exit(exit_code);
}

/// Handle the doctor command.
///
/// # Returns
///
/// Returns the result of the doctor command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if health checks fail.
fn handle_doctor_command() -> Result<()> {
    let config = Config::load()?;
    let report = run_health_checks(&config)?;
    display_health_report(&report);

    if !report.is_healthy() {
        std::process::exit(1);
    }
    Ok(())
}

/// Handle the import command.
///
/// # Parameters
///
/// * `command`: The import command to handle.
///
/// # Returns
///
/// Returns the result of the import command.
/// Handle the completion command.
///
/// # Parameters
///
/// * `shell`: The shell type to generate completions for.
///
/// # Returns
///
/// Returns the result of the completion command.
fn handle_completion_command(shell: &str) {
    use clap_complete::{generate, shells::Zsh};
    let mut cmd = Cli::command();
    if shell.to_lowercase().as_str() == "zsh" {
        generate(Zsh, &mut cmd, "flux", &mut std::io::stdout());
    } else {
        eprintln!(
            "{} Unsupported shell: {}. Supported shells: zsh",
            "Error:".red().bold(),
            shell
        );
        std::process::exit(1);
    }
}

/// Main command router.
///
/// # Parameters
///
/// * `cli`: The parsed CLI arguments.
/// * `_env_config`: The environment configuration (validated at startup).
///
/// # Returns
///
/// Returns the result of the command execution.
///
/// # Errors
///
/// Returns an error if any command fails.
fn run(cli: Cli, _env_config: EnvironmentConfig) -> Result<()> {
    // Note: env_config is validated at startup for early error detection.
    // It's used for custom config file paths and git authentication.
    match cli.command {
        Commands::Init { repo_path } => handle_init_command(repo_path)?,
        Commands::Add {
            tool,
            file,
            dest,
            profile,
            dry_run,
            from_repo,
        } => handle_add_command(
            &tool,
            &file,
            dest.as_deref(),
            profile.as_deref(),
            dry_run,
            from_repo,
        )?,
        Commands::Commit {
            profile,
            message,
            dry_run,
            verbose,
        } => handle_commit_command(profile.as_deref(), message.as_deref(), dry_run, verbose)?,
        Commands::Rm {
            tool,
            file,
            dry_run,
        } => handle_rm_command(&tool, &file, dry_run)?,
        Commands::LsFiles { profile } => handle_ls_files_command(profile.as_deref())?,
        Commands::Apply {
            profile,
            dry_run,
            diff,
            yes,
            description,
            force,
        } => handle_apply_command(
            profile.as_deref(),
            dry_run,
            diff,
            yes,
            description.as_deref(),
            force,
        )?,
        Commands::History {
            limit,
            profile,
            kind,
            since,
            json,
        } => handle_history_command(
            limit,
            profile.as_deref(),
            kind.as_deref(),
            since.as_deref(),
            json,
        )?,
        Commands::Profile { command } => handle_profile_command(command)?,
        Commands::Config { command } => handle_config_command(&command)?,
        Commands::Remote { command } => handle_remote_command(command)?,
        Commands::Push {
            remote,
            branch,
            set_upstream,
            dry_run,
            timeout,
        } => tokio::runtime::Runtime::new()?.block_on(handle_push_command(
            remote,
            branch,
            set_upstream,
            dry_run,
            timeout,
        ))?,
        Commands::Pull {
            remote,
            branch,
            dry_run,
            timeout,
        } => tokio::runtime::Runtime::new()?
            .block_on(handle_pull_command(remote, branch, dry_run, timeout))?,
        Commands::Status {
            profile,
            conflicts,
            json,
        } => handle_status_command(profile.as_deref(), conflicts, json)?,
        Commands::Maintain { command } => return handle_maintain_command(command),
        Commands::Check { profile } => handle_check_command(profile.as_deref(), cli.quiet)?,
        Commands::Doctor => handle_doctor_command()?,
        Commands::Completion { shell } => handle_completion_command(&shell),
    }

    Ok(())
}
