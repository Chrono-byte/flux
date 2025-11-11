use crate::error::{DotfilesError, Result};
use std::path::PathBuf;

pub struct BrowserProfile {
    pub name: String,
    pub profile_path: PathBuf,
    pub key_files: Vec<&'static str>,
}

pub fn detect_firefox_profiles() -> Result<Vec<BrowserProfile>> {
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
    let firefox_dir = home.join(".mozilla").join("firefox");

    if !firefox_dir.exists() {
        return Ok(Vec::new());
    }

    let mut profiles = Vec::new();

    // Look for profiles.ini to find all profiles
    let profiles_ini = firefox_dir.join("profiles.ini");
    if profiles_ini.exists() {
        // Parse profiles.ini to find profile directories
        if let Ok(content) = std::fs::read_to_string(&profiles_ini) {
            let mut current_profile: Option<String> = None;
            let mut current_path: Option<String> = None;

            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("Name=") {
                    if let Some(name) = current_profile.take()
                        && let Some(path) = current_path.take()
                    {
                        // Only add default profile
                        if name.contains("default") {
                            let profile_path = if path.starts_with('/') {
                                PathBuf::from(path)
                            } else {
                                firefox_dir.join(path)
                            };
                            profiles.push(BrowserProfile {
                                name: "firefox-default".to_string(),
                                profile_path,
                                key_files: vec![
                                    "prefs.js",
                                    "user.js",
                                    "places.sqlite",
                                    "extensions",
                                    "storage",
                                ],
                            });
                            break; // Found default, stop looking
                        }
                    }
                    current_profile = Some(line.strip_prefix("Name=").unwrap_or("").to_string());
                } else if line.starts_with("Path=") {
                    current_path = Some(line.strip_prefix("Path=").unwrap_or("").to_string());
                } else if line.starts_with("[Profile") {
                    // Reset for new profile section
                    if let Some(name) = current_profile.take()
                        && let Some(path) = current_path.take()
                    {
                        // Only add default profile
                        if name.contains("default") {
                            let profile_path = if path.starts_with('/') {
                                PathBuf::from(path)
                            } else {
                                firefox_dir.join(path)
                            };
                            profiles.push(BrowserProfile {
                                name: "firefox-default".to_string(),
                                profile_path,
                                key_files: vec![
                                    "prefs.js",
                                    "user.js",
                                    "places.sqlite",
                                    "extensions",
                                    "storage",
                                ],
                            });
                            break; // Found default, stop looking
                        }
                    }
                }
            }

            // Handle last profile (if default)
            if let Some(name) = current_profile
                && name.contains("default")
                && let Some(path) = current_path
            {
                let profile_path = if path.starts_with('/') {
                    PathBuf::from(path)
                } else {
                    firefox_dir.join(path)
                };
                profiles.push(BrowserProfile {
                    name: "firefox-default".to_string(),
                    profile_path,
                    key_files: vec![
                        "prefs.js",
                        "user.js",
                        "places.sqlite",
                        "extensions",
                        "storage",
                    ],
                });
            }
        }
    }

    // Fallback: look for directories matching default profile pattern
    if profiles.is_empty()
        && let Ok(entries) = std::fs::read_dir(&firefox_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                if dir_name.contains("default") && !dir_name.starts_with('.') {
                    profiles.push(BrowserProfile {
                        name: "firefox-default".to_string(),
                        profile_path: path,
                        key_files: vec![
                            "prefs.js",
                            "user.js",
                            "places.sqlite",
                            "extensions",
                            "storage",
                        ],
                    });
                    break; // Found default, stop
                }
            }
        }
    }

    Ok(profiles)
}

pub fn detect_zen_profiles() -> Result<Vec<BrowserProfile>> {
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;

    // Try common Zen browser locations
    let possible_dirs = vec![
        home.join(".zenbrowser"),
        home.join(".config").join("zenbrowser"),
        home.join(".local").join("share").join("zenbrowser"),
    ];

    let mut profiles = Vec::new();

    for zen_dir in possible_dirs {
        if zen_dir.exists() {
            // Look for profiles directory
            let profiles_dir = zen_dir.join("profiles");
            if profiles_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let dir_name = path.file_name().unwrap().to_string_lossy();
                            profiles.push(BrowserProfile {
                                name: format!("zen-{}", dir_name),
                                profile_path: path,
                                key_files: vec![
                                    "prefs.js",
                                    "user.js",
                                    "places.sqlite",
                                    "extensions",
                                    "storage",
                                ],
                            });
                        }
                    }
                }
            } else {
                // Single profile in root directory - treat as default
                profiles.push(BrowserProfile {
                    name: "zen-default".to_string(),
                    profile_path: zen_dir.clone(),
                    key_files: vec![
                        "prefs.js",
                        "user.js",
                        "places.sqlite",
                        "extensions",
                        "storage",
                    ],
                });
            }
            break; // Found a directory, stop looking
        }
    }

    // Filter to only default profile if multiple found
    profiles.retain(|p| p.name.contains("default"));

    Ok(profiles)
}

pub fn get_browser_profile_files(profile: &BrowserProfile) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();

    // Add key files and directories
    for key_file in &profile.key_files {
        let source = profile.profile_path.join(key_file);
        if source.exists() {
            // Determine destination based on browser type
            // For default profiles, we need to find the actual profile directory name
            let dest = if profile.name.starts_with("firefox-") {
                // Extract actual profile directory from profile_path
                if let Some(profile_dir_name) = profile.profile_path.file_name() {
                    format!(
                        ".mozilla/firefox/{}/{}",
                        profile_dir_name.to_string_lossy(),
                        key_file
                    )
                } else {
                    format!(".mozilla/firefox/default/{}", key_file)
                }
            } else if profile.name.starts_with("zen-") {
                // Extract actual profile directory from profile_path
                if let Some(profile_dir_name) = profile.profile_path.file_name() {
                    format!(
                        ".zenbrowser/profiles/{}/{}",
                        profile_dir_name.to_string_lossy(),
                        key_file
                    )
                } else {
                    format!(".zenbrowser/default/{}", key_file)
                }
            } else {
                continue;
            };

            files.push((source, dest));
        }
    }

    files
}

pub fn detect_alacritty_configs() -> Result<Vec<(PathBuf, String)>> {
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;

    let mut configs = Vec::new();

    // Check for config in ~/.config/alacritty/ (most common location)
    let config_dir = home.join(".config").join("alacritty");
    let possible_configs = vec![
        config_dir.join("alacritty.toml"),
        config_dir.join("alacritty.yml"),
    ];

    for config_path in possible_configs {
        if config_path.exists() {
            // Determine destination path relative to home
            let dest = if let Ok(relative) = config_path.strip_prefix(&home) {
                format!(".{}", relative.to_string_lossy().replace('\\', "/"))
            } else {
                // Fallback: construct expected path
                if config_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .ends_with(".toml")
                {
                    ".config/alacritty/alacritty.toml".to_string()
                } else {
                    ".config/alacritty/alacritty.yml".to_string()
                }
            };

            configs.push((config_path, dest));
            break; // Only use the first found config (prefer toml over yml)
        }
    }

    // Fallback: check legacy location ~/.alacritty.yml
    if configs.is_empty() {
        let legacy_config = home.join(".alacritty.yml");
        if legacy_config.exists() {
            configs.push((legacy_config, ".alacritty.yml".to_string()));
        }
    }

    Ok(configs)
}

pub fn detect_starship_configs() -> Result<Vec<(PathBuf, String)>> {
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;

    let mut configs = Vec::new();

    // Check for config in ~/.config/starship.toml (standard location)
    let config_path = home.join(".config").join("starship.toml");

    if config_path.exists() {
        // Determine destination path relative to home
        let dest = if let Ok(relative) = config_path.strip_prefix(&home) {
            format!(".{}", relative.to_string_lossy().replace('\\', "/"))
        } else {
            ".config/starship.toml".to_string()
        };

        configs.push((config_path, dest));
    }

    Ok(configs)
}
