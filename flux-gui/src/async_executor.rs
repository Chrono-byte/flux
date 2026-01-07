//! Async executor for running blocking flux library operations.

use flux::commands::status::{FileStatus, check_status};
use flux::config::Config;
use flux::services::git::{
    add_remote as git_add_remote, get_current_branch, init_repo,
    remove_remote as git_remove_remote, set_remote_url as git_set_remote_url,
};
use flux::services::history::{HistoryEvent, read_events};
use flux::utils::error::Result as FluxResult;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task;

#[derive(Debug)]
pub struct AsyncTask {
    pub operation: Operation,
    pub reply: mpsc::Sender<TaskResult>,
}

#[derive(Debug, Clone)]
pub enum Operation {
    LoadConfig,
    SaveConfig {
        config: Config,
    },
    CheckStatus {
        profile: Option<String>,
    },
    ListProfiles,
    CreateProfile {
        name: String,
    },
    SwitchProfile {
        name: String,
    },
    GetTrackedFiles {
        profile: Option<String>,
    },
    GetGitBranch,
    ListRemotes,
    AddRemote {
        name: String,
        url: String,
    },
    RemoveRemote {
        name: String,
    },
    SetRemoteUrl {
        name: String,
        url: String,
    },
    Push {
        remote: String,
        branch: String,
    },
    Pull {
        remote: String,
        branch: String,
    },
    ReadHistory {
        limit: Option<usize>,
    },
    GetGitStatus,
    AddFile {
        tool: String,
        source_path: PathBuf,
        dest_path: PathBuf,
        profile: Option<String>,
    },
    RemoveFile {
        tool: String,
        file_name: String,
    },
    ApplyConfig {
        profile: Option<String>,
        dry_run: bool,
        force: bool,
    },
    ValidateConfigContent {
        content: String,
    },
}

#[derive(Debug)]
pub enum TaskResult {
    Config(FluxResult<Config>),
    ConfigSaved(FluxResult<()>),
    StatusReports(FluxResult<Vec<GuiStatusReport>>),
    Profiles(FluxResult<Vec<String>>),
    ProfileCreated(FluxResult<()>),
    ProfileSwitched(FluxResult<()>),
    TrackedFiles(FluxResult<Vec<TrackedFileInfo>>),
    GitBranch(FluxResult<String>),
    Remotes(FluxResult<Vec<GuiRemoteInfo>>),
    RemoteAdded(FluxResult<()>),
    RemoteRemoved(FluxResult<()>),
    RemoteUrlSet(FluxResult<()>),
    Pushed(FluxResult<()>),
    Pulled(FluxResult<()>),
    History(FluxResult<Vec<HistoryEvent>>),
    GitStatus(FluxResult<GitStatusInfo>),
    FileAdded(FluxResult<()>),
    FileRemoved(FluxResult<()>),
    ConfigApplied(FluxResult<Vec<String>>),
    ConfigValidated(FluxResult<Vec<String>>),
}

#[derive(Debug, Clone)]
pub struct GuiStatusReport {
    pub tool: String,
    pub repo_path: String,
    pub dest_path: String,
    pub profile: Option<String>,
    pub status: GuiFileStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiFileStatus {
    Synced,
    MissingSymlink,
    BrokenSymlink,
    OutOfSync,
    MissingRepo,
}

impl From<FileStatus> for GuiFileStatus {
    fn from(status: FileStatus) -> Self {
        match status {
            FileStatus::Synced => GuiFileStatus::Synced,
            FileStatus::MissingSymlink => GuiFileStatus::MissingSymlink,
            FileStatus::BrokenSymlink => GuiFileStatus::BrokenSymlink,
            FileStatus::OutOfSync => GuiFileStatus::OutOfSync,
            FileStatus::MissingRepo => GuiFileStatus::MissingRepo,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackedFileInfo {
    pub tool: String,
    pub repo_path: PathBuf,
    pub dest_path: PathBuf,
    pub profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GuiRemoteInfo {
    pub name: String,
    pub url: String,
    pub branch: String,
}

#[derive(Debug, Clone)]
pub struct GitStatusInfo {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub has_remote: bool,
}

pub struct FluxExecutor {
    pub tx: mpsc::Sender<AsyncTask>,
    _handle: tokio::task::JoinHandle<()>,
}

impl FluxExecutor {
    #[must_use]
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel::<AsyncTask>(32);

        let handle = tokio::spawn(async move {
            while let Some(task) = rx.recv().await {
                let result = Self::execute_task(task.operation).await;
                let _ = task.reply.send(result).await;
            }
        });

        Self {
            tx,
            _handle: handle,
        }
    }

    /// Execute an operation asynchronously
    pub async fn execute_task(operation: Operation) -> TaskResult {
        match operation {
            Operation::LoadConfig => {
                let result = task::spawn_blocking(|| {
                    Config::load()
                        .map_err(|e| flux::utils::error::DotfilesError::Config(e.to_string()))
                })
                .await;
                TaskResult::Config(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::SaveConfig { config } => {
                let result = task::spawn_blocking(move || config.save(false)).await;
                TaskResult::ConfigSaved(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::CheckStatus { profile } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|config| {
                        let profile_ref = profile.as_deref();
                        let reports = check_status(&config, profile_ref)?;
                        let gui_reports: Vec<GuiStatusReport> = reports
                            .into_iter()
                            .map(|r| GuiStatusReport {
                                tool: r.file.tool,
                                repo_path: r.file.repo_path.to_string_lossy().to_string(),
                                dest_path: r.file.dest_path.to_string_lossy().to_string(),
                                profile: r.file.profile,
                                status: r.status.into(),
                                message: r.message,
                            })
                            .collect();
                        Ok(gui_reports)
                    })
                })
                .await;
                TaskResult::StatusReports(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::ListProfiles => {
                let result = task::spawn_blocking(|| {
                    Config::load().and_then(|config| flux::config::profile::list_profiles(&config))
                })
                .await;
                TaskResult::Profiles(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::CreateProfile { name } => {
                let result = task::spawn_blocking(move || {
                    Config::load()
                        .and_then(|config| flux::config::profile::create_profile(&config, &name))
                })
                .await;
                TaskResult::ProfileCreated(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::SwitchProfile { name } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|mut config| {
                        flux::config::profile::switch_profile(&mut config, &name)
                    })
                })
                .await;
                TaskResult::ProfileSwitched(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::GetTrackedFiles { profile } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|config| {
                        let profile_ref = profile.as_deref();
                        let files = config.get_tracked_files(profile_ref)?;
                        let file_infos: Vec<TrackedFileInfo> = files
                            .into_iter()
                            .map(|f| TrackedFileInfo {
                                tool: f.tool,
                                repo_path: f.repo_path,
                                dest_path: f.dest_path,
                                profile: f.profile,
                            })
                            .collect();
                        Ok(file_infos)
                    })
                })
                .await;
                TaskResult::TrackedFiles(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::GetGitBranch => {
                let result = task::spawn_blocking(|| {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        init_repo(&repo_path).and_then(|repo| get_current_branch(&repo))
                    })
                })
                .await;
                TaskResult::GitBranch(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::ListRemotes => {
                let result = task::spawn_blocking(|| {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        let repo = init_repo(&repo_path)?;
                        let branch = get_current_branch(&repo).unwrap_or_default();

                        // Access the underlying git2 repository to list remotes
                        let git_repo = repo.inner();
                        let remotes = git_repo.remotes()?;

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
                    })
                })
                .await;
                TaskResult::Remotes(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::AddRemote { name, url } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        let repo = init_repo(&repo_path)?;
                        let mut dry_run = flux::utils::dry_run::DryRun::new();
                        git_add_remote(&repo, &name, &url, &mut dry_run, false)
                    })
                })
                .await;
                TaskResult::RemoteAdded(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::RemoveRemote { name } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        let repo = init_repo(&repo_path)?;
                        let mut dry_run = flux::utils::dry_run::DryRun::new();
                        git_remove_remote(&repo, &name, &mut dry_run, false)
                    })
                })
                .await;
                TaskResult::RemoteRemoved(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::SetRemoteUrl { name, url } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        let repo = init_repo(&repo_path)?;
                        let mut dry_run = flux::utils::dry_run::DryRun::new();
                        git_set_remote_url(&repo, &name, &url, &mut dry_run, false)
                    })
                })
                .await;
                TaskResult::RemoteUrlSet(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            // Note: push_to_remote and pull_from_remote are async functions
            // They require tokio runtime context, so we handle them separately
            Operation::Push {
                remote: _,
                branch: _,
            } => TaskResult::Pushed(Err(flux::utils::error::DotfilesError::Config(
                "Push not yet implemented in GUI".to_string(),
            ))),

            Operation::Pull {
                remote: _,
                branch: _,
            } => TaskResult::Pulled(Err(flux::utils::error::DotfilesError::Config(
                "Pull not yet implemented in GUI".to_string(),
            ))),

            Operation::ReadHistory { limit } => {
                let result = task::spawn_blocking(move || read_events(limit)).await;
                TaskResult::History(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::GetGitStatus => {
                let result = task::spawn_blocking(|| {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        let repo = init_repo(&repo_path)?;
                        let branch = get_current_branch(&repo).unwrap_or_default();
                        let has_remote =
                            repo.inner().remotes().map(|r| !r.is_empty()).unwrap_or(false);

                        Ok(GitStatusInfo {
                            branch,
                            ahead: 0,
                            behind: 0,
                            has_remote,
                        })
                    })
                })
                .await;
                TaskResult::GitStatus(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::AddFile {
                tool,
                source_path,
                dest_path,
                profile,
            } => {
                let result = task::spawn_blocking(move || {
                    let mut config = Config::load()?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    let mut fs_manager =
                        flux::file_manager::FileSystemManager::new(&mut dry_run, false);
                    flux::file_manager::add_file(
                        &mut config,
                        &tool,
                        &source_path,
                        &dest_path,
                        profile.as_deref(),
                        &mut fs_manager,
                    )
                })
                .await;
                TaskResult::FileAdded(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::RemoveFile { tool, file_name } => {
                let result = task::spawn_blocking(move || {
                    let mut config = Config::load()?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    let mut fs_manager =
                        flux::file_manager::FileSystemManager::new(&mut dry_run, false);
                    flux::file_manager::remove_file(&mut config, &tool, &file_name, &mut fs_manager)
                })
                .await;
                TaskResult::FileRemoved(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::ApplyConfig {
                profile,
                dry_run: apply_dry_run,
                force,
            } => {
                let result = task::spawn_blocking(move || {
                    let config = Config::load()?;
                    let profile_ref = profile.as_deref();

                    // Use the ApplyOptions builder
                    let options = flux::commands::apply::ApplyOptions {
                        config: &config,
                        profile: profile_ref,
                        dry_run: apply_dry_run,
                        yes: false,
                        description: None,
                        force,
                    };

                    flux::commands::apply::apply_config(&options)?;
                    // Return empty list since apply_config returns () and doesn't return file list
                    Ok::<Vec<String>, flux::utils::error::DotfilesError>(Vec::new())
                })
                .await;
                TaskResult::ConfigApplied(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::ValidateConfigContent { content } => {
                let result = task::spawn_blocking(move || {
                    // Try to parse the content as TOML
                    match toml::from_str::<Config>(&content) {
                        Ok(_) => Ok(Vec::new()),
                        Err(e) => Ok(vec![e.to_string()]),
                    }
                })
                .await;
                TaskResult::ConfigValidated(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }
        }
    }

    /// Execute a task synchronously (blocking, used at startup)
    pub fn execute_task_sync(operation: Operation) -> TaskResult {
        match operation {
            Operation::LoadConfig => {
                let result = Config::load()
                    .map_err(|e| flux::utils::error::DotfilesError::Config(e.to_string()));
                TaskResult::Config(result)
            }

            Operation::SaveConfig { config } => {
                let result = config.save(false);
                TaskResult::ConfigSaved(result)
            }

            Operation::CheckStatus { profile } => {
                let result = Config::load().and_then(|config| {
                    let profile_ref = profile.as_deref();
                    let reports = check_status(&config, profile_ref)?;
                    let gui_reports: Vec<GuiStatusReport> = reports
                        .into_iter()
                        .map(|r| GuiStatusReport {
                            tool: r.file.tool,
                            repo_path: r.file.repo_path.to_string_lossy().to_string(),
                            dest_path: r.file.dest_path.to_string_lossy().to_string(),
                            profile: r.file.profile,
                            status: r.status.into(),
                            message: r.message,
                        })
                        .collect();
                    Ok(gui_reports)
                });
                TaskResult::StatusReports(result)
            }

            Operation::ListProfiles => {
                let result =
                    Config::load().and_then(|config| flux::config::profile::list_profiles(&config));
                TaskResult::Profiles(result)
            }

            Operation::CreateProfile { name } => {
                let result = Config::load()
                    .and_then(|config| flux::config::profile::create_profile(&config, &name));
                TaskResult::ProfileCreated(result)
            }

            Operation::SwitchProfile { name } => {
                let result = Config::load().and_then(|mut config| {
                    flux::config::profile::switch_profile(&mut config, &name)
                });
                TaskResult::ProfileSwitched(result)
            }

            Operation::GetTrackedFiles { profile } => {
                let result = Config::load().and_then(|config| {
                    let profile_ref = profile.as_deref();
                    let files = config.get_tracked_files(profile_ref)?;
                    let file_infos: Vec<TrackedFileInfo> = files
                        .into_iter()
                        .map(|f| TrackedFileInfo {
                            tool: f.tool,
                            repo_path: f.repo_path,
                            dest_path: f.dest_path,
                            profile: f.profile,
                        })
                        .collect();
                    Ok(file_infos)
                });
                TaskResult::TrackedFiles(result)
            }

            Operation::GetGitBranch => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    init_repo(&repo_path).and_then(|repo| get_current_branch(&repo))
                });
                TaskResult::GitBranch(result)
            }

            Operation::ListRemotes => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let branch = get_current_branch(&repo).unwrap_or_default();

                    // Access the underlying git2 repository to list remotes
                    let git_repo = repo.inner();
                    let remotes = git_repo.remotes()?;

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
                });
                TaskResult::Remotes(result)
            }

            Operation::AddRemote { name, url } => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    git_add_remote(&repo, &name, &url, &mut dry_run, false)
                });
                TaskResult::RemoteAdded(result)
            }

            Operation::RemoveRemote { name } => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    git_remove_remote(&repo, &name, &mut dry_run, false)
                });
                TaskResult::RemoteRemoved(result)
            }

            Operation::SetRemoteUrl { name, url } => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    git_set_remote_url(&repo, &name, &url, &mut dry_run, false)
                });
                TaskResult::RemoteUrlSet(result)
            }

            Operation::Push {
                remote: _,
                branch: _,
            } => TaskResult::Pushed(Err(flux::utils::error::DotfilesError::Config(
                "Push not yet implemented".to_string(),
            ))),

            Operation::Pull {
                remote: _,
                branch: _,
            } => TaskResult::Pulled(Err(flux::utils::error::DotfilesError::Config(
                "Pull not yet implemented".to_string(),
            ))),

            Operation::ReadHistory { limit } => {
                let result = read_events(limit);
                TaskResult::History(result)
            }

            Operation::GetGitStatus => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let branch = get_current_branch(&repo).unwrap_or_default();
                    let has_remote = repo.inner().remotes().map(|r| !r.is_empty()).unwrap_or(false);

                    Ok(GitStatusInfo {
                        branch,
                        ahead: 0,
                        behind: 0,
                        has_remote,
                    })
                });
                TaskResult::GitStatus(result)
            }

            Operation::AddFile {
                tool,
                source_path,
                dest_path,
                profile,
            } => {
                let result = {
                    let mut config = Config::load()?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    let mut fs_manager =
                        flux::file_manager::FileSystemManager::new(&mut dry_run, false);
                    flux::file_manager::add_file(
                        &mut config,
                        &tool,
                        &source_path,
                        &dest_path,
                        profile.as_deref(),
                        &mut fs_manager,
                    )
                };
                TaskResult::FileAdded(result)
            }

            Operation::RemoveFile { tool, file_name } => {
                let result = {
                    let mut config = Config::load()?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    let mut fs_manager =
                        flux::file_manager::FileSystemManager::new(&mut dry_run, false);
                    flux::file_manager::remove_file(&mut config, &tool, &file_name, &mut fs_manager)
                };
                TaskResult::FileRemoved(result)
            }

            Operation::ApplyConfig {
                profile,
                dry_run: apply_dry_run,
                force,
            } => {
                let result = {
                    let config = Config::load()?;
                    let profile_ref = profile.as_deref();
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    let mut fs_manager =
                        flux::file_manager::FileSystemManager::new(&mut dry_run, apply_dry_run);
                    let files = flux::commands::apply::apply_config(
                        &config,
                        profile_ref,
                        force,
                        &mut fs_manager,
                    )?;
                    Ok(files.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
                };
                TaskResult::ConfigApplied(result)
            }

            Operation::ValidateConfigContent { content } => {
                let result = match toml::from_str::<Config>(&content) {
                    Ok(_) => Ok(Vec::new()),
                    Err(e) => Ok(vec![e.to_string()]),
                };
                TaskResult::ConfigValidated(result)
            }
        }
    }
}

impl Default for FluxExecutor {
    fn default() -> Self {
        Self::new()
    }
}
