mod browser;
mod config;
mod dry_run;
mod error;
mod file_manager;
mod git;
mod logging;
mod migrate;
mod profile;
mod prompt;
mod restore;
mod security;
mod status;
mod types;
mod untracked;
mod validate;

#[cfg(test)]
mod tests;

use browser::{
    detect_alacritty_configs, detect_firefox_profiles, detect_starship_configs,
    detect_zen_profiles, get_browser_profile_files,
};
use clap::{Parser, Subcommand};
use colored::Colorize;
use config::Config;
use dry_run::DryRun;
use error::Result;
use file_manager::{add_file, backup_all_files, remove_file, sync_files};
use git::{
    add_remote, commit_changes, detect_changes, init_repo, list_remotes, push_to_remote,
    remove_remote, set_remote_url, stage_changes,
};
use migrate::migrate_files;
use profile::{create_profile, get_profile_files, list_profiles, switch_profile};
use prompt::{prompt_commit_message, prompt_yes_no};
use restore::{add_backup_to_repo, cleanup_backups, display_backups, list_backups, restore_backup};
use status::{check_status, display_status};
use untracked::{display_discrepancies, find_discrepancies};
use validate::{display_validation, validate_config};

#[derive(Parser)]
#[command(name = "dotfiles-manager")]
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
    /// File management operations
    File {
        #[command(subcommand)]
        command: FileCommands,
    },
    /// Backup and restore operations
    Backup {
        #[command(subcommand)]
        command: BackupCommands,
    },
    /// Maintenance and repair operations
    Maintain {
        #[command(subcommand)]
        command: MaintainCommands,
    },
    /// Profile management
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
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
    },
}

#[derive(Subcommand)]
enum FileCommands {
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
    },
    /// Auto-detect and add browser profiles (Firefox and Zen) or terminal/prompt configs (Alacritty, Starship)
    AddBrowser {
        /// Browser/terminal/prompt name (firefox, zen, alacritty, starship, or all)
        #[arg(default_value = "all")]
        browser: String,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove a file from tracking
    Remove {
        /// Tool name
        tool: String,
        /// File name in repository
        file: String,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Sync tracked files
    Sync {
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
    /// List tracked files
    List {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Show sync status of tracked files
    Status {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
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
    /// Clean up old backups based on retention policy
    Cleanup {
        /// Number of recent backups to keep (default: 10)
        #[arg(long)]
        keep: Option<usize>,
        /// Keep all backups from the last N days (default: 7)
        #[arg(long)]
        days: Option<i64>,
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
    /// Migrate files with discrepancies: copy current files to repo and create symlinks
    Migrate {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
    /// Validate configuration integrity
    Validate,
    /// Generate a .gitignore file for the repository
    Gitignore,
}

#[derive(Subcommand)]
enum ProfileCommands {
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
    /// List all profiles
    List,
}

#[derive(Subcommand)]
enum RemoteCommands {
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
    /// Remove a remote repository
    Remove {
        /// Remote name
        name: String,
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
    /// List all remotes
    List,
}

fn main() {
    // Initialize logging system
    logging::init_logging();

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn handle_file_command(command: FileCommands) -> Result<()> {
    match command {
        FileCommands::Add {
            tool,
            file,
            dest,
            profile,
            dry_run,
        } => {
            let mut config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();
            let mut fs_manager =
                file_manager::FileSystemManager::new(&mut dry_run_tracker, dry_run);

            let source_path = std::path::Path::new(&file);
            if !source_path.exists() {
                return Err(error::DotfilesError::Path(format!(
                    "Source file does not exist: {}",
                    file
                )));
            }

            let dest_path = if let Some(dest) = dest {
                std::path::Path::new(&dest).to_path_buf()
            } else {
                // Use source path relative to home
                let home = dirs::home_dir().ok_or_else(|| {
                    error::DotfilesError::Config("Could not find home directory".to_string())
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

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
        FileCommands::AddBrowser { browser, dry_run } => {
            let mut config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();
            let mut fs_manager =
                file_manager::FileSystemManager::new(&mut dry_run_tracker, dry_run);
            let mut added_count = 0;

            if browser == "all" || browser == "firefox" {
                let firefox_profiles = detect_firefox_profiles()?;
                for profile in firefox_profiles {
                    let files = get_browser_profile_files(&profile);
                    for (source_path, dest_str) in files {
                        if source_path.exists() {
                            let dest_path = std::path::Path::new(&dest_str);
                            // Use add_file() to properly copy files to repo and handle backups
                            add_file(
                                &mut config,
                                "firefox",
                                &source_path,
                                dest_path,
                                None,
                                &mut fs_manager,
                            )?;
                            added_count += 1;
                        }
                    }
                }
            }

            if browser == "all" || browser == "zen" {
                let zen_profiles = detect_zen_profiles()?;
                for profile in zen_profiles {
                    let files = get_browser_profile_files(&profile);
                    for (source_path, dest_str) in files {
                        if source_path.exists() {
                            let dest_path = std::path::Path::new(&dest_str);
                            // Use add_file() to properly copy files to repo and handle backups
                            add_file(
                                &mut config,
                                "zen",
                                &source_path,
                                dest_path,
                                None,
                                &mut fs_manager,
                            )?;
                            added_count += 1;
                        }
                    }
                }
            }

            if browser == "all" || browser == "alacritty" {
                let alacritty_configs = detect_alacritty_configs()?;
                for (source_path, dest_str) in alacritty_configs {
                    if source_path.exists() {
                        let dest_path = std::path::Path::new(&dest_str);
                        // Use add_file() to properly copy files to repo and handle backups
                        add_file(
                            &mut config,
                            "alacritty",
                            &source_path,
                            dest_path,
                            None,
                            &mut fs_manager,
                        )?;
                        added_count += 1;
                    }
                }
            }

            if browser == "all" || browser == "starship" {
                let starship_configs = detect_starship_configs()?;
                for (source_path, dest_str) in starship_configs {
                    if source_path.exists() {
                        let dest_path = std::path::Path::new(&dest_str);
                        // Use add_file() to properly copy files to repo and handle backups
                        add_file(
                            &mut config,
                            "starship",
                            &source_path,
                            dest_path,
                            None,
                            &mut fs_manager,
                        )?;
                        added_count += 1;
                    }
                }
            }

            if dry_run {
                dry_run_tracker.display_summary();
            } else if added_count > 0 {
                // config.save() is already called by add_file() for each file
                println!(
                    "\n{} Added {} file(s) to tracking",
                    "✓".green(),
                    added_count
                );
            } else {
                println!(
                    "{} No browser profiles or terminal configs found",
                    "⊘".yellow()
                );
            }
        }
        FileCommands::Remove {
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
        FileCommands::Sync {
            profile,
            message,
            dry_run,
        } => {
            let config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();

            // If dry_run is true, we'll track operations but not execute them
            // The is_empty() check in file_manager will determine execution

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
        FileCommands::List { profile } => {
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
        FileCommands::Status { profile } => {
            let config = Config::load()?;
            let reports = check_status(&config, profile.as_deref())?;
            display_status(&reports);
        }
    }
    Ok(())
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
                    error::DotfilesError::Path(
                        "Invalid backup index. Use 'latest', 'list', or a number".to_string(),
                    )
                })?;
                if index == 0 || index > backups.len() {
                    return Err(error::DotfilesError::Path(format!(
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
                    error::DotfilesError::Path(
                        "Invalid backup index. Use 'latest', 'list', or a number".to_string(),
                    )
                })?;
                if index == 0 || index > backups.len() {
                    return Err(error::DotfilesError::Path(format!(
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
                    error::DotfilesError::Config("Could not find home directory".to_string())
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
        BackupCommands::Cleanup {
            keep,
            days,
            dry_run,
        } => {
            let config = Config::load()?;
            cleanup_backups(&config, keep, days, dry_run)?;
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
        MaintainCommands::Migrate { profile, dry_run } => {
            let config = Config::load()?;
            let mut dry_run_tracker = DryRun::new();

            migrate_files(&config, profile.as_deref(), &mut dry_run_tracker, dry_run)?;

            if dry_run {
                dry_run_tracker.display_summary();
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

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::File { command } => {
            return handle_file_command(command);
        }
        Commands::Backup { command } => {
            return handle_backup_command(command);
        }
        Commands::Maintain { command } => {
            return handle_maintain_command(command);
        }
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
        Commands::Profile { command } => {
            let mut config = Config::load()?;
            match command {
                ProfileCommands::Create { name } => {
                    create_profile(&mut config, &name)?;
                }
                ProfileCommands::Switch { name } => {
                    switch_profile(&mut config, &name)?;
                }
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
            }
        }
        Commands::Remote { command } => {
            let config = Config::load()?;
            let repo_path = config.get_repo_path()?;
            let repo = init_repo(&repo_path)?;
            let mut dry_run_tracker = DryRun::new();

            match command {
                RemoteCommands::Add { name, url, dry_run } => {
                    add_remote(&repo, &name, &url, &mut dry_run_tracker, dry_run)?;
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
                RemoteCommands::SetUrl { name, url, dry_run } => {
                    set_remote_url(&repo, &name, &url, &mut dry_run_tracker, dry_run)?;
                    if dry_run {
                        dry_run_tracker.display_summary();
                    }
                }
                RemoteCommands::List => {
                    list_remotes(&repo)?;
                }
            }
        }
        Commands::Push {
            remote,
            branch,
            set_upstream,
            dry_run,
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

            push_to_remote(
                &repo,
                &resolved_remote,
                &resolved_branch,
                set_upstream,
                &mut dry_run_tracker,
                dry_run,
            )?;

            if dry_run {
                dry_run_tracker.display_summary();
            }
        }
    }

    Ok(())
}
