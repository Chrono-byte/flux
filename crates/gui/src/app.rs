use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub current_profile: Option<String>,
    pub current_profile_name: Option<String>,
    pub repo_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileReport {
    pub status: String,
    pub dest_path: Option<String>,
    pub repo_path: Option<String>,
    pub tool: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub ahead: i32,
    pub behind: i32,
    pub has_remote: bool,
    pub is_repo: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub timestamp: String,
    pub command: String,
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncSummary {
    pub synced: usize,
    pub out_of_sync: usize,
    pub missing: usize,
    pub broken: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub message: String,
    pub is_error: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum View {
    Home,
    Files,
    Status,
    Profiles,
    Remotes,
    History,
    Settings,
}

impl View {
    fn to_title(&self) -> &'static str {
        match self {
            View::Home => "Dashboard",
            View::Files => "Files",
            View::Status => "Sync Status",
            View::Profiles => "Profiles",
            View::Remotes => "Remotes",
            View::History => "History",
            View::Settings => "Settings",
        }
    }
}

#[derive(Clone)]
struct AppState {
    current_view: RwSignal<View>,
    current_profile: RwSignal<String>,
    repo_path: RwSignal<String>,
    dry_run_mode: RwSignal<bool>,
    is_loading: RwSignal<bool>,
    sync_summary: RwSignal<SyncSummary>,
    git_status: RwSignal<GitStatus>,
    files: RwSignal<Vec<FileReport>>,
    profiles: RwSignal<Vec<String>>,
    remotes: RwSignal<Vec<Remote>>,
    history: RwSignal<Vec<HistoryEvent>>,
    notification: RwSignal<Option<Notification>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_view: RwSignal::new(View::Home),
            current_profile: RwSignal::new("default".to_string()),
            repo_path: RwSignal::new("~/.dotfiles".to_string()),
            dry_run_mode: RwSignal::new(false),
            is_loading: RwSignal::new(false),
            sync_summary: RwSignal::new(SyncSummary {
                synced: 0,
                out_of_sync: 0,
                missing: 0,
                broken: 0,
                total: 0,
            }),
            git_status: RwSignal::new(GitStatus {
                branch: String::new(),
                ahead: 0,
                behind: 0,
                has_remote: false,
                is_repo: false,
            }),
            files: RwSignal::new(Vec::new()),
            profiles: RwSignal::new(Vec::new()),
            remotes: RwSignal::new(Vec::new()),
            history: RwSignal::new(Vec::new()),
            notification: RwSignal::new(None),
        }
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_context(AppState::default());
    view! {
        <div class="app-container">
            <Titlebar />
            <div class="main-container">
                <Sidebar />
                <MainContent />
            </div>
            <NotificationBar />
        </div>
    }
}

#[component]
fn Titlebar() -> impl IntoView {
    view! { <header class="titlebar"><div class="titlebar-title">"Flux"</div></header> }
}

#[component]
fn Sidebar() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let current_view = state.current_view;

    view! {
        <nav class="sidebar">
            <div class="nav-spacer"></div>
            <button class="nav-item active" on:click=move |_| current_view.set(View::Home)>"Home"</button>
            <button class="nav-item" on:click=move |_| current_view.set(View::Files)>"Files"</button>
            <button class="nav-item" on:click=move |_| current_view.set(View::Status)>"Status"</button>
            <button class="nav-item" on:click=move |_| current_view.set(View::Profiles)>"Profiles"</button>
            <button class="nav-item" on:click=move |_| current_view.set(View::Remotes)>"Remotes"</button>
            <button class="nav-item" on:click=move |_| current_view.set(View::History)>"History"</button>
            <div class="nav-spacer flex-grow"></div>
            <button class="nav-item" on:click=move |_| current_view.set(View::Settings)>"Settings"</button>
        </nav>
    }
}

#[component]
fn MainContent() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let current_view = state.current_view;

    view! {
        <main class="content">
            <Header />
            <div class="views-container">
                {move || match current_view.get() {
                    View::Home => view! { <HomeView /> }.into_any(),
                    View::Files => view! { <FilesView /> }.into_any(),
                    View::Status => view! { <StatusView /> }.into_any(),
                    View::Profiles => view! { <ProfilesView /> }.into_any(),
                    View::Remotes => view! { <RemotesView /> }.into_any(),
                    View::History => view! { <HistoryView /> }.into_any(),
                    View::Settings => view! { <SettingsView /> }.into_any(),
                }}
            </div>
        </main>
    }
}

#[component]
fn Header() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let current_view = state.current_view;
    let current_profile = state.current_profile;
    let is_loading = state.is_loading;

    view! {
        <div class="header">
            <h1>{move || current_view.get().to_title()}</h1>
            <div class="header-right">
                <span class=move || if is_loading.get() { "loading" } else { "loading hidden" }>"Loading..."</span>
                <span class="profile-badge">"Profile: "{current_profile}</span>
            </div>
        </div>
    }
}

#[component]
fn NotificationBar() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let notification = state.notification;

    view! {
        <div class=move || match notification.get() {
            Some(n) if n.is_error => "notification error",
            Some(_) => "notification success",
            None => "notification hidden",
        }>
            <span>{move || notification.get().as_ref().map(|n| n.message.clone()).unwrap_or_default()}</span>
        </div>
    }
}

#[component]
fn HomeView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let repo_path = state.repo_path;
    let sync_summary = state.sync_summary;
    let git_status = state.git_status;
    let history = state.history;

    Effect::new(move |_| {
        spawn_local(async move {
            let _ = load_config().await;
            let _ = load_status().await;
            let _ = load_git_status().await;
            let _ = load_history().await;
        });
    });

    let recent_activity = move || {
        let h = history.get();
        view! {
            <div class="activity-container">
                {if h.is_empty() {
                    view! { <p class="text-secondary">"No recent activity"</p> }.into_any()
                } else {
                    view! {
                        <div class="activity-list">
                            {h.into_iter().take(5).map(|event| {
                                let timestamp = parse_timestamp(&event.timestamp);
                                view! { <div class="activity-item">{format!("{} - {}", timestamp, event.command)}</div> }
                            }).collect::<Vec<_>>()}
                        </div>
                    }
                    .into_any()
                }}
            </div>
        }
    };

    view! {
        <div class="view active">
            <h2 class="welcome-title">"Welcome to Flux"</h2>
            <p class="repo-path">"Repository: "{repo_path}</p>
            <div class="stat-cards">
                <div class="stat-card"><span class="stat-value synced">{move || sync_summary.get().synced.to_string()}</span><span class="stat-label">"Synced"</span></div>
                <div class="stat-card"><span class="stat-value warning">{move || sync_summary.get().out_of_sync.to_string()}</span><span class="stat-label">"Out of Sync"</span></div>
                <div class="stat-card"><span class="stat-value muted">{move || sync_summary.get().missing.to_string()}</span><span class="stat-label">"Missing"</span></div>
                <div class="stat-card"><span class="stat-value error">{move || sync_summary.get().broken.to_string()}</span><span class="stat-label">"Conflicts"</span></div>
            </div>
            <div class="two-column">
                <div class="card">
                    <h3>"Git Status"</h3>
                    <p class="text-secondary">"Branch: "{move || {
                        let gs = git_status.get();
                        if gs.is_repo { gs.branch.clone() } else { "Not a git repository".to_string() }
                    }}</p>
                    <p class=move || {
        let gs = git_status.get();
        if gs.has_remote { "text-secondary" } else { "text-secondary hidden" }
    }>
                        "Ahead: "{move || git_status.get().ahead.to_string()}" | Behind: "{move || git_status.get().behind.to_string()}
                    </p>
                </div>
                <div class="card">
                    <h3>"Quick Actions"</h3>
                    <div class="button-group">
                        <button class="btn btn-primary" on:click=|_| spawn_local(async { sync_all().await })>"Sync All"</button>
                        <button class="btn btn-secondary" on:click=|_| spawn_local(async { load_home_data().await })>"Refresh"</button>
                    </div>
                </div>
            </div>
            <div class="card">
                <h3>"Recent Activity"</h3>
                {recent_activity}
            </div>
        </div>
    }
}

#[component]
fn FilesView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let files = state.files;

    Effect::new(|_| {
        spawn_local(async move {
            let _ = load_status().await;
        });
    });

    let file_list = move || {
        let f = files.get();
        if f.is_empty() {
            view! { <div class="empty-state"><p class="text-secondary">"No tracked files"</p></div> }.into_any()
        } else {
            view! { <div class="files-list">
                {f.into_iter().map(|file| {
                    let status_class = get_status_class(&file.status);
                    view! {
                        <div class="file-item">
                            <div class=format!("file-status-dot {}", status_class.as_str())></div>
                            <div class="file-info">
                                <div class="file-path">{file.dest_path.clone().unwrap_or_default()}</div>
                                <div class="file-details">{file.tool.clone().unwrap_or_default()}</div>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div> }.into_any()
        }
    };

    view! {
        <div class="view">
            <div class="view-header">
                <p class="text-secondary">{move || format!("{} tracked files", files.get().len())}</p>
                <button class="btn btn-secondary" on:click=|_| spawn_local(async { let _ = load_status().await; })>"Refresh"</button>
            </div>
            <h3>"Tracked Files"</h3>
            {file_list}
        </div>
    }
}

#[component]
fn StatusView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let sync_summary = state.sync_summary;

    Effect::new(|_| {
        spawn_local(async move {
            let _ = load_status().await;
        });
    });

    view! {
        <div class="view">
            <div class="stat-cards">
                <div class="stat-card"><span class="stat-value synced">{move || sync_summary.get().synced.to_string()}</span><span class="stat-label">"Synced"</span></div>
                <div class="stat-card"><span class="stat-value warning">{move || sync_summary.get().out_of_sync.to_string()}</span><span class="stat-label">"Out of Sync"</span></div>
                <div class="stat-card"><span class="stat-value muted">{move || sync_summary.get().missing.to_string()}</span><span class="stat-label">"Missing"</span></div>
                <div class="stat-card"><span class="stat-value error">{move || sync_summary.get().broken.to_string()}</span><span class="stat-label">"Conflicts"</span></div>
            </div>
            <div class="button-group">
                <button class="btn btn-primary" on:click=|_| spawn_local(async { sync_all().await })>"Sync All"</button>
                <button class="btn btn-secondary" on:click=|_| spawn_local(async { let _ = load_status().await; })>"Refresh Status"</button>
            </div>
        </div>
    }
}

#[component]
fn ProfilesView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let profiles = state.profiles;
    let current_profile = state.current_profile;

    Effect::new(|_| {
        spawn_local(async move {
            let _ = load_profiles().await;
        });
    });

    view! {
        <div class="view">
            <p class="text-secondary">"Manage your dotfiles profiles"</p>
            <h3>"Available Profiles"</h3>
            <div class="profiles-list">
                {move || profiles.get().into_iter().map(|profile| {
                    let is_active = profile == current_profile.get();
                    let name = profile.clone();
                    view! {
                        <div class="profile-item">
                            <span class=format!("profile-name {}", if is_active { "active" } else { "" })>{profile}</span>
                            {if is_active {
                                view! { <span class="profile-badge active">"Active"</span> }.into_any()
                            } else {
                                view! { <button class="profile-badge switch" on:click=move |_| {
                                    let profile_name = name.clone();
                                    spawn_local(async move { switch_profile(profile_name).await });
                                } >"Switch"</button> }.into_any()
                            }}
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
            <div class="button-group mt-4">
                <button class="btn btn-secondary" on:click=|_| spawn_local(async { let _ = load_profiles().await; })>"Refresh"</button>
            </div>
        </div>
    }
}

#[component]
fn RemotesView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let remotes = state.remotes;

    Effect::new(|_| {
        spawn_local(async move {
            let _ = load_remotes().await;
        });
    });

    view! {
        <div class="view">
            <p class="text-secondary">"Manage git remotes"</p>
            <div class="card">
                <h3>"Git Operations"</h3>
                <div class="button-group">
                    <button class="btn btn-primary" on:click=|_| spawn_local(async { push_to_remote().await })>"Push"</button>
                    <button class="btn btn-primary" on:click=|_| spawn_local(async { pull_from_remote().await })>"Pull"</button>
                    <button class="btn btn-secondary" on:click=|_| spawn_local(async { load_remotes_and_git().await })>"Refresh"</button>
                </div>
            </div>
            <h3>"Configured Remotes"</h3>
            <div class="remotes-list">
                {move || remotes.get().into_iter().map(|remote| {
                    view! {
                        <div class="remote-item">
                            <div class="remote-icon">"git"</div>
                            <div class="remote-info">
                                <div class="remote-name">{remote.name}</div>
                                <div class="remote-url">{remote.url}</div>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn HistoryView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let history = state.history;

    Effect::new(|_| {
        spawn_local(async move {
            let _ = load_history().await;
        });
    });

    view! {
        <div class="view">
            <p class="text-secondary">"View past operations"</p>
            <div class="button-group">
                <button class="btn btn-secondary" on:click=|_| spawn_local(async { let _ = load_history().await; })>"Refresh"</button>
            </div>
            <div class="history-list">
                {move || history.get().into_iter().map(|event| {
                    let (date_str, time_str) = parse_timestamp_full(&event.timestamp);
                    view! {
                        <div class="history-item">
                            <div class="history-time">
                                <span class="history-date">{date_str}</span>
                                <span class="history-clock">{time_str}</span>
                            </div>
                            <div class="history-details">
                                <div class="history-command">{event.command}</div>
                                <div class="history-tags">
                                    <span class=format!("history-tag {}", if event.dry_run { "dry-run" } else { "executed" })>
                                        {if event.dry_run { "Dry Run" } else { "Executed" }}
                                    </span>
                                </div>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn SettingsView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let repo_path = state.repo_path;
    let current_profile = state.current_profile;
    let dry_run_mode = state.dry_run_mode;

    view! {
        <div class="view">
            <p class="text-secondary">"Configure Flux settings"</p>
            <h3>"General"</h3>
            <div class="card">
                <div class="setting-row"><div><span class="setting-label">"Repository Path"</span><span class="setting-value text-secondary">{repo_path}</span></div></div>
                <div class="setting-row"><div><span class="setting-label">"Current Profile"</span><span class="setting-value text-secondary">{current_profile}</span></div></div>
            </div>
            <h3>"Advanced"</h3>
            <div class="card">
                <div class="setting-row">
                    <div>
                        <span class="setting-label">"Dry Run Mode"</span>
                        <span class="setting-description text-secondary">"Preview changes"</span>
                    </div>
                    <label class="toggle">
                        <input type="checkbox" checked=dry_run_mode on:change=move |e| {
                            let checked = event_target_checked(&e);
                            dry_run_mode.set(checked);
                            spawn_local(async move { let () = set_dry_run_mode(checked).await; });
                        } />
                        <span class="toggle-slider"></span>
                    </label>
                </div>
            </div>
            <div class="button-group mt-6">
                <button class="btn btn-secondary" on:click=|_| spawn_local(async { let _ = load_config().await; })>"Reload Config"</button>
                <button class="btn btn-secondary" on:click=|_| spawn_local(async { validate_config().await })>"Validate"</button>
            </div>
        </div>
    }
}

fn get_status_class(status: &str) -> String {
    match status {
        "synced" => "synced".to_string(),
        "outOfSync" => "warning".to_string(),
        "brokenSymlink" => "error".to_string(),
        "missingSymlink" | "missingRepo" => "muted".to_string(),
        _ => "muted".to_string(),
    }
}

fn parse_timestamp(ts: &str) -> String {
    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(ts) {
        date.format("%m/%d %H:%M").to_string()
    } else {
        ts.to_string()
    }
}

fn parse_timestamp_full(ts: &str) -> (String, String) {
    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(ts) {
        (
            date.format("%m/%d").to_string(),
            date.format("%H:%M").to_string(),
        )
    } else {
        (ts.to_string(), String::new())
    }
}

fn log(msg: &str) {
    web_sys::console::log_1(&msg.to_string().into());
}

fn show_notification(message: &str, is_error: bool) {
    if let Some(state) = use_context::<AppState>() {
        state.notification.set(Some(Notification {
            message: message.to_string(),
            is_error,
        }));
        let state_clone = state.clone();
        gloo_timers::callback::Timeout::new(5000, move || {
            state_clone.notification.set(None);
        })
        .forget();
    }
}

async fn load_home_data() {
    let _ = load_config().await;
    let _ = load_status().await;
    let _ = load_git_status().await;
    let _ = load_history().await;
}

async fn load_config() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };
    state.is_loading.set(true);
    let result = invoke("load_config", JsValue::NULL).await;
    let config: Config = serde_wasm_bindgen::from_value(result)?;
    if let Some(p) = config.current_profile.or(config.current_profile_name) {
        state.current_profile.set(p);
    }
    if let Some(p) = config.repo_path {
        state.repo_path.set(p);
    }
    state.is_loading.set(false);
    Ok(())
}

async fn load_status() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };
    state.is_loading.set(true);
    let result = invoke("check_status", JsValue::NULL).await;
    let reports: Vec<FileReport> = serde_wasm_bindgen::from_value(result)?;
    state.files.set(reports.clone());
    let mut s = SyncSummary {
        synced: 0,
        out_of_sync: 0,
        missing: 0,
        broken: 0,
        total: reports.len(),
    };
    for r in &reports {
        match r.status.as_str() {
            "synced" => s.synced += 1,
            "outOfSync" => s.out_of_sync += 1,
            "missingSymlink" | "missingRepo" => s.missing += 1,
            "brokenSymlink" => s.broken += 1,
            _ => {}
        }
    }
    state.sync_summary.set(s);
    state.is_loading.set(false);
    Ok(())
}

async fn load_profiles() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };
    state.is_loading.set(true);
    let result = invoke("list_profiles", JsValue::NULL).await;
    state.profiles.set(serde_wasm_bindgen::from_value(result)?);
    state.is_loading.set(false);
    Ok(())
}

async fn load_remotes() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };
    state.is_loading.set(true);
    let result = invoke("list_remotes", JsValue::NULL).await;
    match serde_wasm_bindgen::from_value::<Vec<Remote>>(result) {
        Ok(remotes) => state.remotes.set(remotes),
        Err(e) => log(&format!("Failed to load remotes: {e:?}")),
    }
    state.is_loading.set(false);
    Ok(())
}

async fn load_git_status() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };
    let result = invoke("get_git_status", JsValue::NULL).await;
    match serde_wasm_bindgen::from_value::<GitStatus>(result) {
        Ok(status) => state.git_status.set(status),
        Err(e) => log(&format!("Failed to load git status: {e:?}")),
    }
    Ok(())
}

async fn load_history() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };
    state.is_loading.set(true);
    let result = invoke("read_history", JsValue::NULL).await;
    match serde_wasm_bindgen::from_value::<Vec<HistoryEvent>>(result) {
        Ok(history) => state.history.set(history),
        Err(e) => log(&format!("Failed to load history: {e:?}")),
    }
    state.is_loading.set(false);
    Ok(())
}

async fn load_remotes_and_git() {
    let _ = load_remotes().await;
    let _ = load_git_status().await;
}

async fn sync_all() {
    let Some(state) = use_context::<AppState>() else {
        return;
    };
    state.is_loading.set(true);
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "profile": serde_json::Value::Null, "dryRun": state.dry_run_mode.get(), "force": false
    }))
    .unwrap();
    let result = invoke("apply_config", args).await;
    match serde_wasm_bindgen::from_value::<()>(result) {
        Ok(()) => {
            show_notification(
                if state.dry_run_mode.get() {
                    "Dry run completed"
                } else {
                    "Applied"
                },
                false,
            );
            let _ = load_status().await;
        }
        Err(e) => {
            log(&format!("Failed to sync: {e:?}"));
            show_notification("Sync failed", true);
        }
    }
    state.is_loading.set(false);
}

async fn switch_profile(name: String) {
    let Some(state) = use_context::<AppState>() else {
        return;
    };
    state.is_loading.set(true);
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "name": name })).unwrap();
    let result = invoke("switch_profile", args).await;
    match serde_wasm_bindgen::from_value::<()>(result) {
        Ok(()) => {
            state.current_profile.set(name.clone());
            show_notification(&format!("Switched to {name}"), false);
            let _ = load_status().await;
        }
        Err(e) => {
            log(&format!("Failed: {e:?}"));
            show_notification("Failed", true);
        }
    }
    state.is_loading.set(false);
}

async fn push_to_remote() {
    let Some(state) = use_context::<AppState>() else {
        return;
    };
    state.is_loading.set(true);
    let git_status = state.git_status.get();
    let b = if git_status.branch.is_empty() {
        "main".to_string()
    } else {
        git_status.branch
    };
    let args =
        serde_wasm_bindgen::to_value(&serde_json::json!({ "remote": "origin", "branch": b }))
            .unwrap();
    let result = invoke("push_to_remote", args).await;
    match serde_wasm_bindgen::from_value::<()>(result) {
        Ok(()) => show_notification("Pushed", false),
        Err(e) => {
            log(&format!("Failed: {e:?}"));
            show_notification("Push failed", true);
        }
    }
    state.is_loading.set(false);
}

async fn pull_from_remote() {
    let Some(state) = use_context::<AppState>() else {
        return;
    };
    state.is_loading.set(true);
    let git_status = state.git_status.get();
    let b = if git_status.branch.is_empty() {
        "main".to_string()
    } else {
        git_status.branch
    };
    let args =
        serde_wasm_bindgen::to_value(&serde_json::json!({ "remote": "origin", "branch": b }))
            .unwrap();
    let result = invoke("pull_from_remote", args).await;
    match serde_wasm_bindgen::from_value::<()>(result) {
        Ok(()) => {
            show_notification("Pulled", false);
            let _ = load_status().await;
        }
        Err(e) => {
            log(&format!("Failed: {e:?}"));
            show_notification("Pull failed", true);
        }
    }
    state.is_loading.set(false);
}

async fn set_dry_run_mode(enabled: bool) {
    let Some(state) = use_context::<AppState>() else {
        return;
    };
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "enabled": enabled })).unwrap();
    let result = invoke("set_dry_run_mode", args).await;
    match serde_wasm_bindgen::from_value::<()>(result) {
        Ok(()) => {
            state.dry_run_mode.set(enabled);
        }
        Err(e) => {
            log(&format!("Failed: {e:?}"));
        }
    }
}

async fn validate_config() {
    let Some(state) = use_context::<AppState>() else {
        return;
    };
    state.is_loading.set(true);
    let result = invoke("load_config", JsValue::NULL).await;
    match serde_wasm_bindgen::from_value::<Config>(result) {
        Ok(config) => {
            let args = serde_wasm_bindgen::to_value(
                &serde_json::json!({ "content": config.repo_path.unwrap_or_default() }),
            )
            .unwrap();
            let result = invoke("validate_config", args).await;
            match serde_wasm_bindgen::from_value::<()>(result) {
                Ok(()) => show_notification("Valid", false),
                Err(e) => {
                    log(&format!("Invalid: {e:?}"));
                    show_notification("Invalid config", true);
                }
            }
        }
        Err(e) => {
            log(&format!("Failed: {e:?}"));
        }
    }
    state.is_loading.set(false);
}
