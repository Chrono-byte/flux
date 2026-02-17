use flux::commands::status as status_cmd;
use flux::config::Config;
use serde::{Deserialize, Serialize};
use shellexpand::tilde;

/// Convert FileStatus enum to frontend-expected string format
fn status_to_string(status: status_cmd::FileStatus) -> String {
    match status {
        status_cmd::FileStatus::Synced => "synced".to_string(),
        status_cmd::FileStatus::MissingSymlink => "missingSymlink".to_string(),
        status_cmd::FileStatus::BrokenSymlink => "brokenSymlink".to_string(),
        status_cmd::FileStatus::OutOfSync => "outOfSync".to_string(),
        status_cmd::FileStatus::MissingRepo => "missingRepo".to_string(),
    }
}

/// Configuration response for the frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub repo_path: Option<String>,
    pub current_profile: Option<String>,
    pub current_profile_name: Option<String>,
}

/// File report response for the frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct FileReportResponse {
    pub tool: Option<String>,
    pub repo_path: Option<String>,
    pub dest_path: Option<String>,
    pub profile: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub size: Option<u64>,
    pub modified: Option<String>,
}

/// Status summary response
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusSummaryResponse {
    pub total: usize,
    pub synced: usize,
    pub out_of_sync: usize,
    pub missing: usize,
}

/// Load configuration
#[tauri::command]
fn load_config() -> Result<ConfigResponse, String> {
    eprintln!("[DEBUG] load_config called");

    let config = Config::load().map_err(|e| {
        let err_msg = format!("Failed to load config: {}", e);
        eprintln!("[ERROR] {}", err_msg);
        err_msg
    })?;

    eprintln!(
        "[DEBUG] loaded config: repo_path={}, current_profile={}",
        config.general.repo_path, config.general.current_profile
    );

    Ok(ConfigResponse {
        repo_path: Some(config.general.repo_path),
        current_profile: Some(config.general.current_profile.clone()),
        current_profile_name: Some(config.general.current_profile),
    })
}

/// Check sync status of all files
#[tauri::command]
fn check_status() -> Result<Vec<FileReportResponse>, String> {
    eprintln!("[DEBUG] check_status called");
    let config = Config::load().map_err(|e| {
        eprintln!("[ERROR] load config in check_status: {}", e);
        format!("Failed to load config: {}", e)
    })?;

    // Get all status reports for all profiles
    let status_reports = status_cmd::check_status(&config, None).map_err(|e| {
        eprintln!("[ERROR] check_status: {}", e);
        format!("Failed to check status: {}", e)
    })?;

    eprintln!("[DEBUG] check_status: got {} reports", status_reports.len());

    let reports: Vec<_> = status_reports
        .iter()
        .map(|report| {
            FileReportResponse {
                tool: Some(report.file.tool.clone()),
                repo_path: Some(report.file.repo_path.display().to_string()),
                dest_path: Some(report.file.dest_path.display().to_string()),
                profile: report.file.profile.clone(),
                status: status_to_string(report.status),
                message: Some(report.message.clone()),
                size: report.file.repo_path.metadata().ok().map(|m| m.len()),
                modified: None, // TODO: add modification time if needed
            }
        })
        .collect();

    eprintln!("[DEBUG] check_status: returning {} reports", reports.len());
    Ok(reports)
}

/// Get sync status summary
#[tauri::command]
fn get_status_summary() -> Result<StatusSummaryResponse, String> {
    let config = Config::load().map_err(|e| format!("Failed to load config: {}", e))?;

    // Get all status reports
    let status_reports = status_cmd::check_status(&config, None)
        .map_err(|e| format!("Failed to check status: {}", e))?;

    let total = status_reports.len();
    let synced = status_reports
        .iter()
        .filter(|r| matches!(r.status, status_cmd::FileStatus::Synced))
        .count();
    let out_of_sync = status_reports
        .iter()
        .filter(|r| matches!(r.status, status_cmd::FileStatus::OutOfSync))
        .count();
    let missing = total - synced - out_of_sync;

    Ok(StatusSummaryResponse {
        total,
        synced,
        out_of_sync,
        missing,
    })
}

/// List available profiles
#[tauri::command]
fn list_profiles() -> Result<Vec<String>, String> {
    let config = Config::load().map_err(|e| format!("Failed to load config: {}", e))?;

    // Extract unique profile names from all files
    let mut profiles = std::collections::HashSet::new();
    profiles.insert(config.general.current_profile);

    for tool_config in config.tools.values() {
        for file_entry in &tool_config.files {
            if let Some(profile) = &file_entry.profile {
                profiles.insert(profile.clone());
            }
        }
    }

    let mut profiles_vec: Vec<_> = profiles.into_iter().collect();
    profiles_vec.sort();

    Ok(profiles_vec)
}

/// List git remotes (simplified - returns remote names)
#[tauri::command]
fn list_remotes() -> Result<Vec<String>, String> {
    let config = Config::load().map_err(|e| format!("Failed to load config: {}", e))?;

    let repo_path_str = &config.general.repo_path;
    let repo_path_expanded = tilde(repo_path_str).into_owned();
    let repo_path = std::path::PathBuf::from(repo_path_expanded);

    // Try to open git repo and get remotes
    match flux::services::git::init_repo(&repo_path) {
        Ok(_git_repo) => {
            // Git2 API doesn't expose remotes directly in the public functions,
            // so we return an empty list for now. This would need deeper integration.
            Ok(vec![])
        }
        Err(_) => Ok(vec![]), // Return empty list if not a git repo
    }
}

/// Get current git branch
#[tauri::command]
fn get_git_branch() -> Result<String, String> {
    let config = Config::load().map_err(|e| format!("Failed to load config: {}", e))?;

    let repo_path_str = &config.general.repo_path;
    let repo_path_expanded = tilde(repo_path_str).into_owned();
    let repo_path = std::path::PathBuf::from(repo_path_expanded);

    match flux::services::git::init_repo(&repo_path) {
        Ok(git_repo) => match git_repo.get_current_branch() {
            Ok(branch) => Ok(branch),
            Err(_) => Ok("unknown".to_string()),
        },
        Err(_) => Ok("unknown".to_string()),
    }
}

/// Read command history
#[tauri::command]
fn read_history() -> Result<Vec<(String, String)>, String> {
    match flux::services::history::read_events(Some(50)) {
        Ok(events) => {
            let history: Vec<_> = events
                .into_iter()
                .map(|event| (event.timestamp.to_string(), event.command))
                .collect();
            Ok(history)
        }
        Err(_) => Ok(Vec::new()), // Return empty list if history unavailable
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_config,
            check_status,
            get_status_summary,
            list_profiles,
            list_remotes,
            get_git_branch,
            read_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
