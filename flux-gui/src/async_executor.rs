//! Async executor for running blocking flux library operations.

use flux::commands::status::{FileStatus, check_status};
use flux::config::Config;
use flux::services::git::{
    add_remote as git_add_remote, get_current_branch, init_repo, pull_from_remote, push_to_remote,
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
                let config = std::sync::Arc::new(config);
                let result = task::spawn_blocking({
                    let config = config.clone();
                    move || config.save(false)
                })
                .await;
                TaskResult::ConfigSaved(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::CheckStatus { profile } => {
                let profile = profile;
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
                let profile = profile;
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
                        let remotes = repo.remotes()?;
                        let branch = get_current_branch(&repo).unwrap_or_default();

                        let remote_infos: Vec<GuiRemoteInfo> = remotes
                            .iter()
                            .filter_map(|name| {
                                name.as_ref().and_then(|n| {
                                    repo.find_remote(n).ok().map(|r| GuiRemoteInfo {
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

            Operation::Push { remote, branch } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        let repo = init_repo(&repo_path)?;
                        let mut dry_run = flux::utils::dry_run::DryRun::new();
                        let timeout = config.general.push_timeout.unwrap_or(60);
                        push_to_remote(&repo, &remote, &branch, false, timeout, &mut dry_run, false)
                    })
                })
                .await;
                TaskResult::Pushed(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::Pull { remote, branch } => {
                let result = task::spawn_blocking(move || {
                    Config::load().and_then(|config| {
                        let repo_path = config.get_repo_path();
                        let repo = init_repo(&repo_path)?;
                        let mut dry_run = flux::utils::dry_run::DryRun::new();
                        pull_from_remote(&repo, &remote, &branch, 60, &mut dry_run, false)
                    })
                })
                .await;
                TaskResult::Pulled(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

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
                        let has_remote = repo.remotes().map(|r| !r.is_empty()).unwrap_or(false);

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
                dry_run,
                force,
            } => {
                let result = task::spawn_blocking(move || {
                    let config = Config::load()?;
                    let options = flux::commands::apply::ApplyOptions {
                        config: &config,
                        profile: profile.as_deref(),
                        dry_run,
                        yes: true,
                        description: None,
                        force,
                    };
                    flux::commands::apply::apply_config(&options)?;
                    let tracked_files = config.get_tracked_files(profile.as_deref())?;
                    let applied_paths: Vec<String> = tracked_files
                        .iter()
                        .map(|f| f.dest_path.to_string_lossy().to_string())
                        .collect();
                    Ok(applied_paths)
                })
                .await;
                TaskResult::ConfigApplied(result.unwrap_or_else(|e| {
                    Err(flux::utils::error::DotfilesError::Config(e.to_string()))
                }))
            }

            Operation::ValidateConfigContent { content } => {
                let result =
                    task::spawn_blocking(move || match toml::from_str::<Config>(&content) {
                        Ok(config) => match flux::commands::validate::validate_config(&config) {
                            Ok(_) => Ok(vec![]),
                            Err(e) => Ok(vec![e.to_string()]),
                        },
                        Err(e) => Ok(vec![format!("TOML parse error: {}", e)]),
                    })
                    .await;
                TaskResult::ConfigValidated(
                    result.unwrap_or_else(|e| Ok(vec![format!("Validation error: {}", e)])),
                )
            }
        }
    }

    #[must_use]
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
                    let remotes = repo.remotes()?;
                    let branch = get_current_branch(&repo).unwrap_or_default();

                    let remote_infos: Vec<GuiRemoteInfo> = remotes
                        .iter()
                        .filter_map(|name| {
                            name.as_ref().and_then(|n| {
                                repo.find_remote(n).ok().map(|r| GuiRemoteInfo {
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

            Operation::Push { remote, branch } => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    push_to_remote(&repo, &remote, &branch, false, 30, &mut dry_run, false)
                });
                TaskResult::Pushed(result)
            }

            Operation::Pull { remote, branch } => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let mut dry_run = flux::utils::dry_run::DryRun::new();
                    pull_from_remote(&repo, &remote, &branch, 30, &mut dry_run, false)
                });
                TaskResult::Pulled(result)
            }

            Operation::ReadHistory { limit } => {
                let result = read_events(limit);
                TaskResult::History(result)
            }

            Operation::GetGitStatus => {
                let result = Config::load().and_then(|config| {
                    let repo_path = config.get_repo_path();
                    let repo = init_repo(&repo_path)?;
                    let branch = get_current_branch(&repo).unwrap_or_default();
                    let remotes = repo.remotes()?;
                    let has_remote = !remotes.is_empty();

                    // Get ahead/behind info (simplified for now)
                    let (ahead, behind) = (0, 0);

                    Ok(GitStatusInfo {
                        branch,
                        ahead,
                        behind,
                        has_remote,
                    })
                });
                TaskResult::GitStatus(result)
            }

            Operation::AddFile {
                tool: _,
                source_path: _,
                dest_path: _,
                profile: _,
            } => {
                // TODO: Implement file addition
                TaskResult::FileAdded(Err(flux::utils::error::DotfilesError::Config(
                    "Not implemented".to_string(),
                )))
            }

            Operation::RemoveFile {
                tool: _,
                file_name: _,
            } => {
                // TODO: Implement file removal
                TaskResult::FileRemoved(Err(flux::utils::error::DotfilesError::Config(
                    "Not implemented".to_string(),
                )))
            }

            Operation::ApplyConfig {
                profile,
                dry_run,
                force,
            } => {
                let result = Config::load().and_then(|config| {
                    let profile_ref = profile.as_deref();
                    if dry_run {
                        // For dry run, just return empty vec since we can't get paths without applying
                        Ok(vec![])
                    } else {
                        let options = flux::commands::apply::ApplyOptions {
                            config: &config,
                            profile: profile_ref,
                            dry_run,
                            yes: true, // Auto-confirm for GUI
                            description: None,
                            force,
                        };
                        flux::commands::apply::apply_config(&options)?;
                        // Return the paths that were applied
                        let diff =
                            flux::commands::apply::compare_states(&config, profile_ref, force)?;
                        let paths: Vec<String> = diff
                            .files_to_sync
                            .iter()
                            .map(|f| f.dest_path.to_string_lossy().to_string())
                            .collect();
                        Ok(paths)
                    }
                });
                TaskResult::ConfigApplied(result)
            }

            Operation::ValidateConfigContent { content } => {
                let result =
                    match toml::from_str::<Config>(&content) {
                        Ok(config) => match flux::commands::validate::validate_config(&config) {
                            Ok(report) => {
                                if report.is_valid {
                                    Ok(vec![])
                                } else {
                                    Ok(report
                                        .issues
                                        .iter()
                                        .map(|issue| format!("{issue:?}"))
                                        .collect())
                                }
                            }
                            Err(e) => Err(e),
                        },
                        Err(e) => Ok(vec![format!("TOML parse error: {}", e)]),
                    };
                TaskResult::ConfigValidated(result)
            }
        }
    }

    pub async fn execute<F, T>(&self, operation: F) -> TaskResult
    where
        F: Future<Output = Operation> + Send + 'static,
        T: Future<Output = TaskResult> + Send + 'static,
    {
        let op = operation.await;
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        let task = AsyncTask {
            operation: op,
            reply: reply_tx,
        };
        self.tx.send(task).await.expect("Executor dropped");
        reply_rx.recv().await.expect("Task failed")
    }
}

impl Default for FluxExecutor {
    fn default() -> Self {
        Self::new()
    }
}
