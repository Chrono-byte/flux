//! Tauri command handlers for Flux operations.
//!
//! These commands bridge the frontend JavaScript with the Flux library operations.

use crate::state::{
    AppState, FileStatus, GitStatusInfo, GuiRemoteInfo, GuiStatusReport, SyncSummary,
    TrackedFileInfo,
};
use flux::commands::status::{FileStatus as FluxFileStatus, check_status as flux_check_status};
use flux::config::Config;
use flux::services::git::{
    add_remote as git_add_remote, get_current_branch, init_repo,
    remove_remote as git_remove_remote, set_remote_url as git_set_remote_url,
};
use flux::services::history::read_events;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

/// Result type for Tauri commands
type CommandResult<T> = Result<T, String>;

/// Convert a flux error to a string for the frontend
fn map_flux_error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Serializable config info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigInfo {
    /// Current profile name
    pub current_profile: String,
    /// Repository path
    pub repo_path: String,
    /// Config content as TOML string
    pub content: String,
}

/// Load the Flux configuration
#[tauri::command]
pub async fn load_config(state: State<'_, Mutex<AppState>>) -> CommandResult<ConfigInfo> {
    let config = Config::load().map_err(map_flux_error)?;

    let current_profile = config.general.current_profile.clone();
    let repo_path = config.get_repo_path().display().to_string();
    let content = toml::to_string_pretty(&config).map_err(map_flux_error)?;

    // Update state
    if let Ok(mut app_state) = state.lock() {
        app_state.current_profile.clone_from(&current_profile);
        app_state.repo_path = config.get_repo_path();
        app_state.config_loaded = true;
    }

    Ok(ConfigInfo {
        current_profile,
        repo_path,
        content,
    })
}

/// Save the configuration content
#[tauri::command]
pub async fn save_config(content: String) -> CommandResult<()> {
    // Parse the content to validate it
    let config: Config = toml::from_str(&content).map_err(map_flux_error)?;
    config.save(false).map_err(map_flux_error)
}

/// Check the sync status of tracked files
#[tauri::command]
pub async fn check_status(
    profile: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> CommandResult<Vec<GuiStatusReport>> {
    let config = Config::load().map_err(map_flux_error)?;
    let profile_ref = profile.as_deref();
    let reports = flux_check_status(&config, profile_ref).map_err(map_flux_error)?;

    let gui_reports: Vec<GuiStatusReport> = reports
        .into_iter()
        .map(|r| GuiStatusReport {
            tool: r.file.tool,
            repo_path: r.file.repo_path.to_string_lossy().to_string(),
            dest_path: r.file.dest_path.to_string_lossy().to_string(),
            profile: r.file.profile,
            status: convert_file_status(r.status),
            message: r.message,
        })
        .collect();

    // Update sync summary in state
    if let Ok(mut app_state) = state.lock() {
        let mut summary = SyncSummary::default();
        for report in &gui_reports {
            summary.total += 1;
            match report.status {
                FileStatus::Synced => summary.synced += 1,
                FileStatus::OutOfSync => summary.out_of_sync += 1,
                FileStatus::MissingSymlink | FileStatus::BrokenSymlink => summary.broken += 1,
                FileStatus::MissingRepo => summary.missing += 1,
            }
        }
        app_state.sync_summary = summary;
    }

    Ok(gui_reports)
}

/// Convert flux file status to GUI file status
fn convert_file_status(status: FluxFileStatus) -> FileStatus {
    match status {
        FluxFileStatus::Synced => FileStatus::Synced,
        FluxFileStatus::MissingSymlink => FileStatus::MissingSymlink,
        FluxFileStatus::BrokenSymlink => FileStatus::BrokenSymlink,
        FluxFileStatus::OutOfSync => FileStatus::OutOfSync,
        FluxFileStatus::MissingRepo => FileStatus::MissingRepo,
    }
}

/// List available profiles
#[tauri::command]
pub async fn list_profiles() -> CommandResult<Vec<String>> {
    let config = Config::load().map_err(map_flux_error)?;
    flux::config::profile::list_profiles(&config).map_err(map_flux_error)
}

/// Create a new profile
#[tauri::command]
pub async fn create_profile(name: String) -> CommandResult<()> {
    let config = Config::load().map_err(map_flux_error)?;
    flux::config::profile::create_profile(&config, &name).map_err(map_flux_error)
}

/// Switch to a different profile
#[tauri::command]
pub async fn switch_profile(name: String, state: State<'_, Mutex<AppState>>) -> CommandResult<()> {
    let mut config = Config::load().map_err(map_flux_error)?;
    flux::config::profile::switch_profile(&mut config, &name).map_err(map_flux_error)?;

    // Update state
    if let Ok(mut app_state) = state.lock() {
        app_state.current_profile = name;
    }

    Ok(())
}

/// Get tracked files
#[tauri::command]
pub async fn get_tracked_files(profile: Option<String>) -> CommandResult<Vec<TrackedFileInfo>> {
    let config = Config::load().map_err(map_flux_error)?;
    let profile_ref = profile.as_deref();
    let files = config.get_tracked_files(profile_ref).map_err(map_flux_error)?;

    Ok(files
        .into_iter()
        .map(|f| TrackedFileInfo {
            tool: f.tool,
            repo_path: f.repo_path.to_string_lossy().to_string(),
            dest_path: f.dest_path.to_string_lossy().to_string(),
            profile: f.profile,
        })
        .collect())
}

/// List git remotes
#[tauri::command]
pub async fn list_remotes() -> CommandResult<Vec<GuiRemoteInfo>> {
    let config = Config::load().map_err(map_flux_error)?;
    let repo_path = config.get_repo_path();
    let repo = init_repo(&repo_path).map_err(map_flux_error)?;
    let branch = get_current_branch(&repo).unwrap_or_default();

    let git_repo = repo.inner();
    let remotes = git_repo.remotes().map_err(map_flux_error)?;

    let remote_infos: Vec<GuiRemoteInfo> = remotes
        .iter()
        .filter_map(|name| {
            name.as_ref().and_then(|n| {
                git_repo.find_remote(n).ok().map(|r| GuiRemoteInfo {
                    name: n.to_string(),
                    url: r.url().unwrap_or("(unknown)").to_string(),
                    branch: branch.clone(),
                })
            })
        })
        .collect();

    Ok(remote_infos)
}

/// Add a new git remote
#[tauri::command]
pub async fn add_remote(name: String, url: String) -> CommandResult<()> {
    let config = Config::load().map_err(map_flux_error)?;
    let repo_path = config.get_repo_path();
    let repo = init_repo(&repo_path).map_err(map_flux_error)?;
    let mut dry_run = flux::utils::dry_run::DryRun::new();
    git_add_remote(&repo, &name, &url, &mut dry_run, false).map_err(map_flux_error)
}

/// Remove a git remote
#[tauri::command]
pub async fn remove_remote(name: String) -> CommandResult<()> {
    let config = Config::load().map_err(map_flux_error)?;
    let repo_path = config.get_repo_path();
    let repo = init_repo(&repo_path).map_err(map_flux_error)?;
    let mut dry_run = flux::utils::dry_run::DryRun::new();
    git_remove_remote(&repo, &name, &mut dry_run, false).map_err(map_flux_error)
}

/// Set remote URL
#[tauri::command]
pub async fn set_remote_url(name: String, url: String) -> CommandResult<()> {
    let config = Config::load().map_err(map_flux_error)?;
    let repo_path = config.get_repo_path();
    let repo = init_repo(&repo_path).map_err(map_flux_error)?;
    let mut dry_run = flux::utils::dry_run::DryRun::new();
    git_set_remote_url(&repo, &name, &url, &mut dry_run, false).map_err(map_flux_error)
}

/// Push to remote
#[tauri::command]
pub async fn push_to_remote(_remote: String, _branch: String) -> CommandResult<()> {
    // Push requires credentials handling which is complex
    // For now, return an informative error
    Err("Git push is not yet supported in the GUI. Please use the CLI: flux push".to_string())
}

/// Pull from remote
#[tauri::command]
pub async fn pull_from_remote(_remote: String, _branch: String) -> CommandResult<()> {
    // Pull requires credentials handling which is complex
    // For now, return an informative error
    Err("Git pull is not yet supported in the GUI. Please use the CLI: flux pull".to_string())
}

/// Serializable history event for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEventInfo {
    /// Timestamp of the event
    pub timestamp: String,
    /// Command that was executed
    pub command: String,
    /// Whether it was a dry run
    pub dry_run: bool,
    /// Profile that was active
    pub profile: Option<String>,
}

/// Read operation history
#[tauri::command]
pub async fn read_history(limit: Option<usize>) -> CommandResult<Vec<HistoryEventInfo>> {
    let events = read_events(limit).map_err(map_flux_error)?;

    Ok(events
        .into_iter()
        .map(|e| HistoryEventInfo {
            timestamp: e.timestamp.to_rfc3339(),
            command: e.command,
            dry_run: e.dry_run,
            profile: e.profile,
        })
        .collect())
}

/// Get git repository status
#[tauri::command]
pub async fn get_git_status(state: State<'_, Mutex<AppState>>) -> CommandResult<GitStatusInfo> {
    let config = Config::load().map_err(map_flux_error)?;
    let repo_path = config.get_repo_path();

    match init_repo(&repo_path) {
        Ok(repo) => {
            let branch = get_current_branch(&repo).unwrap_or_default();
            let has_remote = repo.inner().remotes().map(|r| !r.is_empty()).unwrap_or(false);

            let status = GitStatusInfo {
                branch,
                ahead: 0,
                behind: 0,
                has_remote,
                is_repo: true,
            };

            // Update state
            if let Ok(mut app_state) = state.lock() {
                app_state.git_status = status.clone();
            }

            Ok(status)
        }
        Err(_) => Ok(GitStatusInfo {
            branch: String::new(),
            ahead: 0,
            behind: 0,
            has_remote: false,
            is_repo: false,
        }),
    }
}

/// Add a file to tracking
#[tauri::command]
pub async fn add_file(
    tool: String,
    source_path: String,
    dest_path: String,
    profile: Option<String>,
) -> CommandResult<()> {
    let mut config = Config::load().map_err(map_flux_error)?;
    let mut dry_run = flux::utils::dry_run::DryRun::new();
    let mut fs_manager = flux::file_manager::FileSystemManager::new(&mut dry_run, false);

    flux::file_manager::add_file(
        &mut config,
        &tool,
        &PathBuf::from(source_path),
        &PathBuf::from(dest_path),
        profile.as_deref(),
        &mut fs_manager,
    )
    .map_err(map_flux_error)
}

/// Remove a file from tracking
#[tauri::command]
pub async fn remove_file(tool: String, file_name: String) -> CommandResult<()> {
    let mut config = Config::load().map_err(map_flux_error)?;
    let mut dry_run = flux::utils::dry_run::DryRun::new();
    let mut fs_manager = flux::file_manager::FileSystemManager::new(&mut dry_run, false);

    flux::file_manager::remove_file(&mut config, &tool, &file_name, &mut fs_manager)
        .map_err(map_flux_error)
}

/// Apply configuration (sync files)
#[tauri::command]
pub async fn apply_config(
    profile: Option<String>,
    dry_run: bool,
    force: bool,
) -> CommandResult<Vec<String>> {
    let config = Config::load().map_err(map_flux_error)?;
    let profile_ref = profile.as_deref();

    let options = flux::commands::apply::ApplyOptions {
        config: &config,
        profile: profile_ref,
        dry_run,
        yes: false,
        description: None,
        force,
    };

    flux::commands::apply::apply_config(&options).map_err(map_flux_error)?;

    // Return empty list since apply_config returns () and doesn't return file list
    Ok(Vec::new())
}

/// Validate configuration content
#[tauri::command]
pub async fn validate_config(content: String) -> CommandResult<Vec<String>> {
    match toml::from_str::<Config>(&content) {
        Ok(_) => Ok(Vec::new()),
        Err(e) => Ok(vec![e.to_string()]),
    }
}

/// Get the current application state
#[tauri::command]
pub async fn get_app_state(
    state: State<'_, Mutex<AppState>>,
) -> CommandResult<crate::state::AppStateSnapshot> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    Ok(app_state.snapshot())
}

/// Set dry run mode
#[tauri::command]
pub async fn set_dry_run_mode(
    enabled: bool,
    state: State<'_, Mutex<AppState>>,
) -> CommandResult<()> {
    let mut app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.dry_run_mode = enabled;
    Ok(())
}
