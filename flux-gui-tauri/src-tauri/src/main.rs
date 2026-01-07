//! Flux GUI - A graphical frontend for the Flux dotfiles manager.
//!
//! This Tauri application provides a native desktop GUI for managing dotfiles
//! using the Flux library.

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::AppState;
use std::sync::Mutex;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize application state
            let app_state = AppState::new();
            app.manage(Mutex::new(app_state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_config,
            commands::save_config,
            commands::check_status,
            commands::list_profiles,
            commands::create_profile,
            commands::switch_profile,
            commands::get_tracked_files,
            commands::list_remotes,
            commands::add_remote,
            commands::remove_remote,
            commands::set_remote_url,
            commands::push_to_remote,
            commands::pull_from_remote,
            commands::read_history,
            commands::get_git_status,
            commands::add_file,
            commands::remove_file,
            commands::apply_config,
            commands::validate_config,
            commands::get_app_state,
            commands::set_dry_run_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
