mod browser;
mod config;
mod dry_run;
mod error;
mod file_manager;
mod git;
mod profile;
mod prompt;
mod restore;
mod status;
mod types;
mod validate;

use clap::{Parser, Subcommand};
use colored::Colorize;
use config::Config;
use dry_run::DryRun;
use error::Result;
use file_manager::{add_file, remove_file, sync_files};
use git::{commit_changes, detect_changes, init_repo, stage_changes};
use profile::{create_profile, get_profile_files, list_profiles, switch_profile};
use prompt::{prompt_commit_message, prompt_yes_no};
use browser::{detect_firefox_profiles, detect_zen_profiles, get_browser_profile_files};
use status::{check_status, display_status};
use restore::{display_backups, list_backups, restore_backup};
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
    },
    /// Auto-detect and add browser profiles (Firefox and Zen)
    AddBrowser {
        /// Browser name (firefox, zen, or all)
        #[arg(default_value = "all")]
        browser: String,
    },
    /// Sync tracked files
    Sync {
        /// Profile name (default: current profile)
        #[arg(long)]
        profile: Option<String>,
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
    /// Restore files from backup
    Restore {
        /// Backup index, 'latest', or 'list' to show backups
        #[arg(default_value = "list")]
        backup: String,
        /// Specific file to restore (optional, restores all if not specified)
        #[arg(long)]
        file: Option<String>,
    },
    /// Validate configuration integrity
    Validate,
    /// Remove a file from tracking
    Remove {
        /// Tool name
        tool: String,
        /// File name in repository
        file: String,
    },
    /// Profile management
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
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

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init { repo_path } => {
            let mut config = Config::load()?;
            if let Some(path) = repo_path {
                config.general.repo_path = path;
            }
            config.save()?;

            let repo_path = config.get_repo_path()?;
            std::fs::create_dir_all(&repo_path)?;

            let repo = init_repo(&repo_path)?;
            println!("{} Initialized repository at {}", "✓".green(), repo_path.display());
            println!("   Git repository: {}", if repo.path().exists() { "initialized" } else { "not initialized" });
        }
        Commands::Add {
            tool,
            file,
            dest,
            profile,
        } => {
            let mut config = Config::load()?;
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
                let home = dirs::home_dir()
                    .ok_or_else(|| error::DotfilesError::Config("Could not find home directory".to_string()))?;
                source_path.strip_prefix(&home)
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|_| source_path.to_path_buf())
            };

            add_file(&mut config, &tool, source_path, &dest_path, profile.as_deref())?;
        }
        Commands::AddBrowser { browser } => {
            let mut config = Config::load()?;
            let mut added_count = 0;

            if browser == "all" || browser == "firefox" {
                let firefox_profiles = detect_firefox_profiles()?;
                for profile in firefox_profiles {
                    let files = get_browser_profile_files(&profile);
                    for (source_path, dest_str) in files {
                        if source_path.exists() {
                            // For directories, use the directory name; for files, use filename
                            let repo_relative = if source_path.is_dir() {
                                source_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string()
                            } else {
                                source_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string()
                            };
                            
                            config.add_file_to_tool("firefox", &repo_relative, &dest_str, None)?;
                            added_count += 1;
                            println!(
                                "{} Added Firefox: {} -> {}",
                                "✓".green(),
                                source_path.display(),
                                dest_str
                            );
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
                            // For directories, use the directory name; for files, use filename
                            let repo_relative = if source_path.is_dir() {
                                source_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string()
                            } else {
                                source_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string()
                            };
                            
                            config.add_file_to_tool("zen", &repo_relative, &dest_str, None)?;
                            added_count += 1;
                            println!(
                                "{} Added Zen: {} -> {}",
                                "✓".green(),
                                source_path.display(),
                                dest_str
                            );
                        }
                    }
                }
            }

            if added_count > 0 {
                config.save()?;
                println!("\n{} Added {} browser file(s) to tracking", "✓".green(), added_count);
            } else {
                println!("{} No browser profiles found", "⊘".yellow());
            }
        }
        Commands::Sync { profile, dry_run } => {
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
                    let commit_message = prompt_commit_message(&changes)?;
                    stage_changes(&repo, &changes, &mut dry_run_tracker, dry_run)?;
                    commit_changes(&repo, &commit_message, &mut dry_run_tracker, dry_run)?;
                }
            }
        }
        Commands::List { profile } => {
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
        Commands::Status { profile } => {
            let config = Config::load()?;
            let reports = check_status(&config, profile.as_deref())?;
            display_status(&reports);
        }
        Commands::Restore { backup, file } => {
            let config = Config::load()?;
            let backups = list_backups(&config)?;
            
            if backups.is_empty() {
                println!("{}", "No backups available.".yellow());
                return Ok(());
            }
            
            // If no backup specified, show list and let user choose
            let selected_backup = if backup == "latest" && file.is_none() {
                display_backups(&backups);
                if !prompt_yes_no("Restore from latest backup?")? {
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
                    error::DotfilesError::Path("Invalid backup index. Use 'latest', 'list', or a number".to_string())
                })?;
                if index == 0 || index > backups.len() {
                    return Err(error::DotfilesError::Path(format!(
                        "Backup index out of range (1-{})",
                        backups.len()
                    )));
                }
                &backups[index - 1]
            };
            
            if let Some(target_file) = file {
                let target_path = std::path::Path::new(&target_file);
                if !prompt_yes_no(&format!("Restore {} from backup?", target_file))? {
                    println!("{}", "Restore cancelled.".yellow());
                    return Ok(());
                }
                restore_backup(selected_backup, target_path)?;
                println!("{} Restored {}", "✓".green(), target_file);
            } else {
                // Restore all files from backup
                if !prompt_yes_no(&format!(
                    "Restore all {} file(s) from backup {}?",
                    selected_backup.files.len(),
                    selected_backup.timestamp.format("%Y-%m-%d %H:%M:%S")
                ))? {
                    println!("{}", "Restore cancelled.".yellow());
                    return Ok(());
                }
                
                let home = dirs::home_dir().ok_or_else(|| {
                    error::DotfilesError::Config("Could not find home directory".to_string())
                })?;
                
                for backup_file in &selected_backup.files {
                    if let Ok(relative) = backup_file.strip_prefix(&selected_backup.path) {
                        let target = home.join(relative);
                        restore_backup(selected_backup, &target)?;
                    }
                }
                println!("{} Restored all files from backup", "✓".green());
            }
        }
        Commands::Validate => {
            let config = Config::load()?;
            let report = validate_config(&config)?;
            display_validation(&report);
            if !report.is_valid {
                std::process::exit(1);
            }
        }
        Commands::Remove { tool, file } => {
            let mut config = Config::load()?;
            let mut dry_run = DryRun::new();
            remove_file(&mut config, &tool, &file, &mut dry_run)?;
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
                            if file_count > 0 { file_count.to_string().cyan() } else { "0".yellow() }
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
