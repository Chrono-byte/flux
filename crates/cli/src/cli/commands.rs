//! CLI command handling for the Flux dotfiles manager.

use atty::{Stream, is};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use clap::CommandFactory;
use colored::Colorize;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

use super::args::{Cli, Commands, MaintainCommands, ProfileCommands, RemoteCommands};
use flux_lib::{
    commands::apply::StateDiff,
    commands::doctor::HealthReport,
    commands::validate::ValidationReport,
    commands::{
        apply_config, check_status, compare_states, display_diffs, display_health_report,
        display_preview, display_status, display_validation, format_diffs, migrate_files,
        run_health_checks, validate_config,
    },
    config::Config,
    config::profile::{create_profile, get_profile_files, list_profiles, switch_profile},
    file_manager::{FileSystemManager, add_file, remove_file, sync_files},
    services::git,
    services::{
        add_remote, commit_changes, detect_changes, init_repo, list_remotes, pull_from_remote,
        push_to_remote, read_events, remove_remote, set_remote_url, show_git_status, stage_changes,
    },
    utils::home_override,
    utils::prompt::prompt_commit_message,
    utils::{DotfilesError, DryRun, Result},
};

/// Handle the maintain command.
pub(super) fn handle_maintain_command(command: MaintainCommands) -> Result<()> {
    match command {
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
pub(super) fn handle_init_command(repo_path: Option<String>) -> Result<()> {
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
pub(super) fn handle_add_command(
    tool: &str,
    file: &str,
    dest: Option<&str>,
    profile: Option<&str>,
    dry_run: bool,
    from_repo: bool,
) -> Result<()> {
    let mut config = Config::load()?;
    let mut dry_run_tracker = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, dry_run);

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

        if repo_file.is_dir() {
            config.add_folder_to_tool(tool, &repo_relative, &dest_path, profile);
        } else {
            config.add_file_to_tool(tool, &repo_relative, &dest_path, profile);
        }

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
            let home = home_override::home_dir().ok_or_else(|| {
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
pub(super) fn handle_commit_command(
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
        use flux_lib::services::hooks::execute_hooks;
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
                use flux_lib::services::hooks::execute_hooks;
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
pub(super) fn handle_rm_command(tool: &str, file: &str, dry_run: bool) -> Result<()> {
    let mut config = Config::load()?;
    let mut dry_run_tracker = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, dry_run);
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
pub(super) fn handle_ls_files_command(profile: Option<&str>) -> Result<()> {
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

#[derive(Debug, Clone, Copy)]
/// Represents a toggleable boolean value
pub enum Toggle {
    /// Represents a disabled state
    Disabled,
    /// Represents an enabled state
    Enabled,
}

impl Toggle {
    #[must_use]
    /// Converts the toggle value to a boolean.
    ///
    /// # Returns
    ///
    /// `true` if the toggle is `Enabled`, `false` if it is `Disabled`.
    pub const fn as_bool(self) -> bool {
        matches!(self, Toggle::Enabled)
    }

    #[must_use]
    /// Create a `Toggle` from a `bool`.
    ///
    /// `true` -> `Toggle::Enabled`, `false` -> `Toggle::Disabled`.
    pub const fn from_bool(bool: bool) -> Self {
        if bool {
            Toggle::Enabled
        } else {
            Toggle::Disabled
        }
    }
}

/// Handle the apply command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be applied or if the configuration cannot be loaded.
pub(super) fn handle_apply_command(
    profile: Option<&str>,
    dry_run: Toggle,
    diff: Toggle,
    yes: Toggle,
    description: Option<&str>,
    force: Toggle,
) -> Result<()> {
    let config = Config::load()?;

    // Normal apply command
    if dry_run.as_bool() {
        // In dry-run mode, only show preview without applying changes
        let state_diff = compare_states(&config, profile, force.as_bool())?;
        display_preview(&state_diff);
        if diff.as_bool() && !state_diff.files_to_sync.is_empty() {
            display_diffs(&state_diff);
        }
    } else {
        use flux_lib::commands::ApplyOptions;
        // apply_config now handles history recording internally
        apply_config(&ApplyOptions {
            config: &config,
            profile,
            dry_run: dry_run.as_bool(),
            yes: yes.as_bool(),
            description,
            force: force.as_bool(),
        })?;
    }
    Ok(())
}

/// Handle the diff command.
pub(super) fn handle_diff_command(
    profile: Option<&str>,
    force: bool,
    name_only: bool,
) -> Result<()> {
    let config = Config::load()?;
    let state_diff = compare_states(&config, profile, force)?;

    if state_diff.is_empty() {
        println!(
            "{} System is already in sync with configuration",
            "✓".green()
        );
        return Ok(());
    }

    display_preview(&state_diff);

    if name_only {
        print_changed_files(&config, &state_diff);
        return Ok(());
    }

    let diff_output = format_diffs(&state_diff);
    if diff_output.trim().is_empty() {
        return Ok(());
    }

    if should_use_pager() {
        open_diff_pager(&diff_output)?;
    } else {
        print!("{diff_output}");
    }

    Ok(())
}

/// Print the relative paths of changed files without a pager.
fn print_changed_files(config: &Config, state_diff: &StateDiff) {
    let repo_path = config.get_repo_path();
    for file in &state_diff.files_to_sync {
        if let Ok(rel_path) = file.repo_path.strip_prefix(&repo_path) {
            println!("{}", rel_path.display());
        } else {
            println!("{}", file.repo_path.display());
        }
    }
}

/// Determine whether to use a pager for diff output.
fn should_use_pager() -> bool {
    is(Stream::Stdout)
}

/// Open a pager for the given diff output.
fn open_diff_pager(output: &str) -> Result<()> {
    let pager_cmd = env::var("PAGER").unwrap_or_else(|_| "less -R".to_string());
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(pager_cmd)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| DotfilesError::Config(format!("Failed to open pager: {err}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(output.as_bytes())?;
    }

    let _ = child
        .wait()
        .map_err(|err| DotfilesError::Config(format!("Pager terminated unexpectedly: {err}")))?;

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
fn parse_since_time(since_str: &str) -> Result<DateTime<Utc>> {
    since_str.parse::<i64>().map_or_else(
        |_| {
            DateTime::parse_from_rfc3339(since_str).map_or_else(
                |_| {
                    NaiveDateTime::parse_from_str(since_str, "%Y-%m-%d %H:%M:%S")
                        .map_or_else(
                            |_| {
                                // Try just date
                                NaiveDate::parse_from_str(since_str, "%Y-%m-%d")
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
                |dt| Ok(dt.with_timezone(&Utc)),
            )
        },
        |hours| {
            // Interpret as hours ago
            Ok(Utc::now() - Duration::hours(hours))
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
pub(super) fn handle_history_command(
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
pub(super) fn handle_profile_command(command: ProfileCommands) -> Result<()> {
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
pub(super) fn handle_remote_command(command: RemoteCommands) -> Result<()> {
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
pub(super) async fn handle_push_command(
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
pub(super) async fn handle_pull_command(
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
pub(super) fn handle_status_command(
    profile: Option<&str>,
    conflicts: bool,
    json: bool,
) -> Result<()> {
    let config = Config::load()?;
    let repo_path = config.get_repo_path();

    if conflicts {
        // Show only conflicted files
        use flux_lib::commands::status::check_conflicts;
        let conflicted = check_conflicts(&config, profile)?;

        if json {
            use flux_lib::commands::status::JsonStatusReport;
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
        use flux_lib::commands::status::JsonStatusReport;
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

/// Handle the doctor command.
///
/// # Parameters
///
/// * `profile`: Optional profile name for state comparison.
///
/// # Returns
///
/// Returns the result of the doctor command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded or if any checks fail.
pub(super) fn handle_doctor_command(profile: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    // Validation + state diff (formerly `flux check`)
    let report = validate_config(&config)?;
    let diff = compare_states(&config, profile, false)?;

    // Health checks
    let health_report = run_health_checks(&config)?;

    display_doctor_report(&report, &diff, &health_report);

    let mut exit_code = 0;
    if !report.is_valid {
        exit_code |= 1;
    }
    if !diff.is_empty() {
        exit_code |= 2;
    }
    if !health_report.is_healthy() {
        exit_code |= 4;
    }

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

/// Display the doctor report, including validation issues, state diff, and health checks.
fn display_doctor_report(
    report: &ValidationReport,
    diff: &StateDiff,
    health_report: &HealthReport,
) {
    println!("\n{} Doctor Report", "→".cyan().bold());
    println!("{}", "=".repeat(60).cyan());

    if report.is_valid && diff.is_empty() {
        println!(
            "{} Configuration is valid and system is in sync",
            "✓".green()
        );
    } else {
        if !report.is_valid {
            display_validation(report);
        }

        if !diff.is_empty() {
            println!(
                "\n{} System is not in sync with configuration",
                "⚠".yellow()
            );
            display_preview(diff);
        }
    }

    display_health_report(health_report);
}

/// Execute the command internally.
/// Internal command dispatcher: forward a `Commands` variant to its respective handler.
///
/// This helper centralizes the match for `Commands` but is intentionally private
/// (not part of the public API) so it does not require top-level documentation
/// for every lint. It also uses `Toggle::from_bool` to keep the match arms concise.
fn execute_internal(cmd: Commands) -> Result<()> {
    match cmd {
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
            Toggle::from_bool(dry_run),
            Toggle::from_bool(diff),
            Toggle::from_bool(yes),
            description.as_deref(),
            Toggle::from_bool(force),
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
        Commands::Diff {
            profile,
            force,
            name_only,
        } => handle_diff_command(profile.as_deref(), force, name_only)?,
        Commands::Maintain { command } => handle_maintain_command(command)?,
        Commands::Doctor { profile } => handle_doctor_command(profile.as_deref())?,
        Commands::Completion { shell } => handle_completion_command(&shell),
    }

    Ok(())
}

impl Commands {
    /// Execute the command.
    pub fn execute(self) -> Result<()> {
        execute_internal(self)
    }
}

/// Handle the completion command.
pub(super) fn handle_completion_command(shell: &str) {
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
