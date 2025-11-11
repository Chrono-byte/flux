pub mod cli;
pub mod profile;

pub use cli::EnvironmentConfig;

// The config module itself is in this file
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::error::{DotfilesError, Result};
use crate::types::FileEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub repo_path: String,
    pub backup_dir: String,
    pub current_profile: String,
    pub symlink_resolution: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            repo_path: "~/.dotfiles".to_string(),
            backup_dir: "~/.dotfiles-backup".to_string(),
            current_profile: "default".to_string(),
            symlink_resolution: "auto".to_string(),
            default_remote: None,
            default_branch: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            tools: HashMap::new(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            // Create default config
            let config = Config::default();
            config.save(false)?;
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| DotfilesError::Config(format!("Failed to read config: {}", e)))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| DotfilesError::Config(format!("Failed to parse config: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, validate: bool) -> Result<()> {
        if validate {
            self.validate()?;
        }

        let config_path = Self::get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| DotfilesError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(&config_path, content)?;

        Ok(())
    }

    pub fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find config directory".to_string()))?;
        Ok(config_dir.join("flux/config.toml"))
    }

    pub fn get_repo_path(&self) -> Result<PathBuf> {
        let expanded = shellexpand::tilde(&self.general.repo_path).into_owned();
        Ok(PathBuf::from(expanded))
    }

    pub fn get_backup_dir(&self) -> Result<PathBuf> {
        let expanded = shellexpand::tilde(&self.general.backup_dir).into_owned();
        Ok(PathBuf::from(expanded))
    }

    pub fn validate(&self) -> Result<()> {
        if self.general.repo_path.is_empty() {
            return Err(DotfilesError::Config(
                "repo_path cannot be empty".to_string(),
            ));
        }
        if self.general.backup_dir.is_empty() {
            return Err(DotfilesError::Config(
                "backup_dir cannot be empty".to_string(),
            ));
        }
        if self.general.current_profile.is_empty() {
            return Err(DotfilesError::Config(
                "current_profile cannot be empty".to_string(),
            ));
        }

        // Validate profile name (alphanumeric, underscore, hyphen)
        if !self
            .general
            .current_profile
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(DotfilesError::Config(format!(
                "Invalid profile name '{}': only alphanumeric, underscore, and hyphen allowed",
                self.general.current_profile
            )));
        }

        // Validate symlink resolution (normalized to lowercase)
        let symlink_resolution_lower = self.general.symlink_resolution.to_lowercase();
        let valid_modes = vec!["auto", "relative", "absolute", "follow", "replace"];
        if !valid_modes.contains(&symlink_resolution_lower.as_str()) {
            return Err(DotfilesError::Config(format!(
                "Invalid symlink_resolution '{}': must be one of {:?}",
                self.general.symlink_resolution, valid_modes
            )));
        }

        Ok(())
    }

    pub fn add_file_to_tool(
        &mut self,
        tool: &str,
        repo_file: &str,
        dest_path: &Path,
        profile: Option<&str>,
    ) -> Result<()> {
        let dest_str = dest_path.to_string_lossy().to_string();

        let entry = FileEntry {
            repo: repo_file.to_string(),
            dest: dest_str,
            profile: profile.map(|p| p.to_string()),
        };

        self.tools
            .entry(tool.to_string())
            .or_insert(ToolConfig {
                files: Vec::new(),
            })
            .files
            .push(entry);

        Ok(())
    }

    pub fn get_tracked_files(&self, profile: Option<&str>) -> Result<Vec<crate::types::TrackedFile>> {
        let repo_path = self.get_repo_path()?;
        let current_profile = profile.unwrap_or(&self.general.current_profile);

        let mut tracked_files = Vec::new();

        for (tool, tool_config) in &self.tools {
            for file in &tool_config.files {
                // Include if no profile specified, or if profile matches current_profile or is None
                let include = file.profile.is_none() || file.profile.as_deref() == Some(current_profile);

                if include {
                    let repo_path = repo_path.join(tool).join(&file.repo);
                    let dest_path = dirs::home_dir()
                        .ok_or_else(|| {
                            DotfilesError::Config("Could not find home directory".to_string())
                        })?
                        .join(&file.dest);

                    tracked_files.push(crate::types::TrackedFile {
                        tool: tool.clone(),
                        repo_path,
                        dest_path,
                        profile: file.profile.clone(),
                    });
                }
            }
        }

        Ok(tracked_files)
    }

    pub fn get_symlink_resolution(&self) -> Result<crate::types::SymlinkResolution> {
        self.general
            .symlink_resolution
            .parse()
            .map_err(|e: String| DotfilesError::Config(e))
    }
}
