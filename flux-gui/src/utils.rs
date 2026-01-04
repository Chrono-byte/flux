//! Utility functions for the Flux GUI.

use dirs::home_dir;
use std::path::PathBuf;

/// Get the flux configuration directory
#[must_use]
pub fn get_flux_config_dir() -> PathBuf {
    if let Ok(config_path) = std::env::var("DOTFILES_CONFIG") {
        return PathBuf::from(config_path);
    }

    if let Some(home) = home_dir() {
        let repo_config = home.join(".dotfiles").join("config.toml");
        if repo_config.exists() {
            return repo_config;
        }

        let xdg_config = home.join(".config").join("flux").join("config.toml");
        if xdg_config.exists() {
            return xdg_config;
        }
    }

    home_dir().map_or_else(
        || PathBuf::from("config.toml"),
        |h| h.join(".config").join("flux").join("config.toml"),
    )
}

/// Get the GUI state directory
#[must_use]
pub fn get_gui_state_dir() -> PathBuf {
    if let Some(home) = home_dir() {
        home.join(".config").join("flux")
    } else {
        PathBuf::from(".config/flux")
    }
}

/// Get the GUI state file path
#[must_use]
pub fn get_gui_state_file() -> PathBuf {
    get_gui_state_dir().join("gui_state.json")
}

/// Create any necessary directories
pub fn ensure_directories() -> anyhow::Result<()> {
    let state_dir = get_gui_state_dir();
    std::fs::create_dir_all(&state_dir)?;
    Ok(())
}

/// Convert a path to a relative path from home directory if possible
#[must_use]
pub fn shorten_path(path: &std::path::Path) -> String {
    if let Some(home) = home_dir()
        && let Ok(relative) = path.strip_prefix(&home)
    {
        return format!("~/{}", relative.display());
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_path() {
        let home = home_dir().expect("home dir");
        let test_path = home.join(".config").join("test.toml");
        let shortened = shorten_path(&test_path);
        assert!(shortened.starts_with("~/"));
    }
}
