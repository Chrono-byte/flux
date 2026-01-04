//! Flux - A dotfiles manager GUI built with GPUI.

mod app;
mod async_executor;
mod components;
mod events;
mod state;
mod styles;
mod utils;
mod views;

use app::FluxApp;
use dirs::home_dir;
use gpui::{
    AppContext, Application, Bounds, SharedString, TitlebarOptions, WindowBackgroundAppearance,
    WindowDecorations, WindowOptions, px, size,
};
use std::path::PathBuf;

fn main() {
    env_logger::init();

    // Ensure directories exist
    if let Err(e) = utils::ensure_directories() {
        eprintln!("Failed to create necessary directories: {e}");
    }

    // Get repository path (default to ~/.dotfiles)
    let repo_path = home_dir().map_or_else(|| PathBuf::from(".dotfiles"), |h| h.join(".dotfiles"));

    Application::new().run(move |cx| {
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Opaque,
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Flux")),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| FluxApp::new(repo_path.clone(), _cx)),
        )
        .expect("Failed to open window");
        cx.activate(true);
    });
}
