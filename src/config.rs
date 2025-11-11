use crate::error::{DotfilesError, Result};
use crate::types::{FileEntry, SymlinkResolution, TrackedFile};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub repo_path: String,
    pub current_profile: String,
    pub backup_dir: String,
    #[serde(default = "default_symlink_resolution")]
    pub symlink_resolution: String,
    #[serde(default)]
    pub default_remote: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
}

fn default_symlink_resolution() -> String {
    "auto".to_string()
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

impl Config {
    pub fn load() -> Result<Self> {
        // Try to find config file, checking repo first, then system config dir
        let config_path = Self::find_config_path()?;

        if !config_path.exists() {
            // Create default config in system config directory
            let config_dir = dirs::config_dir()
                .ok_or_else(|| {
                    DotfilesError::Config(
                        "What: Cannot find XDG config directory\n  \
                         Why: The XDG_CONFIG_HOME environment variable is not set and default location could not be found\n  \
                         This is typically ~/.config on Linux/macOS\n  \
                         💡 Solution:\n    \
                         - Set XDG_CONFIG_HOME: export XDG_CONFIG_HOME=\"$HOME/.config\"\n    \
                         - Add to your shell config (~/.bashrc, ~/.zshrc, etc.)\n    \
                         - Restart your shell or run: source ~/.bashrc"
                            .to_string(),
                    )
                })?
                .join("flux");
            std::fs::create_dir_all(&config_dir)?;
            create_example_config(&config_dir)?;

            let mut config = Self::default();
            config.validate()?;
            config.save(false)?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(&config_path)?;
        let mut config: Config = toml::from_str(&content)?;

        // Validate the loaded config
        config.validate()?;

        debug!(
            "Configuration loaded and validated successfully from {}",
            config_path.display()
        );
        Ok(config)
    }

    /// Find the configuration file, checking multiple locations in order:
    /// 1. DOTFILES_CONFIG environment variable (if set)
    /// 2. ~/.dotfiles/config.toml (in repository root)
    /// 3. ~/.config/flux/config.toml (XDG standard location)
    fn find_config_path() -> Result<PathBuf> {
        // Check environment variable first
        if let Ok(env_config) = std::env::var("DOTFILES_CONFIG") {
            let env_path = PathBuf::from(&env_config);
            if env_path.exists() {
                debug!("Found config from DOTFILES_CONFIG: {}", env_path.display());
                return Ok(env_path);
            }
        }

        let home = dirs::home_dir()
            .ok_or_else(|| {
                DotfilesError::Config(
                    "What: Could not determine home directory\n  \
                     Why: The $HOME environment variable is not set or home directory lookup failed\n  \
                     This is required for all dotfile operations\n  \
                     💡 Solution:\n    \
                     - Check that $HOME is exported: `echo $HOME`\n    \
                     - If empty, add to your shell config (~/.bashrc, ~/.zshrc, etc.):\n    \
                       export HOME=\"/home/your_username\"\n    \
                     - Restart your shell or run: `source ~/.bashrc`"
                        .to_string(),
                )
            })?;

        // Check for config in default repo location (~/.dotfiles/config.toml)
        let default_repo_config = home.join(".dotfiles").join("config.toml");
        if default_repo_config.exists() {
            debug!(
                "Found config in repository: {}",
                default_repo_config.display()
            );
            return Ok(default_repo_config);
        }

        // Fall back to system config directory
        let config_dir = dirs::config_dir()
            .ok_or_else(|| {
                DotfilesError::Config(
                    "What: Cannot find XDG config directory\n  \
                     Why: The XDG_CONFIG_HOME environment variable is not set and default location could not be found\n  \
                     This is typically ~/.config on Linux/macOS\n  \
                     💡 Solution:\n    \
                     - Set XDG_CONFIG_HOME: export XDG_CONFIG_HOME=\"$HOME/.config\"\n    \
                     - Add to your shell config (~/.bashrc, ~/.zshrc, etc.)\n    \
                     - Restart your shell or run: source ~/.bashrc"
                        .to_string(),
                )
            })?
            .join("flux");

        std::fs::create_dir_all(&config_dir)?;
        create_example_config(&config_dir)?;

        let system_config = config_dir.join("config.toml");
        debug!(
            "Will use system config location: {}",
            system_config.display()
        );
        Ok(system_config)
    }

    /// Validate the configuration for correctness
    fn validate(&mut self) -> Result<()> {
        // Validate symlink resolution mode
        self.general.symlink_resolution = self.general.symlink_resolution.to_lowercase();
        SymlinkResolution::from_str(&self.general.symlink_resolution).map_err(|_| {
            crate::error_utils::invalid_config_value(
                "symlink_resolution",
                &self.general.symlink_resolution,
                &["auto", "relative", "absolute", "follow", "replace"],
                "~/.config/flux/config.toml",
            )
        })?;

        // Validate repo path is not empty
        if self.general.repo_path.is_empty() {
            return Err(DotfilesError::Config(
                "repo_path cannot be empty".to_string(),
            ));
        }

        // Validate backup dir is not empty
        if self.general.backup_dir.is_empty() {
            return Err(DotfilesError::Config(
                "backup_dir cannot be empty".to_string(),
            ));
        }

        // Validate current profile is not empty
        if self.general.current_profile.is_empty() {
            return Err(DotfilesError::Config(
                "current_profile cannot be empty".to_string(),
            ));
        }

        // Warn about potentially problematic configurations
        if self.general.repo_path == self.general.backup_dir {
            warn!("repo_path and backup_dir are the same - this is not recommended");
        }

        // Validate file entries for duplicates
        for (tool, tool_config) in &self.tools {
            let mut seen_dests = std::collections::HashSet::new();
            for entry in &tool_config.files {
                if !seen_dests.insert(entry.dest.clone()) {
                    warn!(
                        "Tool '{}' has duplicate destination path: {}",
                        tool, entry.dest
                    );
                }
            }
        }

        Ok(())
    }

    /// Save the configuration to disk.
    ///
    /// In dry run mode:
    /// - Returns early without writing to disk
    /// - Configuration changes remain in memory only
    pub fn save(&self, is_dry_run: bool) -> Result<()> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find config directory".to_string()))?
            .join("flux");

        if is_dry_run {
            // In dry run mode, don't actually save the config
            return Ok(());
        }

        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn get_tracked_files(&self, profile: Option<&str>) -> Result<Vec<TrackedFile>> {
        let profile = profile.unwrap_or(&self.general.current_profile);
        let home = dirs::home_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
        let repo_path = expand_path(&self.general.repo_path)?;

        let mut tracked = Vec::new();
        let mut processed_dests = std::collections::HashSet::new();

        // First, collect base files (no profile specified)
        for (tool, tool_config) in &self.tools {
            for file_entry in &tool_config.files {
                if file_entry.profile.is_some() {
                    continue; // Skip profile-specific for now
                }

                let repo_file = PathBuf::from(&repo_path).join(&file_entry.repo);
                let dest_file = home.join(&file_entry.dest);
                let dest_key = file_entry.dest.clone();

                tracked.push(TrackedFile {
                    tool: tool.clone(),
                    repo_path: repo_file,
                    dest_path: dest_file,
                    profile: None,
                });
                processed_dests.insert(dest_key);
            }
        }

        // Then, add profile-specific overrides (these override base files)
        for (tool, tool_config) in &self.tools {
            for file_entry in &tool_config.files {
                if let Some(file_profile) = &file_entry.profile {
                    if file_profile != profile {
                        continue; // Not for this profile
                    }

                    let dest_key = file_entry.dest.clone();

                    // Check if this destination is already in base files
                    if processed_dests.contains(&dest_key) {
                        // Override: remove base entry and add profile entry
                        tracked.retain(|t| t.dest_path != home.join(&file_entry.dest));
                    }

                    // Check for profile-specific repo path
                    let repo_file = if file_entry.repo.starts_with("profiles/") {
                        PathBuf::from(&repo_path).join(&file_entry.repo)
                    } else {
                        PathBuf::from(&repo_path)
                            .join("profiles")
                            .join(profile)
                            .join(tool)
                            .join(&file_entry.repo)
                    };

                    let dest_file = home.join(&file_entry.dest);

                    tracked.push(TrackedFile {
                        tool: tool.clone(),
                        repo_path: repo_file,
                        dest_path: dest_file,
                        profile: Some(file_profile.clone()),
                    });
                }
            }
        }

        Ok(tracked)
    }

    pub fn add_file_to_tool(
        &mut self,
        tool: &str,
        repo_path: &str,
        dest_path: &str,
        profile: Option<&str>,
    ) -> Result<()> {
        let tool_config = self
            .tools
            .entry(tool.to_string())
            .or_insert_with(|| ToolConfig { files: Vec::new() });

        let file_entry = FileEntry {
            repo: repo_path.to_string(),
            dest: dest_path.to_string(),
            profile: profile.map(|s| s.to_string()),
        };

        tool_config.files.push(file_entry);
        Ok(())
    }

    pub fn get_symlink_resolution(&self) -> Result<SymlinkResolution> {
        self.general
            .symlink_resolution
            .parse()
            .map_err(DotfilesError::Config)
    }

    pub fn get_repo_path(&self) -> Result<PathBuf> {
        expand_path(&self.general.repo_path)
    }

    pub fn get_backup_dir(&self) -> Result<PathBuf> {
        expand_path(&self.general.backup_dir)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                repo_path: "~/.dotfiles".to_string(),
                current_profile: "default".to_string(),
                backup_dir: "~/.dotfiles-backups".to_string(),
                symlink_resolution: "auto".to_string(),
                default_remote: None,
                default_branch: None,
            },
            tools: HashMap::new(),
        }
    }
}

fn expand_path(path: &str) -> Result<PathBuf> {
    if path.starts_with('~') {
        let home = dirs::home_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
        Ok(home.join(path.strip_prefix("~/").unwrap_or(path)))
    } else {
        Ok(PathBuf::from(path))
    }
}

pub fn create_example_config(config_dir: &PathBuf) -> Result<()> {
    let example_path = config_dir.join("config.toml.example");
    if example_path.exists() {
        return Ok(()); // Already exists
    }

    // Try to copy from project config directory
    if let Ok(project_config) = std::env::current_exe()
        && let Some(project_dir) = project_config.parent()
    {
        let project_example = project_dir.join("config").join("config.toml.example");
        if project_example.exists() {
            std::fs::copy(&project_example, &example_path)?;
            return Ok(());
        }
    }

    Ok(())
}
