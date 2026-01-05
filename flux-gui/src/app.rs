//! Main application state and component.

use crate::async_executor::{
    GitStatusInfo, GuiFileStatus, GuiRemoteInfo, GuiStatusReport, Operation, TaskResult,
};
use crate::state::{AppView, FileStatus};
use crate::styles::{
    accent_color, border_color, card_bg, dimensions, sidebar_bg, text_primary, text_secondary,
    window_bg,
};
use flux::config::Config;
use flux::services::history::HistoryEvent;
use gpui::{Context, Entity, Render, Window, div, prelude::*, px};
use std::path::PathBuf;

/// Main application state
#[derive(Clone)]
pub struct AppState {
    /// Current view being displayed
    pub current_view: AppView,
    /// Loaded configuration
    pub config: Result<Config, String>,
    /// Current active profile
    pub current_profile: String,
    /// Whether an operation is in progress
    pub is_loading: bool,
    /// Repository path
    pub repo_path: PathBuf,
    /// Output from last operation
    pub operation_output: Vec<String>,
    /// Current notification to display
    pub notification: Option<Notification>,
    /// List of tracked files with their status
    pub tracked_files: Vec<TrackedFileState>,
    /// Available profiles
    pub profiles: Vec<String>,
    /// Configured git remotes
    pub remotes: Vec<GuiRemoteInfo>,
    /// Operation history
    pub history: Vec<HistoryEvent>,
    /// Sync summary statistics
    pub sync_summary: SyncSummaryState,
    /// Git repository status
    pub git_status: GitStatusState,
    /// File list filter state
    pub file_filter: FileFilterState,
    /// History filter state
    pub history_filter: HistoryFilterState,
    /// Config editor content
    pub editor_content: String,
    /// Validation errors
    pub validation_errors: Vec<String>,
    /// Whether settings are being edited
    pub is_editing_settings: bool,
    /// Current profile action state
    pub profile_action: ProfileActionState,
    /// Current remote action state
    pub remote_action: RemoteActionState,
    /// Current file action state
    pub file_action: FileActionState,
    /// Dry run mode enabled
    pub dry_run_mode: bool,
}

/// A notification to display to the user
#[derive(Clone, Debug)]
pub struct Notification {
    /// The message to display
    pub message: String,
    /// Whether this is an error notification
    pub is_error: bool,
}

/// State of a tracked file
#[derive(Clone, Debug)]
pub struct TrackedFileState {
    /// Tool name
    pub tool: String,
    /// Path in repository
    pub repo_path: String,
    /// Destination path
    pub dest_path: String,
    /// Profile (if any)
    pub profile: Option<String>,
    /// Current sync status
    pub status: FileStatus,
    /// Whether selected in UI
    pub selected: bool,
    /// Whether diff is expanded
    pub expanding_diff: bool,
}

impl Default for TrackedFileState {
    fn default() -> Self {
        Self {
            tool: String::new(),
            repo_path: String::new(),
            dest_path: String::new(),
            profile: None,
            status: FileStatus::Pending,
            selected: false,
            expanding_diff: false,
        }
    }
}

/// Sync summary statistics
#[derive(Clone, Debug, Default)]
pub struct SyncSummaryState {
    /// Number of synced files
    pub synced: usize,
    /// Number of out of sync files
    pub out_of_sync: usize,
    /// Number of missing files
    pub missing: usize,
    /// Number of broken/conflicted files
    pub broken: usize,
    /// Total number of files
    pub total: usize,
}

/// Git repository status
#[derive(Clone, Debug, Default)]
pub struct GitStatusState {
    /// Current branch name
    pub branch: String,
    /// Commits ahead of remote
    pub ahead: usize,
    /// Commits behind remote
    pub behind: usize,
    /// Whether remote is configured
    pub has_remote: bool,
    /// Whether this is a git repo
    pub is_repo: bool,
}

/// File filter state
#[derive(Clone, Debug, Default)]
pub struct FileFilterState {
    /// Search query
    pub search: String,
    /// Filter by status
    pub status_filter: Option<FileStatus>,
    /// Filter by tool
    pub tool_filter: Option<String>,
}

/// History filter state
#[derive(Clone, Debug, Default)]
pub struct HistoryFilterState {
    /// Search query
    pub search: String,
    /// Filter by profile
    pub profile_filter: Option<String>,
}

/// Profile action state machine
#[derive(Clone, Debug, Default)]
pub enum ProfileActionState {
    /// No action in progress
    #[default]
    None,
    /// Creating a new profile
    Creating {
        /// Profile name being created
        name: String,
        /// Whether confirming
        confirming: bool,
    },
    /// Deleting a profile
    Deleting {
        /// Profile name to delete
        name: String,
        /// Whether confirming
        confirming: bool,
    },
    /// Switching to a profile
    Switching {
        /// Profile name to switch to
        name: String,
        /// Whether confirming
        confirming: bool,
    },
}

/// Remote action state machine
#[derive(Clone, Debug, Default)]
pub enum RemoteActionState {
    /// No action in progress
    #[default]
    None,
    /// Adding a new remote
    Adding {
        /// Remote name
        name: String,
        /// Remote URL
        url: String,
        /// Whether confirming
        confirming: bool,
    },
    /// Removing a remote
    Removing {
        /// Remote name
        name: String,
        /// Whether confirming
        confirming: bool,
    },
    /// Editing a remote
    Editing {
        /// Remote name
        name: String,
        /// New URL
        new_url: String,
        /// Whether confirming
        confirming: bool,
    },
}

/// File action state machine
#[derive(Clone, Debug, Default)]
pub enum FileActionState {
    /// No action in progress
    #[default]
    None,
    /// Adding a new file
    Adding {
        /// Source path
        source: String,
        /// Destination path
        dest: String,
        /// Tool name
        tool: String,
        /// Profile (optional)
        profile: Option<String>,
        /// Whether confirming
        confirming: bool,
    },
    /// Removing a file
    Removing {
        /// File to remove
        file: TrackedFileState,
        /// Whether confirming
        confirming: bool,
    },
}

impl AppState {
    /// Create a new application state
    #[must_use]
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            current_view: AppView::Home,
            config: Err("Not loaded".to_string()),
            current_profile: "default".to_string(),
            is_loading: false,
            repo_path,
            operation_output: Vec::new(),
            notification: None,
            tracked_files: Vec::new(),
            profiles: vec!["default".to_string()],
            remotes: Vec::new(),
            history: Vec::new(),
            sync_summary: SyncSummaryState::default(),
            git_status: GitStatusState::default(),
            file_filter: FileFilterState::default(),
            history_filter: HistoryFilterState::default(),
            editor_content: String::new(),
            validation_errors: Vec::new(),
            is_editing_settings: false,
            profile_action: ProfileActionState::None,
            remote_action: RemoteActionState::None,
            file_action: FileActionState::None,
            dry_run_mode: false,
        }
    }

    /// Update the sync summary from tracked files
    pub fn update_sync_summary(&mut self) {
        let mut summary = SyncSummaryState::default();
        for file in &self.tracked_files {
            summary.total += 1;
            match file.status {
                FileStatus::Synced => summary.synced += 1,
                FileStatus::OutOfSync => summary.out_of_sync += 1,
                FileStatus::Pending => summary.missing += 1,
                FileStatus::Conflict => summary.broken += 1,
            }
        }
        self.sync_summary = summary;
    }
}

/// The main Flux application
pub struct FluxApp {
    /// Application state
    pub state: AppState,
}

impl gpui::Global for FluxApp {}

impl FluxApp {
    /// Create a new Flux application
    pub fn new(repo_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut app = Self {
            state: AppState::new(repo_path),
        };

        // Load initial data
        app.load_initial_data(cx);

        app
    }

    /// Load initial data on startup
    fn load_initial_data(&mut self, cx: &mut Context<Self>) {
        // Load config
        self.spawn_operation(Operation::LoadConfig, cx);
        // Check status
        self.spawn_operation(Operation::CheckStatus { profile: None }, cx);
        // List profiles
        self.spawn_operation(Operation::ListProfiles, cx);
        // List remotes
        self.spawn_operation(Operation::ListRemotes, cx);
        // Get git status
        self.spawn_operation(Operation::GetGitStatus, cx);
        // Load history
        self.spawn_operation(Operation::ReadHistory { limit: Some(50) }, cx);
    }

    /// Execute an operation (synchronously for now)
    fn spawn_operation(&mut self, operation: Operation, cx: &mut Context<Self>) {
        self.state.is_loading = true;
        cx.notify();

        let result = crate::async_executor::FluxExecutor::execute_task_sync(operation);
        self.handle_task_result(result, cx);
    }

    /// Handle the result of an async task
    fn handle_task_result(&mut self, result: TaskResult, cx: &mut Context<Self>) {
        self.state.is_loading = false;

        match result {
            TaskResult::Config(Ok(config)) => {
                // Update current profile from config
                self.state.current_profile = config.general.current_profile.clone();
                self.state.repo_path = config.get_repo_path();
                self.state.editor_content = toml::to_string_pretty(&config).unwrap_or_default();
                self.state.config = Ok(config);
            }
            TaskResult::Config(Err(e)) => {
                self.state.config = Err(e.to_string());
                self.show_notification(&format!("Error loading config: {e}"), true);
            }

            TaskResult::ConfigSaved(Ok(())) => {
                self.show_notification("Configuration saved", false);
                // Reload after saving
                self.spawn_operation(Operation::LoadConfig, cx);
            }
            TaskResult::ConfigSaved(Err(e)) => {
                self.show_notification(&format!("Error saving config: {e}"), true);
            }

            TaskResult::StatusReports(Ok(reports)) => {
                self.state.tracked_files = self.convert_reports_to_state(reports);
                self.state.update_sync_summary();
            }
            TaskResult::StatusReports(Err(e)) => {
                self.show_notification(&format!("Error checking status: {e}"), true);
            }

            TaskResult::Profiles(Ok(profiles)) => {
                self.state.profiles = profiles;
            }
            TaskResult::Profiles(Err(e)) => {
                self.show_notification(&format!("Error loading profiles: {e}"), true);
            }

            TaskResult::ProfileCreated(Ok(())) => {
                self.show_notification("Profile created", false);
                self.spawn_operation(Operation::ListProfiles, cx);
            }
            TaskResult::ProfileCreated(Err(e)) => {
                self.show_notification(&format!("Error creating profile: {e}"), true);
            }

            TaskResult::ProfileSwitched(Ok(())) => {
                self.show_notification("Profile switched", false);
                // Refresh data after profile switch
                self.spawn_operation(
                    Operation::CheckStatus {
                        profile: Some(self.state.current_profile.clone()),
                    },
                    cx,
                );
            }
            TaskResult::ProfileSwitched(Err(e)) => {
                self.show_notification(&format!("Error switching profile: {e}"), true);
            }

            TaskResult::TrackedFiles(Ok(_files)) => {
                // Tracked files loaded - handled by StatusReports
            }
            TaskResult::TrackedFiles(Err(e)) => {
                self.show_notification(&format!("Error loading files: {e}"), true);
            }

            TaskResult::GitBranch(Ok(branch)) => {
                self.state.git_status.branch = branch;
                self.state.git_status.is_repo = true;
            }
            TaskResult::GitBranch(Err(e)) => {
                self.show_notification(&format!("Error getting git branch: {e}"), true);
            }

            TaskResult::Remotes(Ok(remotes)) => {
                self.state.remotes = remotes;
            }
            TaskResult::Remotes(Err(e)) => {
                self.show_notification(&format!("Error loading remotes: {e}"), true);
            }

            TaskResult::RemoteAdded(Ok(())) => {
                self.show_notification("Remote added", false);
                self.spawn_operation(Operation::ListRemotes, cx);
            }
            TaskResult::RemoteAdded(Err(e)) => {
                self.show_notification(&format!("Error adding remote: {e}"), true);
            }

            TaskResult::RemoteRemoved(Ok(())) => {
                self.show_notification("Remote removed", false);
                self.spawn_operation(Operation::ListRemotes, cx);
            }
            TaskResult::RemoteRemoved(Err(e)) => {
                self.show_notification(&format!("Error removing remote: {e}"), true);
            }

            TaskResult::RemoteUrlSet(Ok(())) => {
                self.show_notification("Remote URL updated", false);
                self.spawn_operation(Operation::ListRemotes, cx);
            }
            TaskResult::RemoteUrlSet(Err(e)) => {
                self.show_notification(&format!("Error updating remote: {e}"), true);
            }

            TaskResult::Pushed(Ok(())) => {
                self.show_notification("Pushed to remote", false);
            }
            TaskResult::Pushed(Err(e)) => {
                self.show_notification(&format!("Error pushing: {e}"), true);
            }

            TaskResult::Pulled(Ok(())) => {
                self.show_notification("Pulled from remote", false);
                // Refresh status after pull
                self.spawn_operation(Operation::CheckStatus { profile: None }, cx);
            }
            TaskResult::Pulled(Err(e)) => {
                self.show_notification(&format!("Error pulling: {e}"), true);
            }

            TaskResult::History(Ok(events)) => {
                self.state.history = events;
            }
            TaskResult::History(Err(e)) => {
                self.show_notification(&format!("Error loading history: {e}"), true);
            }

            TaskResult::GitStatus(Ok(status)) => {
                self.convert_git_status_info(status);
            }
            TaskResult::GitStatus(Err(e)) => {
                self.show_notification(&format!("Error getting git status: {e}"), true);
            }

            TaskResult::FileAdded(Ok(())) => {
                self.show_notification("File added", false);
                self.spawn_operation(Operation::CheckStatus { profile: None }, cx);
            }
            TaskResult::FileAdded(Err(e)) => {
                self.show_notification(&format!("Error adding file: {e}"), true);
            }

            TaskResult::FileRemoved(Ok(())) => {
                self.show_notification("File removed", false);
                self.spawn_operation(Operation::CheckStatus { profile: None }, cx);
            }
            TaskResult::FileRemoved(Err(e)) => {
                self.show_notification(&format!("Error removing file: {e}"), true);
            }

            TaskResult::ConfigApplied(Ok(paths)) => {
                self.show_notification(&format!("Applied {} files", paths.len()), false);
                self.spawn_operation(Operation::CheckStatus { profile: None }, cx);
            }
            TaskResult::ConfigApplied(Err(e)) => {
                self.show_notification(&format!("Error applying config: {e}"), true);
            }

            TaskResult::ConfigValidated(Ok(errors)) => {
                if errors.is_empty() {
                    self.state.validation_errors.clear();
                    self.show_notification("Configuration is valid", false);
                } else {
                    self.state.validation_errors = errors;
                    self.show_notification("Configuration has errors", true);
                }
            }
            TaskResult::ConfigValidated(Err(e)) => {
                self.show_notification(&format!("Error validating config: {e}"), true);
            }
        }

        cx.notify();
    }

    /// Show a notification
    fn show_notification(&mut self, message: &str, is_error: bool) {
        self.state.notification = Some(Notification {
            message: message.to_string(),
            is_error,
        });
    }

    /// Convert status reports to tracked file state
    fn convert_reports_to_state(&self, reports: Vec<GuiStatusReport>) -> Vec<TrackedFileState> {
        reports
            .into_iter()
            .map(|r| TrackedFileState {
                tool: r.tool,
                repo_path: r.repo_path,
                dest_path: r.dest_path,
                profile: r.profile,
                status: self.convert_gui_status(r.status),
                selected: false,
                expanding_diff: false,
            })
            .collect()
    }

    /// Convert GUI file status to app file status
    fn convert_gui_status(&self, status: GuiFileStatus) -> FileStatus {
        match status {
            GuiFileStatus::Synced => FileStatus::Synced,
            GuiFileStatus::OutOfSync => FileStatus::OutOfSync,
            GuiFileStatus::BrokenSymlink | GuiFileStatus::MissingSymlink => FileStatus::Conflict,
            GuiFileStatus::MissingRepo => FileStatus::Pending,
        }
    }

    /// Convert git status info
    fn convert_git_status_info(&mut self, info: GitStatusInfo) {
        self.state.git_status = GitStatusState {
            branch: info.branch,
            ahead: info.ahead,
            behind: info.behind,
            has_remote: info.has_remote,
            is_repo: true,
        };
    }
}

impl Render for FluxApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(window_bg())
            .flex()
            .flex_col()
            .child(self.render_titlebar())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(self.render_sidebar(cx))
                    .child(self.render_main_content(cx)),
            )
    }
}

impl FluxApp {
    /// Render the titlebar
    fn render_titlebar(&self) -> impl IntoElement {
        div()
            .w_full()
            .h(dimensions::TITLEBAR_HEIGHT)
            .bg(sidebar_bg())
            .flex()
            .items_center()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(accent_color())
                    .ml_4()
                    .child("Flux"),
            )
    }

    /// Render the sidebar
    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_view = self.state.current_view;
        let entity = cx.entity().clone();

        div()
            .w(dimensions::SIDEBAR_WIDTH)
            .h_full()
            .bg(sidebar_bg())
            .flex()
            .flex_col()
            .p_4()
            .child(div().h(px(16.0)))
            .child(self.make_nav_item("Home", AppView::Home, current_view, &entity, cx))
            .child(self.make_nav_item("Files", AppView::Files, current_view, &entity, cx))
            .child(self.make_nav_item("Status", AppView::Status, current_view, &entity, cx))
            .child(self.make_nav_item("Profiles", AppView::Profiles, current_view, &entity, cx))
            .child(self.make_nav_item("Remotes", AppView::Remotes, current_view, &entity, cx))
            .child(self.make_nav_item("History", AppView::History, current_view, &entity, cx))
            .child(div().flex_grow())
            .child(self.make_nav_item("Settings", AppView::Settings, current_view, &entity, cx))
    }

    /// Create a navigation item
    fn make_nav_item(
        &mut self,
        label: &str,
        view: AppView,
        current_view: AppView,
        _entity: &Entity<Self>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = current_view == view;
        let label = label.to_string();

        let mut item = div()
            .px_4()
            .py_3()
            .rounded_md()
            .cursor_pointer()
            .bg(if is_active {
                accent_color()
            } else {
                gpui::hsla(0.0, 0.0, 0.0, 0.0)
            })
            .text_color(if is_active {
                window_bg()
            } else {
                text_primary()
            })
            .child(label);

        if !is_active {
            let target_view = view;
            item = item.on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _event, _window, cx| {
                    this.state.current_view = target_view;
                    cx.notify();
                }),
            );
        }

        item
    }

    /// Render the main content area
    fn render_main_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .bg(window_bg())
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_header())
            .child(self.render_notification())
            .child(self.render_current_view(cx))
    }

    /// Render the header
    fn render_header(&self) -> impl IntoElement {
        let view_title = match self.state.current_view {
            AppView::Home => "Dashboard",
            AppView::Files => "Files",
            AppView::Status => "Sync Status",
            AppView::Profiles => "Profiles",
            AppView::Remotes => "Remotes",
            AppView::Settings => "Settings",
            AppView::History => "History",
        };

        let loading_indicator = if self.state.is_loading {
            div().text_color(accent_color()).text_sm().child("Loading...")
        } else {
            div()
        };

        div()
            .w_full()
            .h(dimensions::HEADER_HEIGHT)
            .px_6()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(border_color())
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child(view_title),
            )
            .child(
                div().flex().flex_row().gap_4().items_center().child(loading_indicator).child(
                    div()
                        .text_color(text_secondary())
                        .child(format!("Profile: {}", self.state.current_profile)),
                ),
            )
    }

    /// Render the notification bar if there's a notification
    fn render_notification(&self) -> impl IntoElement {
        if let Some(ref notification) = self.state.notification {
            let bg_color = if notification.is_error {
                gpui::hsla(0.0, 0.7, 0.5, 1.0) // Red for errors
            } else {
                gpui::hsla(0.35, 0.7, 0.4, 1.0) // Green for success
            };

            div()
                .w_full()
                .px_6()
                .py_2()
                .bg(bg_color)
                .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                .child(notification.message.clone())
        } else {
            div()
        }
    }

    /// Render the current view
    fn render_current_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_1().overflow_y_hidden().child(match self.state.current_view {
            AppView::Home => self.render_home_view(cx).into_any_element(),
            AppView::Files => self.render_files_view(cx).into_any_element(),
            AppView::Status => self.render_status_view(cx).into_any_element(),
            AppView::Profiles => self.render_profiles_view(cx).into_any_element(),
            AppView::Remotes => self.render_remotes_view(cx).into_any_element(),
            AppView::Settings => self.render_settings_view(cx).into_any_element(),
            AppView::History => self.render_history_view(cx).into_any_element(),
        })
    }

    /// Render the home view
    fn render_home_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let sync_summary = self.state.sync_summary.clone();
        let git_status = self.state.git_status.clone();
        let repo_path = self.state.repo_path.clone();

        let synced_color = gpui::hsla(0.35, 0.7, 0.5, 1.0);
        let warning_color = gpui::hsla(0.12, 0.9, 0.55, 1.0);
        let missing_color = gpui::hsla(0.0, 0.0, 0.53, 1.0);
        let conflict_color = gpui::hsla(0.0, 0.8, 0.55, 1.0);

        div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Welcome to Flux"),
            )
            .child(
                div()
                    .text_color(text_secondary())
                    .child(format!("Repository: {}", repo_path.display())),
            )
            // Summary cards
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(self.stat_card("Synced", sync_summary.synced, synced_color))
                    .child(self.stat_card("Out of Sync", sync_summary.out_of_sync, warning_color))
                    .child(self.stat_card("Missing", sync_summary.missing, missing_color))
                    .child(self.stat_card("Conflicts", sync_summary.broken, conflict_color)),
            )
            // Git status and quick actions
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(text_primary())
                                    .child("Git Status"),
                            )
                            .child(div().h_4())
                            .child(div().text_color(text_secondary()).child(
                                if git_status.is_repo {
                                    format!("Branch: {}", git_status.branch)
                                } else {
                                    "Not a git repository".to_string()
                                },
                            ))
                            .when(git_status.is_repo && git_status.has_remote, |el| {
                                el.child(div().text_color(text_secondary()).text_sm().mt_2().child(
                                    format!(
                                        "Ahead: {} | Behind: {}",
                                        git_status.ahead, git_status.behind
                                    ),
                                ))
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(text_primary())
                                    .child("Quick Actions"),
                            )
                            .child(div().h_4())
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_4()
                                    .child(self.action_button("Sync All", true, cx, |this, cx| {
                                        this.spawn_operation(
                                            Operation::ApplyConfig {
                                                profile: None,
                                                dry_run: this.state.dry_run_mode,
                                                force: false,
                                            },
                                            cx,
                                        );
                                    }))
                                    .child(self.action_button("Refresh", false, cx, |this, cx| {
                                        this.spawn_operation(
                                            Operation::CheckStatus { profile: None },
                                            cx,
                                        );
                                    })),
                            ),
                    ),
            )
            // Recent activity
            .child(
                div()
                    .w_full()
                    .p_4()
                    .bg(card_bg())
                    .rounded_md()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_primary())
                            .child("Recent Activity"),
                    )
                    .child(div().h_4())
                    .child(if self.state.history.is_empty() {
                        div().text_color(text_secondary()).child("No recent activity")
                    } else {
                        div().flex().flex_col().gap_2().children(
                            self.state.history.iter().take(5).map(|event| {
                                div().text_color(text_secondary()).text_sm().child(format!(
                                    "{} - {}",
                                    event.timestamp.format("%Y-%m-%d %H:%M"),
                                    event.command
                                ))
                            }),
                        )
                    }),
            )
    }

    /// Create a stat card
    fn stat_card(&self, label: &str, value: usize, color: gpui::Hsla) -> impl IntoElement {
        let label = label.to_string();
        div()
            .flex_1()
            .p_4()
            .bg(card_bg())
            .rounded_md()
            .child(
                div()
                    .text_color(color)
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(value.to_string()),
            )
            .child(div().text_color(text_secondary()).text_sm().child(label))
    }

    /// Create an action button
    fn action_button(
        &mut self,
        label: &str,
        primary: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let label = label.to_string();
        div()
            .px_4()
            .py_2()
            .bg(if primary { accent_color() } else { card_bg() })
            .text_color(if primary {
                gpui::hsla(0.0, 0.0, 1.0, 1.0)
            } else {
                text_primary()
            })
            .rounded_md()
            .cursor_pointer()
            .child(label)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _event, _window, cx| {
                    on_click(this, cx);
                }),
            )
    }

    /// Render the files view
    fn render_files_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let files = self.state.tracked_files.clone();

        div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(text_secondary())
                            .child(format!("{} tracked files", files.len())),
                    )
                    .child(self.action_button("Refresh", false, cx, |this, cx| {
                        this.spawn_operation(Operation::CheckStatus { profile: None }, cx);
                    })),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Tracked Files"),
            )
            .child(if files.is_empty() {
                div()
                    .w_full()
                    .p_8()
                    .bg(card_bg())
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(text_secondary())
                            .child("No tracked files. Add files using the CLI: flux add <file>"),
                    )
            } else {
                div().flex().flex_col().gap_2().children(files.iter().map(|file| {
                    let status_color = match file.status {
                        FileStatus::Synced => gpui::hsla(0.35, 0.7, 0.5, 1.0),
                        FileStatus::OutOfSync => gpui::hsla(0.12, 0.9, 0.55, 1.0),
                        FileStatus::Conflict => gpui::hsla(0.0, 0.8, 0.55, 1.0),
                        FileStatus::Pending => gpui::hsla(0.0, 0.0, 0.53, 1.0),
                    };
                    let status_text = match file.status {
                        FileStatus::Synced => "Synced",
                        FileStatus::OutOfSync => "Out of Sync",
                        FileStatus::Conflict => "Conflict",
                        FileStatus::Pending => "Pending",
                    };
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .p_3()
                        .bg(card_bg())
                        .rounded_md()
                        .child(div().w_3().h_3().rounded_full().bg(status_color))
                        .child(
                            div()
                                .ml_3()
                                .flex_1()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(file.dest_path.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_xs()
                                        .mt_1()
                                        .child(format!("{} | {}", file.tool, file.repo_path)),
                                ),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .bg(gpui::hsla(status_color.h, status_color.s, 0.15, 1.0))
                                .text_color(status_color)
                                .rounded_sm()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(status_text),
                        )
                }))
            })
    }

    /// Render the status view
    fn render_status_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let summary = self.state.sync_summary.clone();

        let synced_color = gpui::hsla(0.35, 0.7, 0.5, 1.0);
        let warning_color = gpui::hsla(0.12, 0.9, 0.55, 1.0);
        let missing_color = gpui::hsla(0.0, 0.0, 0.53, 1.0);
        let conflict_color = gpui::hsla(0.0, 0.8, 0.55, 1.0);

        div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(self.stat_card("Synced", summary.synced, synced_color))
                    .child(self.stat_card("Out of Sync", summary.out_of_sync, warning_color))
                    .child(self.stat_card("Missing", summary.missing, missing_color))
                    .child(self.stat_card("Conflicts", summary.broken, conflict_color)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(self.action_button("Sync All", true, cx, |this, cx| {
                        this.spawn_operation(
                            Operation::ApplyConfig {
                                profile: None,
                                dry_run: this.state.dry_run_mode,
                                force: false,
                            },
                            cx,
                        );
                    }))
                    .child(self.action_button("Refresh Status", false, cx, |this, cx| {
                        this.spawn_operation(Operation::CheckStatus { profile: None }, cx);
                    })),
            )
    }

    /// Render the profiles view
    fn render_profiles_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let profiles = self.state.profiles.clone();
        let current_profile = self.state.current_profile.clone();

        div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(div().text_color(text_secondary()).child("Manage your dotfiles profiles"))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Available Profiles"),
            )
            .child(
                div().flex().flex_col().gap_2().children(profiles.iter().map(|profile| {
                    let is_current = *profile == current_profile;
                    let profile_name = profile.clone();

                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .p_3()
                        .bg(card_bg())
                        .rounded_md()
                        .child(
                            div()
                                .flex_1()
                                .text_color(if is_current {
                                    accent_color()
                                } else {
                                    text_primary()
                                })
                                .font_weight(if is_current {
                                    gpui::FontWeight::BOLD
                                } else {
                                    gpui::FontWeight::MEDIUM
                                })
                                .child(if is_current {
                                    format!("{profile} (current)")
                                } else {
                                    profile.clone()
                                }),
                        )
                        .when(!is_current, |el| {
                            el.child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .bg(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                                    .text_color(text_secondary())
                                    .rounded_sm()
                                    .text_xs()
                                    .cursor_pointer()
                                    .child("Switch")
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(move |this, _event, _window, cx| {
                                            this.state.current_profile = profile_name.clone();
                                            this.spawn_operation(
                                                Operation::SwitchProfile {
                                                    name: profile_name.clone(),
                                                },
                                                cx,
                                            );
                                        }),
                                    ),
                            )
                        })
                        .when(is_current, |el| {
                            el.child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .bg(accent_color())
                                    .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                    .rounded_sm()
                                    .text_xs()
                                    .child("Active"),
                            )
                        })
                })),
            )
            .child(
                div().flex().flex_row().gap_4().mt_4().child(self.action_button(
                    "Refresh Profiles",
                    false,
                    cx,
                    |this, cx| {
                        this.spawn_operation(Operation::ListProfiles, cx);
                    },
                )),
            )
    }

    /// Render the remotes view
    fn render_remotes_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let remotes = self.state.remotes.clone();

        div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_color(text_secondary())
                    .child("Manage git remotes for your dotfiles repository"),
            )
            // Git operations
            .child(
                div()
                    .w_full()
                    .p_4()
                    .bg(card_bg())
                    .rounded_md()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_primary())
                            .child("Git Operations"),
                    )
                    .child(div().h_4())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_4()
                            .child(self.action_button("Push", true, cx, |this, cx| {
                                let remote = "origin".to_string();
                                let branch = this.state.git_status.branch.clone();
                                this.spawn_operation(Operation::Push { remote, branch }, cx);
                            }))
                            .child(self.action_button("Pull", true, cx, |this, cx| {
                                let remote = "origin".to_string();
                                let branch = this.state.git_status.branch.clone();
                                this.spawn_operation(Operation::Pull { remote, branch }, cx);
                            }))
                            .child(self.action_button("Refresh", false, cx, |this, cx| {
                                this.spawn_operation(Operation::ListRemotes, cx);
                                this.spawn_operation(Operation::GetGitStatus, cx);
                            })),
                    ),
            )
            // Configured remotes
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Configured Remotes"),
            )
            .child(if remotes.is_empty() {
                div()
                    .w_full()
                    .p_8()
                    .bg(card_bg())
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(text_secondary())
                            .child("No remotes configured. Add a remote using the CLI: flux remote add <name> <url>"),
                    )
            } else {
                div().flex().flex_col().gap_3().children(remotes.iter().map(|remote| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .p_4()
                        .bg(card_bg())
                        .rounded_md()
                        .child(
                            div()
                                .w_10()
                                .h_10()
                                .bg(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div().text_lg().text_color(text_secondary()).child("git"),
                                ),
                        )
                        .child(
                            div()
                                .ml_4()
                                .flex_1()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(remote.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_sm()
                                        .mt_1()
                                        .child(remote.url.clone()),
                                ),
                        )
                }))
            })
    }

    /// Render the history view
    fn render_history_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let history = self.state.history.clone();

        div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(div().text_color(text_secondary()).child("View past operations and changes"))
            .child(div().flex().flex_row().gap_4().child(self.action_button(
                "Refresh History",
                false,
                cx,
                |this, cx| {
                    this.spawn_operation(Operation::ReadHistory { limit: Some(50) }, cx);
                },
            )))
            .child(if history.is_empty() {
                div()
                    .w_full()
                    .p_8()
                    .bg(card_bg())
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(text_secondary())
                            .child("No history yet. Operations will appear here as you use Flux."),
                    )
            } else {
                div().flex().flex_col().gap_3().children(history.iter().map(|event| {
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .p_4()
                        .bg(card_bg())
                        .rounded_md()
                        .child(
                            div()
                                .w_24()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_sm()
                                        .child(event.timestamp.format("%Y-%m-%d").to_string()),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_xs()
                                        .child(event.timestamp.format("%H:%M:%S").to_string()),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(event.command.clone()),
                                )
                                .child(
                                    div().flex().flex_row().gap_4().mt_2().child(
                                        div()
                                            .text_color(if event.dry_run {
                                                gpui::hsla(0.12, 0.9, 0.55, 1.0)
                                            } else {
                                                gpui::hsla(0.35, 0.7, 0.5, 1.0)
                                            })
                                            .text_sm()
                                            .child(if event.dry_run {
                                                "Dry Run"
                                            } else {
                                                "Executed"
                                            }),
                                    ),
                                ),
                        )
                }))
            })
    }

    /// Render the settings view
    fn render_settings_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let dry_run_mode = self.state.dry_run_mode;

        div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(div().text_color(text_secondary()).child("Configure Flux application settings"))
            // General settings
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("General"),
            )
            .child(
                div()
                    .p_4()
                    .bg(card_bg())
                    .rounded_md()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div().flex().flex_row().items_center().justify_between().child(
                            div()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child("Repository Path"),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_sm()
                                        .mt_1()
                                        .child(self.state.repo_path.display().to_string()),
                                ),
                        ),
                    )
                    .child(
                        div().flex().flex_row().items_center().justify_between().child(
                            div()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child("Current Profile"),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_sm()
                                        .mt_1()
                                        .child(self.state.current_profile.clone()),
                                ),
                        ),
                    ),
            )
            // Advanced settings
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Advanced"),
            )
            .child(
                div()
                    .p_4()
                    .bg(card_bg())
                    .rounded_md()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .child(
                                div()
                                    .text_color(text_primary())
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Dry Run Mode"),
                            )
                            .child(
                                div()
                                    .text_color(text_secondary())
                                    .text_sm()
                                    .mt_1()
                                    .child("Preview changes without applying them"),
                            ),
                    )
                    .child(
                        div()
                            .w_12()
                            .h_6()
                            .bg(if dry_run_mode {
                                accent_color()
                            } else {
                                gpui::hsla(0.0, 0.0, 0.15, 1.0)
                            })
                            .rounded_full()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .w_5()
                                    .h_5()
                                    .rounded_full()
                                    .bg(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                    .ml(if dry_run_mode { px(24.0) } else { px(2.0) }),
                            )
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    this.state.dry_run_mode = !this.state.dry_run_mode;
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            // Actions
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .mt_6()
                    .child(self.action_button("Reload Config", false, cx, |this, cx| {
                        this.spawn_operation(Operation::LoadConfig, cx);
                    }))
                    .child(
                        self.action_button("Validate Config", false, cx, |this, cx| {
                            let content = this.state.editor_content.clone();
                            this.spawn_operation(Operation::ValidateConfigContent { content }, cx);
                        }),
                    ),
            )
    }
}
