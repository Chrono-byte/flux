//! Main application state and component.

use crate::async_executor::{
    GitStatusInfo, GuiFileStatus, GuiRemoteInfo, GuiStatusReport, Operation, TaskResult,
};
use crate::events::AppEvent;
use crate::state::{AppView, FileStatus};
use crate::styles::{
    accent_color, border_color, dimensions, sidebar_bg, text_primary, text_secondary, window_bg,
};
use flux::config::Config;
use flux::services::history::HistoryEvent;
use gpui::{Context, Render, Window, div, prelude::*, px};
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub current_view: AppView,
    pub config: Result<Config, String>,
    pub current_profile: String,
    pub is_loading: bool,
    pub repo_path: PathBuf,
    pub operation_output: Vec<String>,
    pub notification: Option<Notification>,
    pub tracked_files: Vec<TrackedFileState>,
    pub profiles: Vec<String>,
    pub remotes: Vec<GuiRemoteInfo>,
    pub history: Vec<HistoryEvent>,
    pub sync_summary: SyncSummaryState,
    pub git_status: GitStatusState,
    pub file_filter: FileFilterState,
    pub history_filter: HistoryFilterState,
    pub editor_content: String,
    pub validation_errors: Vec<String>,
    pub is_editing_settings: bool,
    pub profile_action: ProfileActionState,
    pub remote_action: RemoteActionState,
    pub file_action: FileActionState,
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub message: String,
    pub is_error: bool,
}

#[derive(Clone, Debug)]
pub struct TrackedFileState {
    pub tool: String,
    pub repo_path: String,
    pub dest_path: String,
    pub profile: Option<String>,
    pub status: FileStatus,
    pub selected: bool,
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

#[derive(Clone, Debug, Default)]
pub struct SyncSummaryState {
    pub synced: usize,
    pub out_of_sync: usize,
    pub missing: usize,
    pub broken: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Default)]
pub struct GitStatusState {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub has_remote: bool,
    pub is_repo: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FileFilterState {
    pub search: String,
    pub status_filter: Option<FileStatus>,
    pub tool_filter: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct HistoryFilterState {
    pub search: String,
    pub profile_filter: Option<String>,
}

#[derive(Clone, Debug)]
#[derive(Default)]
pub enum ProfileActionState {
    #[default]
    None,
    Creating { name: String, confirming: bool },
    Deleting { name: String, confirming: bool },
    Switching { name: String, confirming: bool },
}


#[derive(Clone, Debug)]
#[derive(Default)]
pub enum RemoteActionState {
    #[default]
    None,
    Adding {
        name: String,
        url: String,
        confirming: bool,
    },
    Removing {
        name: String,
        confirming: bool,
    },
    Editing {
        name: String,
        new_url: String,
        confirming: bool,
    },
}


#[derive(Clone, Debug)]
#[derive(Default)]
pub enum FileActionState {
    #[default]
    None,
    Adding {
        source: String,
        dest: String,
        tool: String,
        profile: Option<String>,
        confirming: bool,
    },
    Removing {
        file: TrackedFileState,
        confirming: bool,
    },
}


impl AppState {
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
        }
    }

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

pub struct FluxApp {
    pub state: AppState,
}

impl gpui::Global for FluxApp {}

impl FluxApp {
    pub fn new(repo_path: PathBuf, _cx: &mut Context<Self>) -> Self {
        Self {
            state: AppState::new(repo_path),
        }
    }

    /// Handle an application event
    pub fn handle_event(&mut self, event: AppEvent, cx: &mut Context<Self>) {
        match event {
            AppEvent::SwitchView(view) => {
                self.state.current_view = view;
                cx.notify();
            }

            AppEvent::RefreshStatus { profile } => {
                self.spawn_operation(Operation::CheckStatus { profile }, cx);
            }

            AppEvent::AddFile { tool, path, dest } => {
                let source_path = PathBuf::from(path);
                let dest_path = PathBuf::from(dest);
                self.spawn_operation(
                    Operation::AddFile {
                        tool,
                        source_path,
                        dest_path,
                        profile: None,
                    },
                    cx,
                );
            }

            AppEvent::RemoveFile { tool, file } => {
                self.spawn_operation(
                    Operation::RemoveFile {
                        tool,
                        file_name: file,
                    },
                    cx,
                );
            }

            AppEvent::RefreshFileList { profile } => {
                self.spawn_operation(Operation::CheckStatus { profile }, cx);
            }

            AppEvent::FileOperationSuccess { message } => {
                self.show_notification(&message, false);
                cx.notify();
            }

            AppEvent::CreateProfile { name } => {
                self.spawn_operation(Operation::CreateProfile { name }, cx);
            }

            AppEvent::SwitchProfile { name } => {
                self.spawn_operation(Operation::SwitchProfile { name: name.clone() }, cx);
                self.state.current_profile = name;
                cx.notify();
            }

            AppEvent::RefreshProfiles => {
                self.spawn_operation(Operation::ListProfiles, cx);
            }

            AppEvent::Push { remote, dry_run: _ } => {
                let remote_name = remote.unwrap_or_else(|| "origin".to_string());
                let branch = self.state.git_status.branch.clone();
                self.spawn_operation(
                    Operation::Push {
                        remote: remote_name,
                        branch,
                    },
                    cx,
                );
            }

            AppEvent::Pull { remote, dry_run: _ } => {
                let remote_name = remote.unwrap_or_else(|| "origin".to_string());
                let branch = self.state.git_status.branch.clone();
                self.spawn_operation(
                    Operation::Pull {
                        remote: remote_name,
                        branch,
                    },
                    cx,
                );
            }

            AppEvent::AddRemote { name, url } => {
                self.spawn_operation(Operation::AddRemote { name, url }, cx);
            }

            AppEvent::RemoveRemote { name } => {
                self.spawn_operation(Operation::RemoveRemote { name }, cx);
            }

            AppEvent::RefreshRemotes => {
                self.spawn_operation(Operation::ListRemotes, cx);
            }

            AppEvent::ApplyConfig { profile, dry_run } => {
                self.spawn_operation(
                    Operation::ApplyConfig {
                        profile,
                        dry_run,
                        force: false,
                    },
                    cx,
                );
            }

            AppEvent::CommitAndSync { message: _, dry_run: _ } => {
                // TODO: Implement commit and sync
                self.show_notification("Commit and sync not yet implemented", true);
                cx.notify();
            }

            AppEvent::LoadConfigForEdit => {
                self.spawn_operation(Operation::LoadConfig, cx);
            }

            AppEvent::SaveConfig { content } => {
                // Parse and save config
                match toml::from_str::<Config>(&content) {
                    Ok(config) => {
                        self.spawn_operation(Operation::SaveConfig { config }, cx);
                    }
                    Err(e) => {
                        self.show_notification(&format!("Invalid TOML: {e}"), true);
                        cx.notify();
                    }
                }
            }

            AppEvent::ValidateConfig { content } => {
                self.spawn_operation(Operation::ValidateConfigContent { content }, cx);
            }

            AppEvent::ShowError { message } => {
                self.show_notification(&message, true);
                cx.notify();
            }

            AppEvent::ClearError => {
                self.state.notification = None;
                cx.notify();
            }

            AppEvent::ShowSuccess { message } => {
                self.show_notification(&message, false);
                cx.notify();
            }

            AppEvent::ClearNotification => {
                self.state.notification = None;
                cx.notify();
            }

            AppEvent::SetLoading { loading } => {
                self.state.is_loading = loading;
                cx.notify();
            }

            AppEvent::LoadDiff { files: _ } => {
                // TODO: Implement diff loading
                self.show_notification("Diff loading not yet implemented", true);
                cx.notify();
            }

            AppEvent::DiffLoaded { lines: _ } => {
                // TODO: Store diff lines
            }

            AppEvent::SaveWindowGeometry | AppEvent::LoadWindowGeometry => {
                // TODO: Implement window geometry persistence
            }
        }
    }

    /// Spawn an async operation
    fn spawn_operation(&mut self, operation: Operation, cx: &mut Context<Self>) {
        self.state.is_loading = true;
        cx.notify();

        // TODO: Make this properly async with correct GPUI pattern
        // For now, just execute synchronously to unblock development
        std::thread::spawn(move || {
            let result = futures::executor::block_on(
                crate::async_executor::FluxExecutor::execute_task(operation),
            );

            // TODO: Update app state with result
            // This requires passing the app entity safely to the thread
            // For now, results won't be handled until we fix the async pattern
            drop(result);
        });
    }

    /// Handle the result of an async task
    fn handle_task_result(&mut self, result: TaskResult, cx: &mut Context<Self>) {
        self.state.is_loading = false;

        match result {
            TaskResult::Config(Ok(config)) => {
                self.state.config = Ok(config.clone());
                self.state.editor_content = toml::to_string_pretty(&config).unwrap_or_default();
                self.show_notification("Configuration loaded", false);
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
                self.show_notification("Status refreshed", false);
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
                // Tracked files loaded
            }
            TaskResult::TrackedFiles(Err(e)) => {
                self.show_notification(&format!("Error loading files: {e}"), true);
            }

            TaskResult::GitBranch(Ok(branch)) => {
                self.state.git_status.branch = branch;
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

            TaskResult::ConfigApplied(Ok(_paths)) => {
                self.show_notification("Configuration applied", false);
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
            .w(px(10000.0))
            .h(px(10000.0))
            .max_w(px(10000.0))
            .max_h(px(10000.0))
            .bg(window_bg())
            .flex()
            .flex_col()
            .child(self.render_titlebar())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .child(self.render_sidebar(cx))
                    .child(self.render_main_content()),
            )
    }
}

impl FluxApp {
    fn render_titlebar(&self) -> impl IntoElement {
        div()
            .w(px(10000.0))
            .max_w(px(10000.0))
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

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_view = self.state.current_view;
        let entity = cx.entity().clone();

        div()
            .w(dimensions::SIDEBAR_WIDTH)
            .h(px(10000.0))
            .max_h(px(10000.0))
            .bg(sidebar_bg())
            .flex()
            .flex_col()
            .p_4()
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(accent_color())
                    .child("Flux"),
            )
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

    fn make_nav_item(
        &mut self,
        label: &str,
        view: AppView,
        current_view: AppView,
        entity: &gpui::Entity<Self>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = current_view == view;
        let label = label.to_string();
        let _target_view = view;
        let entity = entity.clone();

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
            let entity = entity.clone();
            item = item.on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |_this, _event, window, _cx| {
                    let target_view = target_view;
                    let entity = entity.clone();
                    let entity_id = entity.entity_id();
                    window.on_next_frame(move |_window, cx| {
                        entity.update(cx, |state, _cx| {
                            state.state.current_view = target_view;
                        });
                        cx.notify(entity_id);
                    });
                }),
            );
        }

        item
    }

    fn render_main_content(&self) -> impl IntoElement {
        div()
            .flex_1()
            .h(px(10000.0))
            .max_h(px(10000.0))
            .bg(window_bg())
            .flex()
            .flex_col()
            .child(self.render_header())
            .child(self.render_current_view())
    }

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

        div()
            .w(px(10000.0))
            .max_w(px(10000.0))
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
            .child(div().text_color(text_secondary()).child(self.state.current_profile.clone()))
    }

    fn render_current_view(&self) -> gpui::AnyElement {
        match self.state.current_view {
            AppView::Home => self.render_home_view().into_any_element(),
            AppView::Files => self.render_placeholder("Files View").into_any_element(),
            AppView::Status => self.render_placeholder("Status View").into_any_element(),
            AppView::Profiles => self.render_placeholder("Profiles View").into_any_element(),
            AppView::Remotes => self.render_placeholder("Remotes View").into_any_element(),
            AppView::Settings => self.render_placeholder("Settings View").into_any_element(),
            AppView::History => self.render_placeholder("History View").into_any_element(),
        }
    }

    fn render_home_view(&self) -> impl IntoElement {
        let repo_path = self.state.repo_path.clone();
        div()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
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
    }

    fn render_placeholder(&self, name: &str) -> impl IntoElement {
        let name = name.to_string();
        div().p_6().child(div().text_2xl().text_color(text_primary()).child(name))
    }
}
