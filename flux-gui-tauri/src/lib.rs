use leptos::html::*;
use leptos::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn mount_app() {
    console_error_panic_hook::set_once();

    leptos::mount_to_body(|cx| {
        view! { cx, <App /> }
    });
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq)]
pub enum View {
    Home,
    Files,
    Status,
    Profiles,
    Remotes,
    History,
    Settings,
}

impl std::fmt::Display for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            View::Home => write!(f, "home"),
            View::Files => write!(f, "files"),
            View::Status => write!(f, "status"),
            View::Profiles => write!(f, "profiles"),
            View::Remotes => write!(f, "remotes"),
            View::History => write!(f, "history"),
            View::Settings => write!(f, "settings"),
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
    tauri_available: RwSignal<bool>,
    sync_summary: RwSignal<SyncSummary>,
    git_status: RwSignal<GitStatus>,
    files: RwSignal<Vec<FileReport>>,
    profiles: RwSignal<Vec<String>>,
    remotes: RwSignal<Vec<Remote>>,
    history: RwSignal<Vec<HistoryEvent>>,
    notification: RwSignal<Option<Notification>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            current_view: RwSignal::new(View::Home),
            current_profile: RwSignal::new("default".to_string()),
            repo_path: RwSignal::new("~/.dotfiles".to_string()),
            dry_run_mode: RwSignal::new(false),
            is_loading: RwSignal::new(false),
            tauri_available: RwSignal::new(false),
            sync_summary: RwSignal::new(SyncSummary {
                synced: 0,
                out_of_sync: 0,
                missing: 0,
                broken: 0,
                total: 0,
            }),
            git_status: RwSignal::new(GitStatus {
                branch: "".to_string(),
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
    let state = use_context::<AppState>().unwrap_or_else(|| provide_context(AppState::new()));

    let tauri_available = state.tauri_available;
    let notification = state.notification;

    create_effect(move |_| {
        let available = tauri_available();
        if available {
            log("Tauri API available");
        }
    });

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
    view! {
        <header class="titlebar">
            <div class="titlebar-title">"Flux"</div>
        </header>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let current_view = state.current_view;

    let nav_items = vec![
        (View::Home, "Home"),
        (View::Files, "Files"),
        (View::Status, "Status"),
        (View::Profiles, "Profiles"),
        (View::Remotes, "Remotes"),
        (View::History, "History"),
        (View::Settings, "Settings"),
    ];

    view! {
        <nav class="sidebar">
            <div class="nav-spacer"></div>
            {nav_items.into_iter().map(|(view, label)| {
                let view_clone = view.clone();
                view! {
                    <button
                        class=move || {
                            if current_view() == view {
                                "nav-item active"
                            } else {
                                "nav-item"
                            }
                        }
                        on:click=move |_| current_view.set(view_clone)
                    >
                        {label}
                    </button>
                }
            }).collect::<Vec<_>>()}
            <div class="nav-spacer flex-grow"></div>
        </nav>
    }
}

#[component]
fn MainContent() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let current_view = state.current_view;
    let current_profile = state.current_profile;
    let is_loading = state.is_loading;

    view! {
        <main class="content">
            <Header />
            <div class="views-container">
                {move || match current_view() {
                    View::Home => view! { <HomeView /> }.into_view(),
                    View::Files => view! { <FilesView /> }.into_view(),
                    View::Status => view! { <StatusView /> }.into_view(),
                    View::Profiles => view! { <ProfilesView /> }.into_view(),
                    View::Remotes => view! { <RemotesView /> }.into_view(),
                    View::History => view! { <HistoryView /> }.into_view(),
                    View::Settings => view! { <SettingsView /> }.into_view(),
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

    let view_titles = HashMap::from([
        (View::Home, "Dashboard"),
        (View::Files, "Files"),
        (View::Status, "Sync Status"),
        (View::Profiles, "Profiles"),
        (View::Remotes, "Remotes"),
        (View::History, "History"),
        (View::Settings, "Settings"),
    ]);

    view! {
        <div class="header">
            <h1>{move || view_titles.get(&current_view()).unwrap_or(&"Dashboard").to_string()}</h1>
            <div class="header-right">
                <span class=move || {
                    if is_loading() {
                        "loading"
                    } else {
                        "loading hidden"
                    }
                }>"Loading..."</span>
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
        <div
            id="notification"
            class=move || {
                match notification() {
                    Some(n) if n.is_error => "notification error",
                    Some(_) => "notification success",
                    None => "notification hidden",
                }
            }
        >
            <span id="notification-message">
                {move || notification.as_ref().map(|n| n.message.clone()).unwrap_or_default()}
            </span>
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
    let tauri_available = state.tauri_available;

    create_effect(move |_| {
        if tauri_available() {
            let _ = load_initial_data();
        }
    });

    view! {
        <div id="view-home" class="view active">
            <h2 class="welcome-title">"Welcome to Flux"</h2>
            <p class="repo-path">"Repository: "{repo_path}</p>

            <div class="stat-cards">
                <div class="stat-card">
                    <span class="stat-value synced">{move || sync_summary().synced.to_string()}</span>
                    <span class="stat-label">"Synced"</span>
                </div>
                <div class="stat-card">
                    <span class="stat-value warning">{move || sync_summary().out_of_sync.to_string()}</span>
                    <span class="stat-label">"Out of Sync"</span>
                </div>
                <div class="stat-card">
                    <span class="stat-value muted">{move || sync_summary().missing.to_string()}</span>
                    <span class="stat-label">"Missing"</span>
                </div>
                <div class="stat-card">
                    <span class="stat-value error">{move || sync_summary().broken.to_string()}</span>
                    <span class="stat-label">"Conflicts"</span>
                </div>
            </div>

            <div class="two-column">
                <div class="card">
                    <h3>"Git Status"</h3>
                    <p class="text-secondary">
                        "Branch: "{move || {
                            if git_status().is_repo {
                                git_status().branch
                            } else {
                                "Not a git repository".to_string()
                            }
                        }}
                    </p>
                    <p class=move || {
                        if git_status().has_remote {
                            "text-secondary"
                        } else {
                            "text-secondary hidden"
                        }
                    }>
                        "Ahead: "{move || git_status().ahead.to_string()}" | Behind: "{move || git_status().behind.to_string()}
                    </p>
                </div>
                <div class="card">
                    <h3>"Quick Actions"</h3>
                    <div class="button-group">
                        <button class="btn btn-primary" on:click=sync_all>"Sync All"</button>
                        <button class="btn btn-secondary" on:click=load_home_data>"Refresh"</button>
                    </div>
                </div>
            </div>

            <div class="card">
                <h3>"Recent Activity"</h3>
                <div class="activity-list">
                    {move || {
                        if history().is_empty() {
                            view! { <p class="text-secondary">"No recent activity"</p> }.into_view()
                        } else {
                            history().iter().take(5).map(|event| {
                                let timestamp = parse_timestamp(&event.timestamp);
                                view! {
                                    <div class="activity-item">
                                        {format!("{} - {}", timestamp, event.command)}
                                    </div>
                                }
                            }).collect::<Vec<_>>()
                        }
                    }.into_view()}
                </div>
            </div>
        </div>
    }
}

#[component]
fn FilesView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let files = state.files;
    let tauri_available = state.tauri_available;

    create_effect(move |_| {
        if tauri_available() {
            let _ = load_status();
        }
    });

    view! {
        <div id="view-files" class="view">
            <div class="view-header">
                <p class="text-secondary">{move || format!("{} tracked files", files().len())}</p>
                <button class="btn btn-secondary" on:click=load_status>"Refresh"</button>
            </div>
            <h3>"Tracked Files"</h3>
            <div class="files-list">
                {move || {
                    if files().is_empty() {
                        view! {
                            <div class="empty-state">
                                <p class="text-secondary">"No tracked files. Add files using the CLI: flux add &lt;file&gt;"</p>
                            </div>
                        }.into_view()
                    } else {
                        files().iter().map(|file| {
                            let status_class = get_status_class(&file.status);
                            let status_text = get_status_text(&file.status);
                            view! {
                                <div class="file-item">
                                    <div class=format!("file-status-dot {}", status_class)></div>
                                    <div class="file-info">
                                        <div class="file-path">
                                            {file.dest_path.clone().unwrap_or_default()}
                                        </div>
                                        <div class="file-details">
                                            {format!("{} | {}",
                                                file.tool.clone().unwrap_or_default(),
                                                file.repo_path.clone().unwrap_or_default()
                                            )}
                                        </div>
                                    </div>
                                    <span class=format!("file-status-badge {}", status_class)>
                                        {status_text}
                                    </span>
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }
                }.into_view()}
            </div>
        </div>
    }
}

#[component]
fn StatusView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let sync_summary = state.sync_summary;
    let tauri_available = state.tauri_available;

    create_effect(move |_| {
        if tauri_available() {
            let _ = load_status();
        }
    });

    view! {
        <div id="view-status" class="view">
            <div class="stat-cards">
                <div class="stat-card">
                    <span class="stat-value synced">{move || sync_summary().synced.to_string()}</span>
                    <span class="stat-label">"Synced"</span>
                </div>
                <div class="stat-card">
                    <span class="stat-value warning">{move || sync_summary().out_of_sync.to_string()}</span>
                    <span class="stat-label">"Out of Sync"</span>
                </div>
                <div class="stat-card">
                    <span class="stat-value muted">{move || sync_summary().missing.to_string()}</span>
                    <span class="stat-label">"Missing"</span>
                </div>
                <div class="stat-card">
                    <span class="stat-value error">{move || sync_summary().broken.to_string()}</span>
                    <span class="stat-label">"Conflicts"</span>
                </div>
            </div>
            <div class="button-group">
                <button class="btn btn-primary" on:click=sync_all>"Sync All"</button>
                <button class="btn btn-secondary" on:click=load_status>"Refresh Status"</button>
            </div>
        </div>
    }
}

#[component]
fn ProfilesView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let profiles = state.profiles;
    let current_profile = state.current_profile;
    let tauri_available = state.tauri_available;

    create_effect(move |_| {
        if tauri_available() {
            let _ = load_profiles();
        }
    });

    view! {
        <div id="view-profiles" class="view">
            <p class="text-secondary">"Manage your dotfiles profiles"</p>
            <h3>"Available Profiles"</h3>
            <div class="profiles-list">
                {move || {
                    if profiles().is_empty() {
                        view! {
                            <div class="empty-state">
                                <p class="text-secondary">"No profiles found."</p>
                            </div>
                        }.into_view()
                    } else {
                        profiles().iter().map(|profile| {
                            let is_active = profile == &current_profile();
                            let profile_name = profile.clone();
                            view! {
                                <div class="profile-item">
                                    <span class=format!("profile-name {}", if is_active { "active" } else { "" })>
                                        {profile}{if is_active { " (current)" } else { "" }}
                                    </span>
                                    {if is_active {
                                        view! { <span class="profile-badge active">"Active"</span> }.into_view()
                                    } else {
                                        view! {
                                            <span class="profile-badge switch" data-profile={profile_name.clone()}>
                                                "Switch"
                                            </span>
                                        }.into_view()
                                    }}
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }
                }.into_view()}
            </div>
            <div class="button-group mt-4">
                <button class="btn btn-secondary" on:click=load_profiles>"Refresh Profiles"</button>
            </div>
        </div>
    }
}

#[component]
fn RemotesView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let remotes = state.remotes;
    let tauri_available = state.tauri_available;

    create_effect(move |_| {
        if tauri_available() {
            let _ = load_remotes();
        }
    });

    view! {
        <div id="view-remotes" class="view">
            <p class="text-secondary">"Manage git remotes for your dotfiles repository"</p>

            <div class="card">
                <h3>"Git Operations"</h3>
                <div class="button-group">
                    <button class="btn btn-primary" on:click=push_to_remote>"Push"</button>
                    <button class="btn btn-primary" on:click=pull_from_remote>"Pull"</button>
                    <button class="btn btn-secondary" on:click=load_remotes_and_git>"Refresh"</button>
                </div>
            </div>

            <h3>"Configured Remotes"</h3>
            <div class="remotes-list">
                {move || {
                    if remotes().is_empty() {
                        view! {
                            <div class="empty-state">
                                <p class="text-secondary">"No remotes configured. Add a remote using the CLI: flux remote add &lt;name&gt; &lt;url&gt;"</p>
                            </div>
                        }.into_view()
                    } else {
                        remotes().iter().map(|remote| {
                            view! {
                                <div class="remote-item">
                                    <div class="remote-icon">"git"</div>
                                    <div class="remote-info">
                                        <div class="remote-name">{&remote.name}</div>
                                        <div class="remote-url">{&remote.url}</div>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }
                }.into_view()}
            </div>
        </div>
    }
}

#[component]
fn HistoryView() -> impl IntoView {
    let state = use_context::<AppState>().unwrap();
    let history = state.history;
    let tauri_available = state.tauri_available;

    create_effect(move |_| {
        if tauri_available() {
            let _ = load_history();
        }
    });

    view! {
        <div id="view-history" class="view">
            <p class="text-secondary">"View past operations and changes"</p>
            <div class="button-group">
                <button class="btn btn-secondary" on:click=load_history>"Refresh History"</button>
            </div>
            <div class="history-list">
                {move || {
                    if history().is_empty() {
                        view! {
                            <div class="empty-state">
                                <p class="text-secondary">"No history yet. Operations will appear here as you use Flux."</p>
                            </div>
                        }.into_view()
                    } else {
                        history().iter().map(|event| {
                            let (date_str, time_str) = parse_timestamp_full(&event.timestamp);
                            view! {
                                <div class="history-item">
                                    <div class="history-time">
                                        <span class="history-date">{date_str}</span>
                                        <span class="history-clock">{time_str}</span>
                                    </div>
                                    <div class="history-details">
                                        <div class="history-command">{&event.command}</div>
                                        <div class="history-tags">
                                            <span class=format!("history-tag {}", if event.dry_run { "dry-run" } else { "executed" })>
                                                {if event.dry_run { "Dry Run" } else { "Executed" }}
                                            </span>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }
                }.into_view()}
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
        <div id="view-settings" class="view">
            <p class="text-secondary">"Configure Flux application settings"</p>

            <h3>"General"</h3>
            <div class="card">
                <div class="setting-row">
                    <div>
                        <span class="setting-label">"Repository Path"</span>
                        <span class="setting-value text-secondary">{repo_path}</span>
                    </div>
                </div>
                <div class="setting-row">
                    <div>
                        <span class="setting-label">"Current Profile"</span>
                        <span class="setting-value text-secondary">{current_profile}</span>
                    </div>
                </div>
            </div>

            <h3>"Advanced"</h3>
            <div class="card">
                <div class="setting-row">
                    <div>
                        <span class="setting-label">"Dry Run Mode"</span>
                        <span class="setting-description text-secondary">"Preview changes without applying them"</span>
                    </div>
                    <label class="toggle">
                        <input
                            type="checkbox"
                            checked=dry_run_mode
                            on:change=move |e| {
                                let checked = event_target_checked(&e);
                                dry_run_mode.set(checked);
                                let _ = set_dry_run_mode(checked);
                            }
                        />
                        <span class="toggle-slider"></span>
                    </label>
                </div>
            </div>

            <div class="button-group mt-6">
                <button class="btn btn-secondary" on:click=load_config>"Reload Config"</button>
                <button class="btn btn-secondary" on:click=validate_config>"Validate Config"</button>
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

fn get_status_text(status: &str) -> String {
    match status {
        "synced" => "Synced".to_string(),
        "outOfSync" => "Out of Sync".to_string(),
        "brokenSymlink" => "Broken".to_string(),
        "missingSymlink" => "Missing Link".to_string(),
        "missingRepo" => "Missing".to_string(),
        _ => "Unknown".to_string(),
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
        (ts.to_string(), "".to_string())
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
        leptos::set_timeout(
            move || {
                state_clone.notification.set(None);
            },
            5000,
        );
    }
}

async fn load_initial_data() {
    let _ = load_config().await;
    let _ = load_status().await;
    let _ = load_git_status().await;
    let _ = load_history().await;
}

async fn load_home_data() {
    let _ = load_config().await;
    let _ = load_status().await;
    let _ = load_git_status().await;
    let _ = load_history().await;
}

fn tauri_available() -> bool {
    web_sys::window().and_then(|w| w.get("__TAURI__")).is_some()
}

async fn invoke_command(cmd: &str, args: JsValue) -> Result<JsValue, JsValue> {
    let window = web_sys::window().unwrap();
    let tauri = js_sys::Reflect::get(&window, &"__TAURI__".into())?;
    let core = js_sys::Reflect::get(&tauri, &"core".into())?;
    let invoke_fn = js_sys::Reflect::get(&core, &"invoke".into())?;

    let args_array = js_sys::Array::new();
    args_array.push(&cmd.into());
    args_array.push(&args);

    js_sys::Reflect::apply(&invoke_fn, &core, &args_array)
}

async fn load_config() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };

    state.is_loading.set(true);

    match invoke_command("load_config", JsValue::NULL).await {
        Ok(result) => {
            let config: Config = serde_wasm_bindgen::from_value(result)?;
            if let Some(profile) = config.current_profile.or(config.current_profile_name) {
                state.current_profile.set(profile);
            }
            if let Some(path) = config.repo_path {
                state.repo_path.set(path);
            }
            Ok(())
        }
        Err(e) => {
            log(&format!("Failed to load config: {:?}", e));
            Ok(())
        }
    }?;

    state.is_loading.set(false);
    Ok(())
}

async fn load_status() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };

    state.is_loading.set(true);

    match invoke_command("check_status", JsValue::NULL).await {
        Ok(result) => {
            let reports: Vec<FileReport> = serde_wasm_bindgen::from_value(result)?;
            state.files.set(reports.clone());

            let mut summary = SyncSummary {
                synced: 0,
                out_of_sync: 0,
                missing: 0,
                broken: 0,
                total: reports.len(),
            };

            for report in &reports {
                match report.status.as_str() {
                    "synced" => summary.synced += 1,
                    "outOfSync" => summary.out_of_sync += 1,
                    "missingSymlink" | "missingRepo" => summary.missing += 1,
                    "brokenSymlink" => summary.broken += 1,
                    _ => {}
                }
            }

            state.sync_summary.set(summary);
            Ok(())
        }
        Err(e) => {
            log(&format!("Failed to load status: {:?}", e));
            Ok(())
        }
    }?;

    state.is_loading.set(false);
    Ok(())
}

async fn load_profiles() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };

    state.is_loading.set(true);

    match invoke_command("list_profiles", JsValue::NULL).await {
        Ok(result) => {
            let profiles: Vec<String> = serde_wasm_bindgen::from_value(result)?;
            state.profiles.set(profiles);
            Ok(())
        }
        Err(e) => {
            log(&format!("Failed to load profiles: {:?}", e));
            Ok(())
        }
    }?;

    state.is_loading.set(false);
    Ok(())
}

async fn load_remotes() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };

    state.is_loading.set(true);

    match invoke_command("list_remotes", JsValue::NULL).await {
        Ok(result) => {
            let remotes: Vec<Remote> = serde_wasm_bindgen::from_value(result)?;
            state.remotes.set(remotes);
            Ok(())
        }
        Err(e) => {
            log(&format!("Failed to load remotes: {:?}", e));
            Ok(())
        }
    }?;

    state.is_loading.set(false);
    Ok(())
}

async fn load_git_status() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };

    match invoke_command("get_git_status", JsValue::NULL).await {
        Ok(result) => {
            let git_status: GitStatus = serde_wasm_bindgen::from_value(result)?;
            state.git_status.set(git_status);
            Ok(())
        }
        Err(e) => {
            log(&format!("Failed to load git status: {:?}", e));
            Ok(())
        }
    }
}

async fn load_history() -> Result<(), JsValue> {
    let Some(state) = use_context::<AppState>() else {
        return Ok(());
    };

    state.is_loading.set(true);

    match invoke_command("read_history", JsValue::NULL).await {
        Ok(result) => {
            let history: Vec<HistoryEvent> = serde_wasm_bindgen::from_value(result)?;
            state.history.set(history);
            Ok(())
        }
        Err(e) => {
            log(&format!("Failed to load history: {:?}", e));
            Ok(())
        }
    }?;

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

    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"profile".into(), &JsValue::NULL).unwrap();
    js_sys::Reflect::set(&args, &"dryRun".into(), &state.dry_run_mode.get().into()).unwrap();
    js_sys::Reflect::set(&args, &"force".into(), &false.into()).unwrap();

    match invoke_command("apply_config", args.into()).await {
        Ok(_) => {
            show_notification(
                if state.dry_run_mode.get() {
                    "Dry run completed"
                } else {
                    "Configuration applied successfully"
                },
                false,
            );
            let _ = load_status().await;
        }
        Err(e) => {
            log(&format!("Failed to sync: {:?}", e));
            show_notification(&format!("Failed to sync: {:?}", e), true);
        }
    }

    state.is_loading.set(false);
}

async fn switch_profile(profile_name: String) {
    let Some(state) = use_context::<AppState>() else {
        return;
    };

    state.is_loading.set(true);

    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"name".into(), &profile_name.clone().into()).unwrap();

    match invoke_command("switch_profile", args.into()).await {
        Ok(_) => {
            state.current_profile.set(profile_name.clone());
            show_notification(&format!("Switched to profile: {}", profile_name), false);
            let _ = load_status().await;
        }
        Err(e) => {
            log(&format!("Failed to switch profile: {:?}", e));
            show_notification(&format!("Failed to switch profile: {:?}", e), true);
        }
    }

    state.is_loading.set(false);
}

async fn push_to_remote() {
    let Some(state) = use_context::<AppState>() else {
        return;
    };

    state.is_loading.set(true);

    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"remote".into(), &"origin".into()).unwrap();
    let branch = state.git_status.get().branch.as_str().unwrap_or("main");
    js_sys::Reflect::set(&args, &"branch".into(), &branch.into()).unwrap();

    match invoke_command("push_to_remote", args.into()).await {
        Ok(_) => {
            show_notification("Pushed to remote successfully", false);
        }
        Err(e) => {
            log(&format!("Failed to push: {:?}", e));
            show_notification(&format!("{}", e), true);
        }
    }

    state.is_loading.set(false);
}

async fn pull_from_remote() {
    let Some(state) = use_context::<AppState>() else {
        return;
    };

    state.is_loading.set(true);

    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"remote".into(), &"origin".into()).unwrap();
    let branch = state.git_status.get().branch.as_str().unwrap_or("main");
    js_sys::Reflect::set(&args, &"branch".into(), &branch.into()).unwrap();

    match invoke_command("pull_from_remote", args.into()).await {
        Ok(_) => {
            show_notification("Pulled from remote successfully", false);
            let _ = load_status().await;
        }
        Err(e) => {
            log(&format!("Failed to pull: {:?}", e));
            show_notification(&format!("{}", e), true);
        }
    }

    state.is_loading.set(false);
}

async fn set_dry_run_mode(enabled: bool) {
    let Some(state) = use_context::<AppState>() else {
        return;
    };

    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"enabled".into(), &enabled.into()).unwrap();

    match invoke_command("set_dry_run_mode", args.into()).await {
        Ok(_) => {
            state.dry_run_mode.set(enabled);
        }
        Err(e) => {
            log(&format!("Failed to set dry run mode: {:?}", e));
            show_notification(&format!("Failed to update settings: {:?}", e), true);
        }
    }
}

async fn validate_config() {
    let Some(state) = use_context::<AppState>() else {
        return;
    };

    state.is_loading.set(true);

    match invoke_command("load_config", JsValue::NULL).await {
        Ok(result) => {
            let config: Config = serde_wasm_bindgen::from_value(result).unwrap_or_default();
            let args = js_sys::Object::new();
            if let Some(content) = config.repo_path {
                js_sys::Reflect::set(&args, &"content".into(), &content.into()).unwrap();
            }

            match invoke_command("validate_config", args.into()).await {
                Ok(_) => {
                    show_notification("Configuration is valid", false);
                }
                Err(e) => {
                    log(&format!("Config validation failed: {:?}", e));
                    show_notification(&format!("Configuration has errors: {:?}", e), true);
                }
            }
        }
        Err(e) => {
            log(&format!("Failed to validate config: {:?}", e));
            show_notification(&format!("Failed to validate config: {:?}", e), true);
        }
    }

    state.is_loading.set(false);
}

#[component]
pub fn Title() -> impl IntoView {
    view! { <title>"Flux - Dotfiles Manager"</title> }
}

#[component]
pub fn MountApp() -> impl IntoView {
    leptos::mount_to_body(|| {
        view! { <App /> }
    });

    view! {}
}
