pub mod cli;
pub mod profile;

pub use cli::EnvironmentConfig;

// The config module itself is in this file
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::types::{EnvironmentSpec, FileEntry, PackageSpec, ServiceSpec};
use crate::utils::error::{DotfilesError, Result};

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
    /// List of config files to include and merge (later files override earlier ones)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
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
            include: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub general: GeneralConfig,
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,

    // ==================== New Declarative System Layer ====================
    /// Package declarations (e.g., [packages.git])
    #[serde(default)]
    pub packages: HashMap<String, PackageSpec>,

    /// Service declarations (e.g., [services.ssh])
    #[serde(default)]
    pub services: HashMap<String, ServiceSpec>,

    /// Environment configuration (e.g., [environment])
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<EnvironmentSpec>,
}

impl Config {
    /// Load configuration, checking DOTFILES_CONFIG environment variable if set
    pub fn load() -> Result<Self> {
        // Check for custom config path from environment variable
        let config_path = if let Ok(config_path_str) = std::env::var(cli::env_keys::CONFIG_FILE) {
            PathBuf::from(&config_path_str)
        } else {
            Self::get_config_path()?
        };

        Self::load_from_path(&config_path, &mut Vec::new())
    }

    /// Load configuration with optional custom path (from EnvironmentConfig)
    #[allow(dead_code)] // Reserved for future use when EnvironmentConfig needs to be explicitly passed
    pub fn load_with_env(env_config: Option<&EnvironmentConfig>) -> Result<Self> {
        // Determine config path: env_config > environment variable > default location
        let config_path = if let Some(env) = env_config {
            env.config_file.clone().unwrap_or_else(|| {
                // Fall back to environment variable or default
                if let Ok(config_path_str) = std::env::var(cli::env_keys::CONFIG_FILE) {
                    PathBuf::from(&config_path_str)
                } else {
                    Self::get_config_path().unwrap_or_default()
                }
            })
        } else {
            // No env_config provided, use standard load logic
            if let Ok(config_path_str) = std::env::var(cli::env_keys::CONFIG_FILE) {
                PathBuf::from(&config_path_str)
            } else {
                Self::get_config_path()?
            }
        };

        Self::load_from_path(&config_path, &mut Vec::new())
    }

    /// Load configuration from a specific path, with circular dependency tracking
    fn load_from_path(config_path: &Path, visited: &mut Vec<PathBuf>) -> Result<Self> {
        let config_path = match config_path.canonicalize() {
            Ok(path) => path,
            Err(_) => config_path.to_path_buf(),
        };

        // Check for circular includes
        if visited.contains(&config_path) {
            return Err(DotfilesError::Config(format!(
                "Circular include detected: {}",
                config_path.display()
            )));
        }
        visited.push(config_path.clone());

        if !config_path.exists() {
            // Create default config if this is the primary config file
            if visited.len() == 1 {
                let config = Config::default();
                config.save(false)?;
                return Ok(config);
            } else {
                return Err(DotfilesError::Config(format!(
                    "Included config file does not exist: {}",
                    config_path.display()
                )));
            }
        }

        let content = fs::read_to_string(&config_path).map_err(|e| {
            DotfilesError::Config(format!(
                "Failed to read config {}: {}",
                config_path.display(),
                e
            ))
        })?;

        let mut config: Config = toml::from_str(&content).map_err(|e| {
            DotfilesError::Config(format!(
                "Failed to parse config {}: {}",
                config_path.display(),
                e
            ))
        })?;

        // Load and merge included configs
        // Extract includes list to avoid borrow checker issues
        let includes = config.general.include.clone();
        if let Some(includes) = includes {
            let base_dir = config_path.parent().ok_or_else(|| {
                DotfilesError::Config("Config file has no parent directory".to_string())
            })?;

            for include_path_str in &includes {
                // Resolve relative paths from the current config file's directory
                let include_path =
                    if include_path_str.starts_with('/') || include_path_str.starts_with('~') {
                        // Absolute path or tilde expansion
                        shellexpand::full(include_path_str)
                            .map(|s| PathBuf::from(s.as_ref()))
                            .map_err(|e| {
                                DotfilesError::Config(format!(
                                    "Failed to expand path {}: {}",
                                    include_path_str, e
                                ))
                            })?
                    } else {
                        // Relative path - resolve from the config file's directory
                        base_dir.join(include_path_str)
                    };

                // Canonicalize to check for self-inclusion
                let include_path_canonical = include_path
                    .canonicalize()
                    .unwrap_or_else(|_| include_path.clone());

                let config_path_canonical = config_path
                    .canonicalize()
                    .unwrap_or_else(|_| config_path.clone());

                // Prevent config from including itself
                if include_path_canonical == config_path_canonical {
                    return Err(DotfilesError::Config(format!(
                        "Config file cannot include itself: {}",
                        include_path_str
                    )));
                }

                let included_config = Self::load_from_path(&include_path, visited)?;
                config.merge(&included_config)?;
            }
        }

        // Remove this path from visited set after processing
        visited.pop();

        // Only validate the final merged config
        if visited.is_empty() {
            config.validate()?;
        }

        Ok(config)
    }

    /// Merge another config into this one (later config overrides earlier)
    fn merge(&mut self, other: &Config) -> Result<()> {
        // Merge general config (later overrides earlier for most fields)
        // For include, we don't merge - only the base config's includes are processed
        if !other.general.repo_path.is_empty() {
            self.general.repo_path = other.general.repo_path.clone();
        }
        if !other.general.backup_dir.is_empty() {
            self.general.backup_dir = other.general.backup_dir.clone();
        }
        if !other.general.current_profile.is_empty() {
            self.general.current_profile = other.general.current_profile.clone();
        }
        if !other.general.symlink_resolution.is_empty() {
            self.general.symlink_resolution = other.general.symlink_resolution.clone();
        }
        if other.general.default_remote.is_some() {
            self.general.default_remote = other.general.default_remote.clone();
        }
        if other.general.default_branch.is_some() {
            self.general.default_branch = other.general.default_branch.clone();
        }

        // Merge tools (later entries override earlier ones)
        for (tool_name, tool_config) in &other.tools {
            let entry = self
                .tools
                .entry(tool_name.clone())
                .or_insert_with(|| ToolConfig { files: Vec::new() });
            // Merge file entries (append, allowing duplicates to be filtered later if needed)
            entry.files.extend(tool_config.files.clone());
        }

        // Merge packages (later overrides earlier)
        for (package_name, package_spec) in &other.packages {
            self.packages
                .insert(package_name.clone(), package_spec.clone());
        }

        // Merge services (later overrides earlier)
        for (service_name, service_spec) in &other.services {
            self.services
                .insert(service_name.clone(), service_spec.clone());
        }

        // Merge environment (later overrides earlier, or replaces if None)
        if other.environment.is_some() {
            self.environment = other.environment.clone();
        }

        Ok(())
    }

    pub fn save(&self, validate: bool) -> Result<()> {
        if validate {
            self.validate()?;
        }

        let config_path = Self::get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Use toml_edit to preserve comments and formatting
        let mut doc = if config_path.exists() {
            // Read existing file to preserve comments
            let content = fs::read_to_string(&config_path).map_err(|e| {
                DotfilesError::Config(format!(
                    "Failed to read config {}: {}",
                    config_path.display(),
                    e
                ))
            })?;
            content.parse::<toml_edit::DocumentMut>().map_err(|e| {
                DotfilesError::Config(format!("Failed to parse existing config: {}", e))
            })?
        } else {
            // Create new document
            toml_edit::DocumentMut::new()
        };

        // Serialize current config to TOML and merge into existing document
        let new_toml = toml::to_string_pretty(self)
            .map_err(|e| DotfilesError::Config(format!("Failed to serialize config: {}", e)))?;
        let new_doc = new_toml.parse::<toml_edit::DocumentMut>().map_err(|e| {
            DotfilesError::Config(format!("Failed to parse serialized config: {}", e))
        })?;

        // Merge new values into existing document, preserving comments
        // Recursively merge nested tables to preserve comments in sub-sections
        Self::merge_toml_documents(&mut doc, &new_doc);

        // Write back
        fs::write(&config_path, doc.to_string())?;

        Ok(())
    }

    /// Recursively merge two TOML documents, preserving comments from the original
    fn merge_toml_documents(target: &mut toml_edit::DocumentMut, source: &toml_edit::DocumentMut) {
        for (key, source_item) in source.iter() {
            let target_item = target.get_mut(key);
            match (target_item, source_item) {
                (Some(target_item), toml_edit::Item::Table(_)) => {
                    // Both are tables, merge recursively
                    if let Some(target_table) = target_item.as_table_mut() {
                        if let toml_edit::Item::Table(source_table) = source_item {
                            Self::merge_toml_tables(target_table, source_table);
                        }
                    }
                }
                (Some(_), _) => {
                    // Target exists but source is not a table, replace it
                    target[key] = source_item.clone();
                }
                (None, _) => {
                    // Key doesn't exist in target, add it
                    target[key] = source_item.clone();
                }
            }
        }
    }

    /// Recursively merge two TOML tables
    fn merge_toml_tables(target: &mut toml_edit::Table, source: &toml_edit::Table) {
        for (key, source_item) in source.iter() {
            let target_item = target.get_mut(key);
            match (target_item, source_item) {
                (Some(target_item), toml_edit::Item::Table(_)) => {
                    // Both are tables, merge recursively
                    if let Some(target_table) = target_item.as_table_mut() {
                        if let toml_edit::Item::Table(source_table) = source_item {
                            Self::merge_toml_tables(target_table, source_table);
                        }
                    }
                }
                (Some(_), _) => {
                    // Target exists, replace it with source value
                    target.insert(key, source_item.clone());
                }
                (None, _) => {
                    // Key doesn't exist in target, add it
                    target.insert(key, source_item.clone());
                }
            }
        }
    }

    /// Get the default config path, checking multiple locations in order:
    /// 1. Environment variable DOTFILES_CONFIG (handled in load())
    /// 2. ~/.dotfiles/config.toml (if repo exists)
    /// 3. ~/.config/flux/config.toml (XDG standard location)
    pub fn get_config_path() -> Result<PathBuf> {
        // First, try to check if ~/.dotfiles/config.toml exists
        // We need to check this without loading the config (to avoid circular dependency)
        let home = dirs::home_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
        let repo_config = home.join(".dotfiles").join("config.toml");

        if repo_config.exists() {
            return Ok(repo_config);
        }

        // Fall back to XDG config directory
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
            .or_insert(ToolConfig { files: Vec::new() })
            .files
            .push(entry);

        Ok(())
    }

    pub fn get_tracked_files(
        &self,
        profile: Option<&str>,
    ) -> Result<Vec<crate::types::TrackedFile>> {
        let repo_path = self.get_repo_path()?;
        let current_profile = profile.unwrap_or(&self.general.current_profile);

        let mut tracked_files = Vec::new();

        for (tool, tool_config) in &self.tools {
            for file in &tool_config.files {
                // Include if no profile specified, or if profile matches current_profile or is None
                let include =
                    file.profile.is_none() || file.profile.as_deref() == Some(current_profile);

                if include {
                    // Handle both cases: file.repo may or may not include the tool name prefix
                    let repo_file_path = if file.repo.starts_with(&format!("{}/", tool)) {
                        // file.repo already includes tool name (e.g., "cursor/settings.json")
                        repo_path.join(&file.repo)
                    } else {
                        // file.repo doesn't include tool name (e.g., "config")
                        repo_path.join(tool).join(&file.repo)
                    };
                    let dest_path = dirs::home_dir()
                        .ok_or_else(|| {
                            DotfilesError::Config("Could not find home directory".to_string())
                        })?
                        .join(&file.dest);

                    tracked_files.push(crate::types::TrackedFile {
                        tool: tool.clone(),
                        repo_path: repo_file_path,
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
