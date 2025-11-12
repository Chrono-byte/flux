mod commands;
mod config;
mod file_manager;
mod services;
mod types;
mod utils;

#[cfg(test)]
mod tests;

use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;
use commands::{
    add_backup_to_repo, apply_config, check_status, cleanup_backups, compare_states,
    display_backups, display_discrepancies, display_preview, display_status, display_validation,
    find_discrepancies, list_backups, migrate_files, restore_backup, validate_config,
};
use config::profile::{create_profile, get_profile_files, list_profiles, switch_profile};
use config::{Config, EnvironmentConfig};
use file_manager::{add_file, backup_all_files, remove_file, sync_files};
use services::git;
use services::{
    add_remote, commit_changes, detect_changes, init_repo, list_remotes, pull_from_remote,
    push_to_remote, remove_remote, set_remote_url, show_git_status, stage_changes,
};
use utils::prompt::{prompt_commit_message, prompt_yes_no};
use utils::{DotfilesError, DryRun, Result, logging};

#[derive(Parser)]
#[command(name = "flux")]
#[command(about = "A tool to manage dotfiles with symlink-based sync")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

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
    },
    /// Remove a file from tracking
    #[command(visible_alias = "rm")]
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
    #[command(visible_alias = "ls-files")]
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
    /// Profile management
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
    /// Configuration management operations
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Backup and restore operations
    Backup {
        #[command(subcommand)]
        command: BackupCommands,
    },
    /// Manage remote repositories
    Remote {
        #[command(subcommand)]
        command: RemoteCommands,
    },
    /// Push changes to remote repository
    Push {
        /// Remote name (default: origin or config default_remote)
        #[arg(long)]
        remote: Option<String>,
        /// Branch name (default: current HEAD or config default_branch)
        #[arg(long)]
        branch: Option<String>,
        /// Set upstream after push
        #[arg(long)]
        set_upstream: bool,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
        /// Timeout in seconds (default: 60 or config push_timeout)
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Pull changes from remote repository
    Pull {
        /// Remote name (default: origin or config default_remote)
        #[arg(long)]
        remote: Option<String>,
        /// Branch name (default: current HEAD or config default_branch)
        #[arg(long)]
        branch: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
        /// Timeout in seconds (default: 60 or config push_timeout)
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Show repository and file sync status
    Status {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Maintenance and repair operations
    Maintain {
        #[command(subcommand)]
        command: MaintainCommands,
    },
    /// Generate shell completions
    Completion {
        /// Shell type (zsh, bash, fish, etc.)
        shell: String,
    },
}

#[derive(Subcommand)]
enum BackupCommands {
    /// Backup all currently tracked files
    Create {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Restore files from backup
    Restore {
        /// Backup index, 'latest', or 'list' to show backups
        #[arg(default_value = "list")]
        backup: String,
        /// Specific file to restore (optional, restores all if not specified)
        #[arg(long)]
        file: Option<String>,
        /// Skip confirmation prompts (auto-confirm)
        #[arg(long)]
        yes: bool,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Add files from a backup to the repository and stage them
    Add {
        /// Backup index, 'latest', or 'list' to show backups
        #[arg(default_value = "latest")]
        backup: String,
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Commit staged changes in the repository
    Commit {
        /// Commit message (optional, will prompt if not provided)
        #[arg(long)]
        message: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean up old backups based on retention policy
    Cleanup {
        /// Number of recent backups to keep (default: 10)
        #[arg(long)]
        keep: Option<usize>,
        /// Keep all backups from the last N days (default: 7)
        #[arg(long)]
        days: Option<i64>,
        /// Minimum size threshold in bytes - backups smaller than this will be deleted (default: 1024 = 1KB)
        #[arg(long)]
        min_size: Option<u64>,
        /// Keep only the most recent N backups, ignoring age (overrides keep and days)
        #[arg(long)]
        only_keep: Option<usize>,
        /// Skip confirmation prompts (auto-confirm)
        #[arg(long)]
        yes: bool,
        /// Dry run mode (show what would be deleted)
        #[arg(long)]
        dry_run: bool,
    },
}

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
    // Load and validate environment configuration early
    let env_config = match EnvironmentConfig::load() {
        Ok(config) => {
            config.display_summary();
            config
        }
        Err(e) => {
            eprintln!(
                "{} Failed to load environment configuration: {}",
                "Error:".red().bold(),
                e
            );
            eprintln!(
                "{} Please check your environment variables",
                "Help:".yellow().bold()
            );
            std::process::exit(1);
        }
    };

    // Initialize logging system
    logging::init_logging();

    let cli = Cli::parse();

    if let Err(e) = run(cli, env_config) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn handle_backup_command(command: BackupCommands) -> Result<()> {
    match command {
        BackupCommands::Create { profile, dry_run } => {
            let config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();
            backup_all_files(&config, profile.as_deref(), &mut dry_run_tracker, dry_run)?;
            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        BackupCommands::Restore {
            backup,
            file,
            yes,
            dry_run,
        } => {
            let config = Config::load()?;
            let backups = list_backups(&config)?;

            if backups.is_empty() {
                println!("{}", "No backups available.".yellow());
                return Ok(());
            }

            // If no backup specified, show list and let user choose
            let selected_backup = if backup == "latest" && file.is_none() {
                display_backups(&backups);
                if !yes && !prompt_yes_no("Restore from latest backup?")? {
                    println!("{}", "Restore cancelled.".yellow());
                    return Ok(());
                }
                &backups[0]
            } else if backup == "latest" {
                &backups[0]
            } else if backup == "list" {
                display_backups(&backups);
                return Ok(());
            } else {
                let index: usize = backup.parse().map_err(|_| {
                    DotfilesError::Path(
                        "Invalid backup index. Use 'latest', 'list', or a number".to_string(),
                    )
                })?;
                if index == 0 || index > backups.len() {
                    return Err(DotfilesError::Path(format!(
                        "Backup index out of range (1-{})",
                        backups.len()
                    )));
                }
                &backups[index - 1]
            };

            let mut dry_run_tracker = DryRun::new();

            if let Some(target_file) = file {
                let target_path = std::path::Path::new(&target_file);
                if !dry_run
                    && !yes
                    && !prompt_yes_no(&format!("Restore {} from backup?", target_file))?
                {
                    println!("{}", "Restore cancelled.".yellow());
                    return Ok(());
                }
                restore_backup(selected_backup, target_path, &mut dry_run_tracker, dry_run)?;
                if dry_run {
                    println!("  [DRY RUN] Would restore {}", target_file);
                } else {
                    println!("{} Restored {}", "✓".green(), target_file);
                }
            } else {
                // Restore all files from backup
                if !dry_run
                    && !yes
                    && !prompt_yes_no(&format!(
                        "Restore all {} file(s) from backup {}?",
                        selected_backup.files.len(),
                        selected_backup.timestamp.format("%Y-%m-%d %H:%M:%S")
                    ))?
                {
                    println!("{}", "Restore cancelled.".yellow());
                    return Ok(());
                }

                let home = dirs::home_dir().ok_or_else(|| {
                    DotfilesError::Config("Could not find home directory".to_string())
                })?;

                for backup_file in &selected_backup.files {
                    if let Ok(relative) = backup_file.strip_prefix(&selected_backup.path) {
                        let target = home.join(relative);
                        restore_backup(selected_backup, &target, &mut dry_run_tracker, dry_run)?;
                    }
                }
                if dry_run {
                    dry_run_tracker.display_summary();
                } else {
                    println!("{} Restored all files from backup", "✓".green());
                }
            }
        }
        BackupCommands::Add {
            backup,
            profile,
            dry_run,
        } => {
            let config = Config::load()?;
            let backups = list_backups(&config)?;

            if backups.is_empty() {
                println!("{}", "No backups available.".yellow());
                return Ok(());
            }

            // Select backup
            let selected_backup = if backup == "latest" {
                &backups[0]
            } else if backup == "list" {
                display_backups(&backups);
                return Ok(());
            } else {
                let index: usize = backup.parse().map_err(|_| {
                    DotfilesError::Path(
                        "Invalid backup index. Use 'latest', 'list', or a number".to_string(),
                    )
                })?;
                if index == 0 || index > backups.len() {
                    return Err(DotfilesError::Path(format!(
                        "Backup index out of range (1-{})",
                        backups.len()
                    )));
                }
                &backups[index - 1]
            };

            let mut dry_run_tracker = DryRun::new();
            add_backup_to_repo(
                selected_backup,
                &config,
                profile.as_deref(),
                &mut dry_run_tracker,
                dry_run,
            )?;
            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        BackupCommands::Commit { message, dry_run } => {
            let config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();

            let repo_path = config.get_repo_path()?;
            let repo = git::init_repo(&repo_path)?;
            let changes = git::detect_changes(&repo)?;

            if changes.is_empty() {
                println!("{} No changes to commit.", "⊘".yellow());
                return Ok(());
            }

            let commit_message = if let Some(msg) = message {
                msg
            } else {
                prompt_commit_message(&changes)?
            };

            git::stage_changes(&repo, &changes, &mut dry_run_tracker, dry_run)?;
            git::commit_changes(&repo, &commit_message, &mut dry_run_tracker, dry_run)?;

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        BackupCommands::Cleanup {
            keep,
            days,
            min_size,
            only_keep,
            yes,
            dry_run,
        } => {
            let config = Config::load()?;
            cleanup_backups(&config, keep, days, min_size, only_keep, yes, dry_run)?;
        }
    }
    Ok(())
}

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
            let mut dry_run_tracker = DryRun::new();

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
            let repo_path = config.get_repo_path()?;
            let gitignore_path = repo_path.join(".gitignore");

            let gitignore_content = r#"# OS files
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
"#;

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

fn run(cli: Cli, _env_config: EnvironmentConfig) -> Result<()> {
    // Note: env_config is validated at startup for early error detection.
    // It's now used for custom config file paths and git auth.
    match cli.command {
        Commands::Init { repo_path } => {
            let mut config = Config::load()?;
            if let Some(path) = repo_path {
                config.general.repo_path = path;
            }
            config.save(false)?;

            let repo_path = config.get_repo_path()?;
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
        }
        Commands::Add {
            tool,
            file,
            dest,
            profile,
            dry_run,
            from_repo,
        } => {
            let mut config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();
            let mut fs_manager =
                file_manager::FileSystemManager::new(&mut dry_run_tracker, dry_run);

            if from_repo {
                // File already exists in repo - just register it
                let repo_path = config.get_repo_path()?;
                let repo_file = repo_path.join(&tool).join(&file);

                if !repo_file.exists() {
                    return Err(DotfilesError::Path(format!(
                        "File does not exist in repo: {}",
                        repo_file.display()
                    )));
                }

                let dest_path = if let Some(dest) = dest {
                    std::path::Path::new(&dest).to_path_buf()
                } else {
                    // Default to same name in home
                    std::path::Path::new(&file).to_path_buf()
                };

                // Just add to config without copying
                let repo_relative = repo_file
                    .strip_prefix(&repo_path)
                    .map_err(|_| {
                        DotfilesError::Path("Could not compute repo relative path".to_string())
                    })?
                    .to_string_lossy()
                    .to_string();

                config.add_file_to_tool(&tool, &repo_relative, &dest_path, profile.as_deref())?;

                if !dry_run {
                    config.save(false)?;
                    println!(
                        "{} Registered {} from repo (no copy needed)",
                        "✓".green(),
                        repo_file.display()
                    );
                } else {
                    println!(
                        "  [DRY RUN] Would register {} from repo",
                        repo_file.display()
                    );
                }
            } else {
                // Normal flow: copy file to repo
                let source_path = std::path::Path::new(&file);
                if !source_path.exists() {
                    return Err(DotfilesError::Path(format!(
                        "Source file does not exist: {}",
                        file
                    )));
                }

                let dest_path = if let Some(dest) = dest {
                    std::path::Path::new(&dest).to_path_buf()
                } else {
                    // Use source path relative to home
                    let home = dirs::home_dir().ok_or_else(|| {
                        DotfilesError::Config("Could not find home directory".to_string())
                    })?;
                    source_path
                        .strip_prefix(&home)
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|_| source_path.to_path_buf())
                };

                add_file(
                    &mut config,
                    &tool,
                    source_path,
                    &dest_path,
                    profile.as_deref(),
                    &mut fs_manager,
                )?;
            }

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        Commands::Commit {
            profile,
            message,
            dry_run,
        } => {
            let config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();

            sync_files(&config, profile.as_deref(), &mut dry_run_tracker, dry_run)?;

            if dry_run {
                dry_run_tracker.display_summary();
            } else {
                // Auto-commit changes
                let repo_path = config.get_repo_path()?;
                let repo = init_repo(&repo_path)?;
                let changes = detect_changes(&repo)?;

                if !changes.is_empty() {
                    let commit_message = if let Some(msg) = message {
                        msg
                    } else {
                        prompt_commit_message(&changes)?
                    };
                    stage_changes(&repo, &changes, &mut dry_run_tracker, dry_run)?;
                    commit_changes(&repo, &commit_message, &mut dry_run_tracker, dry_run)?;
                }
            }
        }
        Commands::Rm {
            tool,
            file,
            dry_run,
        } => {
            let mut config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();
            let mut fs_manager =
                file_manager::FileSystemManager::new(&mut dry_run_tracker, dry_run);
            remove_file(&mut config, &tool, &file, &mut fs_manager)?;

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        Commands::LsFiles { profile } => {
            let config = Config::load()?;
            let files = config.get_tracked_files(profile.as_deref())?;

            println!("\n{}", "Tracked files:".bold().cyan());
            for file in files {
                println!(
                    "  {} -> {}",
                    file.repo_path.display(),
                    file.dest_path.display()
                );
            }
        }
        Commands::Apply {
            profile,
            dry_run,
            yes,
            description,
            force,
        } => {
            let config = Config::load()?;

            if dry_run {
                // In dry-run mode, just show preview
                let diff = compare_states(&config, profile.as_deref(), force)?;
                display_preview(&diff);
            } else {
                use crate::commands::ApplyOptions;
                apply_config(ApplyOptions {
                    config: &config,
                    profile: profile.as_deref(),
                    dry_run,
                    yes,
                    description: description.as_deref(),
                    force,
                })?;
            }
        }
        Commands::Profile { command } => {
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
                    create_profile(&mut config, &name)?;
                }
                ProfileCommands::Switch { name } => {
                    switch_profile(&mut config, &name)?;
                }
            }
        }
        Commands::Config { command } => match command {
            ConfigCommands::Sync { dry_run } => {
                Config::sync_xdg_to_repo(dry_run)?;
            }
            ConfigCommands::Format { dry_run } => {
                let xdg_config = Config::get_xdg_config_path()?;

                if !xdg_config.exists() {
                    return Err(DotfilesError::Config(
                        "XDG config does not exist. Nothing to format.".to_string(),
                    ));
                }

                if dry_run {
                    println!(
                        "{} [DRY RUN] Would format and organize {}",
                        "⊘".yellow(),
                        xdg_config.display()
                    );
                    return Ok(());
                }

                // Load config (will load from XDG if it exists)
                let config = Config::load()?;

                // Save it back (will save to XDG and format it)
                config.save(true)?;

                println!(
                    "{} Formatted and organized config: {}",
                    "✓".green(),
                    xdg_config.display()
                );
            }
        },
        Commands::Backup { command } => {
            return handle_backup_command(command);
        }
        Commands::Remote { command } => {
            let config = Config::load()?;
            let repo_path = config.get_repo_path()?;
            let repo = init_repo(&repo_path)?;
            let mut dry_run_tracker = DryRun::new();

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
        }
        Commands::Push {
            remote,
            branch,
            set_upstream,
            dry_run,
            timeout,
        } => {
            let config = Config::load()?;
            let repo_path = config.get_repo_path()?;
            let repo = init_repo(&repo_path)?;
            let mut dry_run_tracker = DryRun::new();

            // Resolve remote: --remote flag > config default_remote > "origin"
            let resolved_remote = remote
                .or_else(|| config.general.default_remote.clone())
                .unwrap_or_else(|| "origin".to_string());

            // Resolve branch: --branch flag > current HEAD > config default_branch > "main"
            let resolved_branch = branch
                .or_else(|| git::get_current_branch(&repo).ok())
                .or_else(|| config.general.default_branch.clone())
                .unwrap_or_else(|| "main".to_string());

            // Resolve timeout: --timeout flag > config push_timeout > 60 seconds
            let resolved_timeout = timeout.or(config.general.push_timeout).unwrap_or(60);

            push_to_remote(
                &repo,
                &resolved_remote,
                &resolved_branch,
                set_upstream,
                resolved_timeout,
                &mut dry_run_tracker,
                dry_run,
            )?;

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        Commands::Pull {
            remote,
            branch,
            dry_run,
            timeout,
        } => {
            let config = Config::load()?;
            let repo_path = config.get_repo_path()?;
            let repo = init_repo(&repo_path)?;
            let mut dry_run_tracker = DryRun::new();

            // Resolve remote: --remote flag > config default_remote > "origin"
            let resolved_remote = remote
                .or_else(|| config.general.default_remote.clone())
                .unwrap_or_else(|| "origin".to_string());

            // Resolve branch: --branch flag > current HEAD > config default_branch > "main"
            let resolved_branch = branch
                .or_else(|| git::get_current_branch(&repo).ok())
                .or_else(|| config.general.default_branch.clone())
                .unwrap_or_else(|| "main".to_string());

            // Resolve timeout: --timeout flag > config push_timeout > 60 seconds
            let resolved_timeout = timeout.or(config.general.push_timeout).unwrap_or(60);

            pull_from_remote(
                &repo,
                &resolved_remote,
                &resolved_branch,
                resolved_timeout,
                &mut dry_run_tracker,
                dry_run,
            )?;

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        Commands::Status { profile } => {
            let config = Config::load()?;
            let repo_path = config.get_repo_path()?;

            // Show git repository status
            if let Ok(repo) = init_repo(&repo_path) {
                show_git_status(&repo)?;
            }

            // Show file sync status
            let reports = check_status(&config, profile.as_deref())?;
            display_status(&reports);
        }
        Commands::Maintain { command } => {
            return handle_maintain_command(command);
        }
        Commands::Completion { shell } => {
            use clap_complete::{generate, shells::Zsh};
            let mut cmd = Cli::command();
            match shell.to_lowercase().as_str() {
                "zsh" => {
                    generate(Zsh, &mut cmd, "flux", &mut std::io::stdout());
                }
                _ => {
                    eprintln!(
                        "{} Unsupported shell: {}. Supported shells: zsh",
                        "Error:".red().bold(),
                        shell
                    );
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
