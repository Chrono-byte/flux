//! Utility functions for the Flux GUI.

use dirs::home_dir;
use std::path::PathBuf;

/// Get the GUI state directory
#[must_use]
pub fn get_gui_state_dir() -> PathBuf {
    if let Some(home) = home_dir() {
        home.join(".config").join("flux")
    } else {
        PathBuf::from(".config/flux")
    }
}

/// Create any necessary directories
///
/// # Errors
/// This function will return an error if it fails to create any of the necessary directories.
pub fn ensure_directories() -> anyhow::Result<()> {
    let state_dir = get_gui_state_dir();
    std::fs::create_dir_all(&state_dir)?;
    Ok(())
}
