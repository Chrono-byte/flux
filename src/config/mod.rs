/// Command-line interface configuration.
pub mod cli;
/// Profile management functionality.
pub mod profile;

pub use cli::EnvironmentConfig;

// The config module itself is in this file
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::types::{ConditionSpec, FileEntry, SymlinkResolution};
use crate::utils::error::{DotfilesError, Result};

/// General configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Repository path (supports ~ expansion)
    pub repo_path: String,
    /// Backup directory path (supports ~ expansion, optional - defaults to ~/.local/share/flux/backups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_dir: Option<String>,
    /// Currently active profile name
    pub current_profile: String,
    /// Symlink resolution strategy
    #[serde(default = "default_symlink_resolution")]
    pub symlink_resolution: SymlinkResolution,
    /// Default git remote name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_remote: Option<String>,
    /// Default git branch name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    /// Push timeout in seconds (default: 60)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_timeout: Option<u64>,
    /// Config files to include and merge (later files override earlier ones)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
}

const fn default_symlink_resolution() -> SymlinkResolution {
    SymlinkResolution::Auto
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            repo_path: "~/.dotfiles".to_string(),
            backup_dir: None,
            current_profile: "default".to_string(),
            symlink_resolution: SymlinkResolution::Auto,
            default_remote: None,
            default_branch: None,
            push_timeout: None,
            include: None,
        }
    }
}

/// Configuration for a single tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Files tracked for this tool
    pub files: Vec<FileEntry>,
}

/// Configuration for the Flux dotfiles manager.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// General configuration
    pub general: GeneralConfig,
    /// Tools configuration
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,

    /// Hooks configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<crate::types::HooksConfig>,
}

/// Static storage for config path override from CLI
static CONFIG_PATH_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

impl Config {
    /// Set the config path override (called from CLI parsing)
    pub fn set_path_override(path: &Path) {
        let _ = CONFIG_PATH_OVERRIDE.set(path.to_path_buf());
    }
    /// Load configuration from default locations.
    ///
    /// Checks `DOTFILES_CONFIG` environment variable first, then repo config,
    /// then XDG config as fallback. Repo config is the source of truth.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file cannot be loaded.
    pub fn load() -> Result<Self> {
        // Check for static override first, then fall back to environment variable
        let override_path = CONFIG_PATH_OVERRIDE.get().map(PathBuf::as_path);
        Self::load_with_override(override_path)
    }

    /// Load configuration from default locations with optional path override.
    ///
    /// Checks `config_path_override` parameter first, then `DOTFILES_CONFIG` environment variable,
    /// then repo config, then XDG config as fallback. Repo config is the source of truth.
    ///
    /// # Parameters
    ///
    /// * `config_path_override`: Optional path to configuration file (overrides environment variable)
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file cannot be loaded.
    pub fn load_with_override(config_path_override: Option<&Path>) -> Result<Self> {
        // Check for custom config path from parameter or environment variable
        let config_path = if let Some(override_path) = config_path_override {
            override_path.to_path_buf()
        } else if let Ok(config_path_str) = std::env::var(cli::env_keys::CONFIG_FILE) {
            PathBuf::from(&config_path_str)
        } else {
            // Use repo config as source of truth, fall back to XDG
            Self::get_config_path()?
        };

        Self::load_from_path(&config_path, &mut Vec::new())
    }

    /// Load configuration from a specific path.
    ///
    /// Supports includes with cycle-safe merging. Later configs override earlier ones.
    fn load_from_path(config_path: &Path, visited: &mut Vec<PathBuf>) -> Result<Self> {
        let config_path = config_path.canonicalize().unwrap_or_else(|_| config_path.to_path_buf());

        // Detect include cycles
        if visited.contains(&config_path) {
            let mut chain: Vec<String> =
                visited.iter().map(|path| path.display().to_string()).collect();
            chain.push(config_path.display().to_string());
            return Err(DotfilesError::Config(format!(
                "Detected configuration include cycle: {}",
                chain.join(" -> ")
            )));
        }
        visited.push(config_path.clone());

        if !config_path.exists() {
            // For the root config, create a default file if it doesn't exist.
            // For included configs, treat missing files as an error.
            if visited.len() == 1 {
                let config = Self::default();
                config.save(false)?;
                let _ = visited.pop();
                return Ok(config);
            }
            let _ = visited.pop();
            return Err(DotfilesError::Config(format!(
                "Included config file does not exist: {}",
                config_path.display()
            )));
        }

        let content = fs::read_to_string(&config_path).map_err(|error| {
            DotfilesError::Config(format!(
                "Failed to read config {}: {}",
                config_path.display(),
                error
            ))
        })?;

        let mut current: Self = toml::from_str(&content).map_err(|error| {
            DotfilesError::Config(format!(
                "Failed to parse config {}: {}",
                config_path.display(),
                error
            ))
        })?;

        // First, load and merge any included configs (if present).
        let mut merged: Option<Self> = None;
        if let Some(include_paths) = current.general.include.clone() {
            let base_dir = config_path.parent().map_or_else(|| PathBuf::from("."), PathBuf::from);

            for raw in include_paths {
                let include_path = Self::resolve_include_path(&base_dir, &raw);
                let included = Self::load_from_path(&include_path, visited)?;
                merged = Some(match merged {
                    Some(base) => Self::merge_configs(base, included),
                    None => included,
                });
            }
        }

        // We have already processed includes, so clear them to avoid re-saving them.
        current.general.include = None;

        let result = match merged {
            Some(base) => Self::merge_configs(base, current),
            None => current,
        };

        // Validate the final merged config
        result.validate()?;

        let _ = visited.pop();

        Ok(result)
    }

    /// Save the configuration to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if there are issues writing the configuration file or
    /// if validation fails.
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
            let content = fs::read_to_string(&config_path).map_err(|error| {
                DotfilesError::Config(format!(
                    "Failed to read config {}: {}",
                    config_path.display(),
                    error
                ))
            })?;
            content.parse::<toml_edit::DocumentMut>().map_err(|error| {
                DotfilesError::Config(format!("Failed to parse existing config: {error}"))
            })?
        } else {
            // Create new document
            toml_edit::DocumentMut::new()
        };

        // Serialize current config to TOML and merge into existing document
        let new_toml = toml::to_string_pretty(self).map_err(|error| {
            DotfilesError::Config(format!("Failed to serialize config: {error}"))
        })?;
        let mut new_doc = new_toml.parse::<toml_edit::DocumentMut>().map_err(|error| {
            DotfilesError::Config(format!("Failed to parse serialized config: {error}"))
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
        Ok(())
    }

    /// Format tools section to use per-tool format `[tools.X] files = [...]`
    /// instead of array-of-tables `[[tools.X.files]]`.
    fn format_tools_section(doc: &mut toml_edit::DocumentMut, tools: &HashMap<String, ToolConfig>) {
        use toml_edit::{Array, Item, Table, Value};

        // Remove existing tools section if present
        let _ = doc.remove("tools");

        // Create new tools table
        let mut tools_table = Table::new();
        tools_table.set_implicit(true);

        for (tool_name, tool_config) in tools {
            // Create table for this tool
            let mut current_tool_table = Table::new();
            current_tool_table.set_implicit(true);

            // Create array of file entries
            let mut files_array = Array::new();
            files_array.set_trailing_comma(true);
            files_array.set_trailing("\n");

            for file_entry in &tool_config.files {
                let mut file_table = toml_edit::InlineTable::new();

                // Add repo field
                let _ = file_table.insert(
                    "repo",
                    Value::String(toml_edit::Formatted::new(file_entry.repo.clone())),
                );

                // Add dest field
                let _ = file_table.insert(
                    "dest",
                    Value::String(toml_edit::Formatted::new(file_entry.dest.clone())),
                );

                // Add profile field if present
                if let Some(profile) = &file_entry.profile {
                    let _ = file_table.insert(
                        "profile",
                        Value::String(toml_edit::Formatted::new(profile.clone())),
                    );
                }

                files_array.push_formatted(Value::InlineTable(file_table));
            }

            let _ = current_tool_table.insert("files", Item::Value(Value::Array(files_array)));
            let _ = tools_table.insert(tool_name, Item::Table(current_tool_table));
        }

        let _ = doc.insert("tools", Item::Table(tools_table));
    }

    /// Remove old array-of-tables format `[[tools.X.files]]` from document.
    fn remove_old_tools_format(doc: &mut toml_edit::DocumentMut) {
        // Array-of-tables like [[tools.cursor.files]] are represented as arrays
        // in the parsed document. We need to find and remove these.
        // Get all keys that match the pattern tools.*.files
        let keys_to_remove: Vec<String> = doc
            .iter()
            .filter_map(|(key, _)| {
                // Check for keys like "tools.cursor.files" (array-of-tables format)
                if key.starts_with("tools.")
                    && Path::new(key)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("files"))
                {
                    Some(key.to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            let _ = doc.remove(&key);
        }
    }

    /// Recursively merge two TOML documents, preserving comments from the original.
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

    /// Recursively merge two TOML tables.
    fn merge_toml_tables(target: &mut toml_edit::Table, source: &toml_edit::Table) {
        for (key, source_item) in source {
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
                    let _ = target.insert(key, source_item.clone());
                }
                (None, _) => {
                    // Key doesn't exist in target, add it
                    let _ = target.insert(key, source_item.clone());
                }
            }
        }
    }

    /// Get the default config path.
    ///
    /// Checks locations in order: `DOTFILES_CONFIG` env var (handled in `load()`),
    /// `~/.config/flux/config.toml` (authoritative), `~/.dotfiles/config.toml` (fallback).
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory or home directory cannot be found.
    pub fn get_config_path() -> Result<PathBuf> {
        // Repo config is the source of truth - prefer it if it exists
        let repo_config = Self::get_repo_config_path()?;
        if repo_config.exists() {
            return Ok(repo_config);
        }

        // Fall back to XDG config
        let config_dir = dirs::config_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find config directory".to_string()))?;
        let xdg_config = config_dir.join("flux/config.toml");
        if xdg_config.exists() {
            return Ok(xdg_config);
        }

        // Return repo config path as default (even if it doesn't exist)
        let home = dirs::home_dir()
            .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
        let repo_config = home.join(".dotfiles").join("config.toml");

        if repo_config.exists() {
            return Ok(repo_config);
        }

        // If neither exists, default to XDG location
        Ok(xdg_config)
    }

    /// Get the XDG config path (fallback location)
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be found.
    pub fn get_xdg_config_path() -> Result<PathBuf> {
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

    /// Get the repository path, expanding ~ if present.
    ///
    /// Get the repository path, expanding `~` to the home directory.
    #[must_use]
    pub fn get_repo_path(&self) -> PathBuf {
        let expanded = shellexpand::tilde(&self.general.repo_path).into_owned();
        PathBuf::from(expanded)
    }

    /// Get the backup directory path.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup directory cannot be determined.
    pub fn get_backup_dir(&self) -> Result<PathBuf> {
        let backup_dir = if let Some(ref dir) = self.general.backup_dir {
            shellexpand::tilde(dir).into_owned()
        } else {
            // Default to XDG data directory
            let data_dir = dirs::data_local_dir().ok_or_else(|| {
                DotfilesError::Config("Could not find data directory".to_string())
            })?;
            data_dir.join("flux").join("backups").to_string_lossy().to_string()
        };
        Ok(PathBuf::from(backup_dir))
    }

    /// Validate that the configuration is correct.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration has invalid values.
    pub fn validate(&self) -> Result<()> {
        if self.general.repo_path.is_empty() {
            return Err(DotfilesError::Config(
                "repo_path cannot be empty".to_string(),
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
            .all(|char| char.is_alphanumeric() || char == '_' || char == '-')
        {
            return Err(DotfilesError::Config(format!(
                "Invalid profile name '{}': only alphanumeric, underscore, and hyphen allowed",
                self.general.current_profile
            )));
        }

        Ok(())
    }

    /// Add a file to a tool's configuration.
    ///
    /// Add a file entry to a tool configuration.
    pub fn add_file_to_tool(
        &mut self,
        tool: &str,
        repo_file: &str,
        dest_path: &Path,
        profile: Option<&str>,
    ) {
        let dest_str = dest_path.to_string_lossy().to_string();

        let entry = FileEntry {
            repo: repo_file.to_string(),
            dest: dest_str,
            profile: profile.map(ToString::to_string),
            conditions: None,
            secret: false,
            secret_backend: None,
            secret_key: None,
        };

        self.tools
            .entry(tool.to_string())
            .or_insert(ToolConfig { files: Vec::new() })
            .files
            .push(entry);
    }

    /// Get all tracked files for the given profile.
    ///
    /// # Errors
    ///
    /// Returns an error if there are issues reading the repository path or
    /// if file paths are invalid.
    pub fn get_tracked_files(
        &self,
        profile: Option<&str>,
    ) -> Result<Vec<crate::types::TrackedFile>> {
        let repo_path = self.get_repo_path();
        let current_profile = match profile {
            Some(path) => path,
            None => &self.general.current_profile,
        };

        // Build template context once per call for deterministic rendering.
        let template_context = Self::build_template_context(current_profile);

        let mut tracked_files = Vec::new();

        for (tool, tool_config) in &self.tools {
            for file in &tool_config.files {
                // Include if no profile specified, or if profile matches current_profile or is None
                let profile_matches =
                    file.profile.is_none() || file.profile.as_deref() == Some(current_profile);

                if !profile_matches {
                    continue;
                }

                // Evaluate optional conditions
                if let Some(conditions) = &file.conditions
                    && !Self::conditions_match(conditions)
                {
                    continue;
                }

                // Render templates in repo and dest paths, if any
                let rendered_repo =
                    Self::render_template(&file.repo, &template_context).map_err(|error| {
                        DotfilesError::Config(format!(
                            "Failed to render template for repo path '{}': {}",
                            file.repo, error
                        ))
                    })?;
                let rendered_dest =
                    Self::render_template(&file.dest, &template_context).map_err(|error| {
                        DotfilesError::Config(format!(
                            "Failed to render template for dest path '{}': {}",
                            file.dest, error
                        ))
                    })?;

                // Handle both cases: repo may or may not include the tool name prefix
                let repo_file_path = if let Some(stripped) = rendered_repo.strip_prefix("./") {
                    // repo starts with ./ - treat as repo root relative (e.g., "./config.toml")
                    repo_path.join(stripped)
                } else if let Some(stripped) = rendered_repo.strip_prefix(&format!("{tool}/")) {
                    // repo already includes tool name (e.g., "cursor/settings.json")
                    repo_path.join(stripped)
                } else {
                    // repo doesn't include tool name (e.g., "config")
                    repo_path.join(tool).join(&rendered_repo)
                };

                let dest_path = dirs::home_dir()
                    .ok_or_else(|| {
                        DotfilesError::Config("Could not find home directory".to_string())
                    })?
                    .join(&rendered_dest);

                tracked_files.push(crate::types::TrackedFile {
                    tool: tool.clone(),
                    repo_path: repo_file_path,
                    dest_path,
                    profile: file.profile.clone(),
                    is_secret: file.secret,
                    secret_backend: file.secret_backend.clone(),
                    secret_key: file.secret_key.clone(),
                });
            }
        }

        Ok(tracked_files)
    }

    /// Build the template context used for rendering repo/dest paths.
    fn build_template_context(current_profile: &str) -> HashMap<String, String> {
        use std::collections::HashMap;

        let mut values: HashMap<String, String> = HashMap::new();

        // Built-in values
        if let Some(home) = dirs::home_dir() {
            let _ = values.insert("home".to_string(), home.display().to_string());
        }

        if let Ok(user) = std::env::var("USER") {
            let _ = values.insert("user".to_string(), user.clone());
            let _ = values.insert("username".to_string(), user);
        }

        // Hostname (best-effort)
        if let Ok(hostname) = Self::detect_hostname() {
            let _ = values.insert("host".to_string(), hostname);
        }

        let _ = values.insert("profile".to_string(), current_profile.to_string());
        let _ = values.insert("os".to_string(), std::env::consts::OS.to_string());

        values
    }

    /// Very small, deterministic template renderer supporting `{{ var }}` placeholders.
    fn render_template(input: &str, values: &HashMap<String, String>) -> Result<String> {
        if !input.contains("{{") {
            return Ok(input.to_string());
        }

        let mut output = String::new();
        let mut rest = input;

        while let Some(start) = rest.find("{{") {
            output.push_str(&rest[..start]);
            let after_start = &rest[start + 2..];
            let end_rel = after_start.find("}}").ok_or_else(|| {
                DotfilesError::Config(format!("Unclosed template variable in '{input}'"))
            })?;
            let key_raw = &after_start[..end_rel];
            let key = key_raw.trim();
            if key.is_empty() {
                return Err(DotfilesError::Config(
                    "Empty template variable name is not allowed".to_string(),
                ));
            }

            let value = values.get(key).ok_or_else(|| {
                DotfilesError::Config(format!("Unknown template variable '{{{{{key}}}}}'"))
            })?;
            output.push_str(value);

            rest = &after_start[end_rel + 2..];
        }

        output.push_str(rest);
        Ok(output)
    }

    /// Determine if a set of conditions matches the current runtime environment.
    #[must_use]
    pub fn conditions_match(conditions: &ConditionSpec) -> bool {
        // OS check
        if !conditions.os.is_empty() {
            let current_os = std::env::consts::OS.to_string();
            let mut os_ok = false;
            for os in &conditions.os {
                if os.eq_ignore_ascii_case(&current_os) {
                    os_ok = true;
                    break;
                }
                // Support "darwin" alias for macOS
                if current_os == "macos" && os.eq_ignore_ascii_case("darwin") {
                    os_ok = true;
                    break;
                }
            }
            if !os_ok {
                return false;
            }
        }

        // Hostname check
        if !conditions.host.is_empty() {
            let Ok(hostname) = Self::detect_hostname() else {
                return false;
            };
            if !conditions
                .host
                .iter()
                .any(|hostname_item| hostname_item.eq_ignore_ascii_case(&hostname))
            {
                return false;
            }
        }

        // Environment variables check
        for (key, expected) in &conditions.env {
            match std::env::var(key) {
                Ok(actual) if actual == *expected => {}
                _ => return false,
            }
        }

        // Command existence check
        for cmd in &conditions.cmd_exists {
            if !Self::command_in_path(cmd) {
                return false;
            }
        }

        true
    }

    /// Best-effort hostname detection using environment variables and common files.
    fn detect_hostname() -> Result<String> {
        // Prefer HOSTNAME environment variable if set.
        if let Ok(host) = std::env::var("HOSTNAME") {
            let trimmed = host.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }

        // Fallback: try reading /etc/hostname on Unix-like systems.
        #[cfg(unix)]
        {
            if let Ok(contents) = fs::read_to_string("/etc/hostname") {
                let trimmed = contents.trim();
                if !trimmed.is_empty() {
                    return Ok(trimmed.to_string());
                }
            }
        }

        Err(DotfilesError::Config(
            "Failed to determine hostname; set HOSTNAME environment variable to override."
                .to_string(),
        ))
    }

    /// Check if a command exists in PATH (simple PATH scan).
    fn command_in_path(cmd: &str) -> bool {
        if cmd.is_empty() {
            return false;
        }

        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(':') {
                if dir.is_empty() {
                    continue;
                }
                let candidate = Path::new(dir).join(cmd);
                if candidate.is_file() {
                    return true;
                }
            }
        }

        false
    }

    /// Resolve an include path relative to a base directory, supporting `~` expansion.
    fn resolve_include_path(base_dir: &Path, raw: &str) -> PathBuf {
        use shellexpand::tilde;

        // Expand leading ~
        let expanded = if raw.starts_with('~') {
            tilde(raw).to_string()
        } else {
            raw.to_string()
        };

        let path = PathBuf::from(&expanded);
        if path.is_absolute() {
            path
        } else {
            base_dir.join(path)
        }
    }

    /// Merge two configs, with `overlay` taking precedence over `base`.
    fn merge_configs(mut base: Self, overlay: Self) -> Self {
        // General settings: overlay wins entirely.
        base.general = overlay.general;

        // Tools: merge tool tables, appending file entries.
        for (tool, tool_cfg) in overlay.tools {
            base.tools
                .entry(tool)
                .or_insert(ToolConfig { files: Vec::new() })
                .files
                .extend(tool_cfg.files);
        }

        base
    }

    /// Sync repo config to XDG location (copy repo config to XDG).
    ///
    /// Useful for manually syncing the repo config to the XDG location for tools
    /// that might check there.
    ///
    /// # Errors
    ///
    /// Returns an error if there are issues reading or writing configuration files.
    pub fn sync_repo_to_xdg(dry_run: bool) -> Result<()> {
        let repo_config = Self::get_repo_config_path()?;
        let xdg_config = Self::get_xdg_config_path()?;

        if !repo_config.exists() {
            return Err(DotfilesError::Config(
                "Repo config does not exist. Nothing to sync.".to_string(),
            ));
        }

        if dry_run {
            println!(
                "{} [DRY RUN] Would copy {} to {}",
                "⊘".yellow(),
                repo_config.display(),
                xdg_config.display()
            );
            return Ok(());
        }

        // Create parent directory if needed
        if let Some(parent) = xdg_config.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                DotfilesError::Config(format!("Failed to create XDG config directory: {error}"))
            })?;
        }

        // Copy repo config to XDG
        fs::copy(&repo_config, &xdg_config).map(|_| ()).map_err(|error| {
            DotfilesError::Config(format!("Failed to copy repo config to XDG: {error}"))
        })?;

        println!(
            "{} Synced repo config to XDG: {} -> {}",
            "✓".green(),
            repo_config.display(),
            xdg_config.display()
        );

        Ok(())
    }
}
