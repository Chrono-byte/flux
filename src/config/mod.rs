pub mod cli;
pub mod profile;

pub use cli::EnvironmentConfig;

// The config module itself is in this file
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::types::{EnvironmentSpec, FileEntry, PackageSpec, ServiceSpec, SymlinkResolution};
use crate::utils::error::{DotfilesError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub repo_path: String,
    pub backup_dir: String,
    pub current_profile: String,
    #[serde(default = "default_symlink_resolution")]
    pub symlink_resolution: SymlinkResolution,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    /// List of config files to include and merge (later files override earlier ones)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
}

fn default_symlink_resolution() -> SymlinkResolution {
    SymlinkResolution::Auto
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            repo_path: "~/.dotfiles".to_string(),
            backup_dir: "~/.dotfiles-backup".to_string(),
            current_profile: "default".to_string(),
            symlink_resolution: SymlinkResolution::Auto,
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
    /// XDG config (~/.config/flux/config.toml) is authoritative and will overwrite repo version
    pub fn load() -> Result<Self> {
        // Check for custom config path from environment variable
        let config_path = if let Ok(config_path_str) = std::env::var(cli::env_keys::CONFIG_FILE) {
            PathBuf::from(&config_path_str)
        } else {
            // Check XDG config first (authoritative)
            let xdg_config = Self::get_xdg_config_path()?;
            if xdg_config.exists() {
                // XDG config exists - use it and overwrite repo version
                let config = Self::load_from_path(&xdg_config, &mut Vec::new())?;
                // Overwrite repo version with XDG contents
                let repo_config = Self::get_repo_config_path()?;
                // Create parent directory if needed
                if let Some(parent) = repo_config.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        DotfilesError::Config(format!(
                            "Failed to create repo config directory: {}",
                            e
                        ))
                    })?;
                }
                // Copy XDG config to repo to keep them in sync
                fs::copy(&xdg_config, &repo_config).map_err(|e| {
                    DotfilesError::Config(format!("Failed to copy XDG config to repo: {}", e))
                })?;
                return Ok(config);
            }

            // Fall back to repo config or default location
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

    /// Load configuration from a specific path
    /// No merging - configs are loaded as-is with precedence order
    fn load_from_path(config_path: &Path, _visited: &mut Vec<PathBuf>) -> Result<Self> {
        let config_path = match config_path.canonicalize() {
            Ok(path) => path,
            Err(_) => config_path.to_path_buf(),
        };

        if !config_path.exists() {
            // Create default config if it doesn't exist
            let config = Config::default();
            config.save(false)?;
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path).map_err(|e| {
            DotfilesError::Config(format!(
                "Failed to read config {}: {}",
                config_path.display(),
                e
            ))
        })?;

        let config: Config = toml::from_str(&content).map_err(|e| {
            DotfilesError::Config(format!(
                "Failed to parse config {}: {}",
                config_path.display(),
                e
            ))
        })?;

        // Validate the config
        config.validate()?;

        Ok(config)
    }

    pub fn save(&self, validate: bool) -> Result<()> {
        if validate {
            self.validate()?;
        }

        // Save to authoritative location: XDG config if it exists, otherwise repo config
        let xdg_config = Self::get_xdg_config_path()?;
        let config_path = if xdg_config.exists() {
            xdg_config
        } else {
            Self::get_config_path()?
        };

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
        let mut new_doc = new_toml.parse::<toml_edit::DocumentMut>().map_err(|e| {
            DotfilesError::Config(format!("Failed to parse serialized config: {}", e))
        })?;

        // Manually format tools section to use per-tool format [tools.X] files = [...]
        // instead of array-of-tables [[tools.X.files]]
        Self::format_tools_section(&mut new_doc, &self.tools);

        // Remove old array-of-tables format from existing document if present
        Self::remove_old_tools_format(&mut doc);

        // Merge new values into existing document, preserving comments
        // Recursively merge nested tables to preserve comments in sub-sections
        Self::merge_toml_documents(&mut doc, &new_doc);

        // Write back
        fs::write(&config_path, doc.to_string())?;

        // If we saved to XDG config, also update repo version
        let xdg_config = Self::get_xdg_config_path()?;
        if config_path == xdg_config {
            let repo_config = Self::get_repo_config_path()?;
            // Create parent directory if needed
            if let Some(parent) = repo_config.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    DotfilesError::Config(format!("Failed to create repo config directory: {}", e))
                })?;
            }
            // Copy XDG config to repo to keep them in sync
            fs::copy(&xdg_config, &repo_config).map_err(|e| {
                DotfilesError::Config(format!("Failed to copy XDG config to repo: {}", e))
            })?;
        }

        Ok(())
    }

    /// Format tools section to use per-tool format [tools.X] files = [...]
    /// instead of array-of-tables [[tools.X.files]]
    fn format_tools_section(doc: &mut toml_edit::DocumentMut, tools: &HashMap<String, ToolConfig>) {
        use toml_edit::{Array, Item, Table, Value};

        // Remove existing tools section if present
        doc.remove("tools");

        // Create new tools table
        let mut tools_table = Table::new();
        tools_table.set_implicit(true);

        for (tool_name, tool_config) in tools {
            // Create table for this tool
            let mut tool_table = Table::new();
            tool_table.set_implicit(true);

            // Create array of file entries
            let mut files_array = Array::new();
            files_array.set_trailing_comma(true);
            files_array.set_trailing("\n");

            for file_entry in &tool_config.files {
                let mut file_table = toml_edit::InlineTable::new();

                // Add repo field
                file_table.insert(
                    "repo",
                    Value::String(toml_edit::Formatted::new(file_entry.repo.clone())),
                );

                // Add dest field
                file_table.insert(
                    "dest",
                    Value::String(toml_edit::Formatted::new(file_entry.dest.clone())),
                );

                // Add profile field if present
                if let Some(profile) = &file_entry.profile {
                    file_table.insert(
                        "profile",
                        Value::String(toml_edit::Formatted::new(profile.clone())),
                    );
                }

                files_array.push_formatted(Value::InlineTable(file_table));
            }

            tool_table.insert("files", Item::Value(Value::Array(files_array)));
            tools_table.insert(tool_name, Item::Table(tool_table));
        }

        doc.insert("tools", Item::Table(tools_table));
    }

    /// Remove old array-of-tables format [[tools.X.files]] from document
    fn remove_old_tools_format(doc: &mut toml_edit::DocumentMut) {
        // Array-of-tables like [[tools.cursor.files]] are represented as arrays
        // in the parsed document. We need to find and remove these.
        // Get all keys that match the pattern tools.*.files
        let keys_to_remove: Vec<String> = doc
            .iter()
            .filter_map(|(key, _)| {
                // Check for keys like "tools.cursor.files" (array-of-tables format)
                if key.starts_with("tools.") && key.ends_with(".files") {
                    Some(key.to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            doc.remove(&key);
        }
    }

    /// Recursively merge two TOML documents, preserving comments from the original
    fn merge_toml_documents(target: &mut toml_edit::DocumentMut, source: &toml_edit::DocumentMut) {
        for (key, source_item) in source.iter() {
            let target_item = target.get_mut(key);
            match (target_item, source_item) {
                (Some(target_item), toml_edit::Item::Table(_)) => {
                    // Both are tables, merge recursively
                    if let Some(target_table) = target_item.as_table_mut()
                        && let toml_edit::Item::Table(source_table) = source_item
                    {
                        Self::merge_toml_tables(target_table, source_table);
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
                    if let Some(target_table) = target_item.as_table_mut()
                        && let toml_edit::Item::Table(source_table) = source_item
                    {
                        Self::merge_toml_tables(target_table, source_table);
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
    /// 2. ~/.config/flux/config.toml (XDG standard location) - authoritative
    /// 3. ~/.dotfiles/config.toml (if repo exists) - fallback
    pub fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find config directory".to_string()))?;
        let xdg_config = config_dir.join("flux/config.toml");

        // XDG config is authoritative - prefer it if it exists
        if xdg_config.exists() {
            return Ok(xdg_config);
        }

        // Fall back to repo config
        let home = dirs::home_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
        let repo_config = home.join(".dotfiles").join("config.toml");

        if repo_config.exists() {
            return Ok(repo_config);
        }

        // If neither exists, default to XDG location
        Ok(xdg_config)
    }

    /// Get the XDG config path (authoritative location)
    fn get_xdg_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find config directory".to_string()))?;
        Ok(config_dir.join("flux/config.toml"))
    }

    /// Get the repo config path
    fn get_repo_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
        Ok(home.join(".dotfiles").join("config.toml"))
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


    /// Sync XDG config to repo (overwrite repo config with XDG config)
    /// This is useful for manually forcing the sync when XDG config is authoritative
    pub fn sync_xdg_to_repo(dry_run: bool) -> Result<()> {
        let xdg_config = Self::get_xdg_config_path()?;
        let repo_config = Self::get_repo_config_path()?;

        if !xdg_config.exists() {
            return Err(DotfilesError::Config(
                "XDG config does not exist. Nothing to sync.".to_string(),
            ));
        }

        if dry_run {
            println!(
                "{} [DRY RUN] Would copy {} to {}",
                "⊘".yellow(),
                xdg_config.display(),
                repo_config.display()
            );
            return Ok(());
        }

        // Create parent directory if needed
        if let Some(parent) = repo_config.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                DotfilesError::Config(format!("Failed to create repo config directory: {}", e))
            })?;
        }

        // Copy XDG config to repo
        fs::copy(&xdg_config, &repo_config).map_err(|e| {
            DotfilesError::Config(format!("Failed to copy XDG config to repo: {}", e))
        })?;

        println!(
            "{} Synced XDG config to repo: {} -> {}",
            "✓".green(),
            xdg_config.display(),
            repo_config.display()
        );

        Ok(())
    }
}
