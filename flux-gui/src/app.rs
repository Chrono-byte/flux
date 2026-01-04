//! Main application state and component.

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
pub struct GuiRemoteInfo {
    pub name: String,
    pub url: String,
    pub branch: String,
}

#[derive(Clone, Debug)]
pub enum ProfileActionState {
    None,
    Creating { name: String, confirming: bool },
    Deleting { name: String, confirming: bool },
    Switching { name: String, confirming: bool },
}

impl Default for ProfileActionState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug)]
pub enum RemoteActionState {
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

impl Default for RemoteActionState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug)]
pub enum FileActionState {
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

impl Default for FileActionState {
    fn default() -> Self {
        Self::None
    }
}

impl AppState {
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
