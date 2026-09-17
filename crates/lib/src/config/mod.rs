/// Command-line interface configuration.
pub mod cli;
/// Profile management functionality.
pub mod profile;

pub use cli::EnvironmentConfig;

// The config module itself is in this file
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::types::{
  Condition, ConditionGroup, ConditionSpec, FileEntry, HookCommand, SymlinkResolution,
};
use crate::utils::error::{DotfilesError, Result};
use crate::utils::home_override;
use crate::utils::security;
use crate::utils::system;

/// General configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
  /// Repository path (supports ~ expansion)
  pub repo_path: String,
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

/// Returns the default symlink resolution strategy.
const fn default_symlink_resolution() -> SymlinkResolution {
  SymlinkResolution::Auto
}

impl Default for GeneralConfig {
  fn default() -> Self {
    Self {
      repo_path: "~/.dotfiles".to_string(),
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolConfig {
  /// Files tracked for this tool
  #[serde(default)]
  pub files: Vec<FileEntry>,
  /// Folder entries tracked for this tool
  #[serde(default)]
  pub folders: Vec<FileEntry>,
}

/// Iterate over each entry (files followed by folders).
impl ToolConfig {
  /// Iterate over each entry (files followed by folders).
  fn entries(&self) -> impl Iterator<Item = &FileEntry> {
    self.files.iter().chain(self.folders.iter())
  }
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
    Self::load_with_override_internal(override_path, true)
  }

  /// Load configuration without creating a missing root config file.
  ///
  /// This is intended for read-only commands and shell hooks that should not
  /// create configuration files as a side effect.
  ///
  /// # Errors
  ///
  /// Returns an error if the configuration file cannot be loaded.
  pub fn load_existing() -> Result<Self> {
    let override_path = CONFIG_PATH_OVERRIDE.get().map(PathBuf::as_path);
    Self::load_with_override_internal(override_path, false)
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
    Self::load_with_override_internal(config_path_override, true)
  }

  /// Load configuration with optional path override, with repo config as source of truth.
  fn load_with_override_internal(
    config_path_override: Option<&Path>,
    create_missing_root: bool,
  ) -> Result<Self> {
    // Check for custom config path from parameter or environment variable
    let config_path = if let Some(override_path) = config_path_override {
      override_path.to_path_buf()
    } else if let Ok(config_path_str) = std::env::var(cli::env_keys::CONFIG_FILE) {
      PathBuf::from(&config_path_str)
    } else {
      // Use repo config as source of truth, fall back to XDG
      Self::get_config_path()?
    };

    Self::load_from_path(&config_path, &mut Vec::new(), create_missing_root)
  }

  /// Load configuration from a specific path.
  ///
  /// Supports includes with cycle-safe merging. Later configs override earlier ones.
  fn load_from_path(
    config_path: &Path,
    visited: &mut Vec<PathBuf>,
    create_missing_root: bool,
  ) -> Result<Self> {
    let config_path = config_path
      .canonicalize()
      .unwrap_or_else(|_| config_path.to_path_buf());

    Self::ensure_no_cycle(&config_path, visited)?;
    visited.push(config_path.clone());

    let result = (|| -> Result<Self> {
      if !config_path.exists() {
        return Self::handle_missing_config(visited.len(), &config_path, create_missing_root);
      }

      let mut current = Self::parse_config(&Self::read_config_file(&config_path)?, &config_path)?;
      Self::migrate_legacy_conditions(&mut current);

      let base_dir = config_path
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
      let merged_include = Self::merge_included_configs(
        &base_dir,
        current.general.include.clone(),
        visited,
        create_missing_root,
      )?;
      current.general.include = None;

      let result = match merged_include {
        Some(base) => Self::merge_configs(base, current),
        None => current,
      };

      result.validate()?;
      Ok(result)
    })();

    let _ = visited.pop();

    result
  }

  /// Return an error if the path has already been loaded in this recursion.
  fn ensure_no_cycle(
    config_path: &Path,
    visited: &[PathBuf],
  ) -> Result<()> {
    if visited.contains(&config_path.to_path_buf()) {
      let mut chain: Vec<String> = visited
        .iter()
        .map(|path| path.display().to_string())
        .collect();
      chain.push(config_path.display().to_string());
      Err(DotfilesError::Config(format!(
        "Detected configuration include cycle: {}",
        chain.join(" -> ")
      )))
    } else {
      Ok(())
    }
  }

  /// Create the root config if missing or error for included files.
  fn handle_missing_config(
    visited_len: usize,
    config_path: &Path,
    create_missing_root: bool,
  ) -> Result<Self> {
    if visited_len == 1 {
      if create_missing_root {
        let config = Self::default();
        config.save(false)?;
        Ok(config)
      } else {
        Err(DotfilesError::Config(format!(
          "Configuration file not found: {}",
          config_path.display()
        )))
      }
    } else {
      Err(DotfilesError::Config(format!(
        "Included config file does not exist: {}",
        config_path.display()
      )))
    }
  }

  /// Read the config file contents or map any I/O errors.
  fn read_config_file(config_path: &Path) -> Result<String> {
    fs::read_to_string(config_path).map_err(|error| {
      DotfilesError::Config(format!(
        "Failed to read config {}: {}",
        config_path.display(),
        error
      ))
    })
  }

  /// Parse TOML content into a `Config`, wrapping parsing errors.
  fn parse_config(
    content: &str,
    config_path: &Path,
  ) -> Result<Self> {
    toml::from_str(content).map_err(|error| {
      DotfilesError::Config(format!(
        "Failed to parse config {}: {}",
        config_path.display(),
        error
      ))
    })
  }

  /// Update any stored condition specs to the latest schema.
  fn migrate_legacy_conditions(config: &mut Self) {
    for tool_config in config.tools.values_mut() {
      for entry in tool_config
        .files
        .iter_mut()
        .chain(tool_config.folders.iter_mut())
      {
        if let Some(ref mut conditions) = entry.conditions {
          conditions.migrate_legacy();
        }
      }
    }

    if let Some(ref mut hooks) = config.hooks {
      Self::migrate_hook_group(&mut hooks.pre_apply);
      Self::migrate_hook_group(&mut hooks.post_apply);
      Self::migrate_hook_group(&mut hooks.pre_commit);
      Self::migrate_hook_group(&mut hooks.post_commit);
    }
  }

  /// Migrate any per-hook condition specs.
  fn migrate_hook_group(group: &mut Option<Vec<HookCommand>>) {
    if let Some(hooks) = group {
      for hook in hooks {
        if let Some(ref mut conditions) = hook.conditions {
          conditions.migrate_legacy();
        }
      }
    }
  }

  /// Load and merge any included configs listed in `general.include`.
  fn merge_included_configs(
    base_dir: &Path,
    include_paths: Option<Vec<String>>,
    visited: &mut Vec<PathBuf>,
    create_missing_root: bool,
  ) -> Result<Option<Self>> {
    if let Some(include_paths) = include_paths {
      let mut merged = None;
      for raw in include_paths {
        let include_path = Self::resolve_include_path(base_dir, &raw);
        let included = Self::load_from_path(&include_path, visited, create_missing_root)?;
        merged = Some(match merged {
          Some(base) => Self::merge_configs(base, included),
          None => included,
        });
      }
      Ok(merged)
    } else {
      Ok(None)
    }
  }

  /// Save the configuration to disk.
  ///
  /// # Errors
  ///
  /// Returns an error if there are issues writing the configuration file or
  /// if validation fails.
  pub fn save(
    &self,
    validate: bool,
  ) -> Result<()> {
    if validate {
      self.validate()?;
    }

    // Always save to repo config as the source of truth
    let config_path = Self::get_repo_config_path()?;

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
      content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| {
          DotfilesError::Config(format!("Failed to parse existing config: {error}"))
        })?
    } else {
      // Create new document
      toml_edit::DocumentMut::new()
    };

    // Serialize current config to TOML and merge into existing document
    let new_toml = toml::to_string_pretty(self)
      .map_err(|error| DotfilesError::Config(format!("Failed to serialize config: {error}")))?;
    let mut new_doc = new_toml
      .parse::<toml_edit::DocumentMut>()
      .map_err(|error| {
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

  /// Format tools section to use per-tool format `[tools.X]` tables.
  fn format_tools_section(
    doc: &mut toml_edit::DocumentMut,
    tools: &HashMap<String, ToolConfig>,
  ) {
    use toml_edit::{Array, Item, Table, Value};

    let _ = doc.remove("tools");

    let mut tools_table = Table::new();
    tools_table.set_implicit(true);

    for (tool_name, tool_config) in tools {
      let mut current_tool_table = Table::new();
      current_tool_table.set_implicit(true);

      if !tool_config.files.is_empty() {
        let mut files_array = Array::new();
        files_array.set_trailing_comma(true);
        files_array.set_trailing("\n");

        for file_entry in &tool_config.files {
          let mut file_table = toml_edit::InlineTable::new();

          let _ = file_table.insert(
            "repo",
            Value::String(toml_edit::Formatted::new(file_entry.repo.clone())),
          );

          let _ = file_table.insert(
            "dest",
            Value::String(toml_edit::Formatted::new(file_entry.dest.clone())),
          );

          if let Some(profile) = &file_entry.profile {
            let _ = file_table.insert(
              "profile",
              Value::String(toml_edit::Formatted::new(profile.clone())),
            );
          }

          files_array.push_formatted(Value::InlineTable(file_table));
        }

        let _ = current_tool_table.insert("files", Item::Value(Value::Array(files_array)));
      }

      if !tool_config.folders.is_empty() {
        let mut folders_array = Array::new();
        folders_array.set_trailing_comma(true);
        folders_array.set_trailing("\n");

        for folder_entry in &tool_config.folders {
          let mut folder_table = toml_edit::InlineTable::new();

          let _ = folder_table.insert(
            "repo",
            Value::String(toml_edit::Formatted::new(folder_entry.repo.clone())),
          );

          let _ = folder_table.insert(
            "dest",
            Value::String(toml_edit::Formatted::new(folder_entry.dest.clone())),
          );

          if let Some(profile) = &folder_entry.profile {
            let _ = folder_table.insert(
              "profile",
              Value::String(toml_edit::Formatted::new(profile.clone())),
            );
          }

          folders_array.push_formatted(Value::InlineTable(folder_table));
        }

        let _ = current_tool_table.insert("folders", Item::Value(Value::Array(folders_array)));
      }

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
          && Path::new(key).extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("files") || ext.eq_ignore_ascii_case("folders")
          })
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
  fn merge_toml_documents(
    target: &mut toml_edit::DocumentMut,
    source: &toml_edit::DocumentMut,
  ) {
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
  fn merge_toml_tables(
    target: &mut toml_edit::Table,
    source: &toml_edit::Table,
  ) {
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
    // Repo config is the source of truth - always use it if it exists
    let repo_config = Self::get_repo_config_path()?;
    if repo_config.exists() {
      return Ok(repo_config);
    }

    // Fall back to XDG config if repo config doesn't exist (new user scenario)
    let config_dir = dirs::config_dir()
      .ok_or_else(|| DotfilesError::Config("Could not find config directory".to_string()))?;
    let xdg_config = config_dir.join("flux/config.toml");
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
    let home = home_override::home_dir()
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

  /// Builds a [`FileEntry`] for a tool configuration.
  fn build_tool_entry(
    repo_file: &str,
    dest_path: &Path,
    profile: Option<&str>,
  ) -> FileEntry {
    FileEntry {
      repo: repo_file.to_string(),
      dest: dest_path.to_string_lossy().to_string(),
      profile: profile.map(ToString::to_string),
      conditions: None,
    }
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
    let entry = Self::build_tool_entry(repo_file, dest_path, profile);
    self
      .tools
      .entry(tool.to_string())
      .or_default()
      .files
      .push(entry);
  }

  /// Add a folder entry to the configuration for a tool.
  pub fn add_folder_to_tool(
    &mut self,
    tool: &str,
    repo_file: &str,
    dest_path: &Path,
    profile: Option<&str>,
  ) {
    let entry = Self::build_tool_entry(repo_file, dest_path, profile);
    self
      .tools
      .entry(tool.to_string())
      .or_default()
      .folders
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
    let home = home_override::home_dir()
      .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
    let current_profile = match profile {
      Some(path) => path,
      None => &self.general.current_profile,
    };

    // Build template context once per call for deterministic rendering.
    let template_context = Self::build_template_context(current_profile);

    let mut tracked_files = Vec::new();

    for (tool, tool_config) in &self.tools {
      for file in tool_config.entries() {
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
        } else if rendered_repo == *tool {
          // repo equals tool name (e.g., "nvim"); track the tool directory directly
          repo_path.join(tool)
        } else if rendered_repo.starts_with(&format!("{tool}/")) {
          // repo already includes tool name (e.g., "cursor/settings.json")
          // Use it as-is, don't strip the tool prefix
          repo_path.join(&rendered_repo)
        } else {
          // repo doesn't include tool name (e.g., "config")
          repo_path.join(tool).join(&rendered_repo)
        };

        let dest_path = home.join(&rendered_dest);
        security::validate_repo_entry_path(&repo_path, &repo_file_path)?;
        security::validate_dest_path(&dest_path, &home)?;

        tracked_files.push(crate::types::TrackedFile {
          tool: tool.clone(),
          repo_path: repo_file_path,
          dest_path,
          profile: file.profile.clone(),
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
    if let Some(home) = home_override::home_dir() {
      let _ = values.insert("home".to_string(), home.display().to_string());
    }

    if let Ok(user) = std::env::var("USER") {
      let _ = values.insert("user".to_string(), user.clone());
      let _ = values.insert("username".to_string(), user);
    }

    // Hostname (best-effort)
    let hostname = system::get_hostname();
    if hostname != "unknown" {
      let _ = values.insert("host".to_string(), hostname);
    }

    let _ = values.insert("profile".to_string(), current_profile.to_string());
    let _ = values.insert("os".to_string(), std::env::consts::OS.to_string());

    values
  }

  /// Very small, deterministic template renderer supporting `{{ var }}` placeholders.
  fn render_template(
    input: &str,
    values: &HashMap<String, String>,
  ) -> Result<String> {
    if !input.contains("{{") {
      return Ok(input.to_string());
    }

    let mut output = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("{{") {
      output.push_str(&rest[..start]);
      let after_start = &rest[start + 2..];
      let end_rel = after_start
        .find("}}")
        .ok_or_else(|| DotfilesError::Config(format!("Unclosed template variable in '{input}'")))?;
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
    // If no groups defined, always match
    if conditions.groups.is_empty() {
      return true;
    }

    log::debug!("Evaluating {} condition group(s)", conditions.groups.len());

    // All groups must match (AND logic between groups)
    for (group_idx, group) in conditions.groups.iter().enumerate() {
      log::debug!(
        "Evaluating group {} with {} condition(s)",
        group_idx + 1,
        group.conditions.len()
      );

      if !Self::evaluate_condition_group(group) {
        log::debug!("Group {} did not match", group_idx + 1);
        return false;
      }

      log::debug!("Group {} matched", group_idx + 1);
    }

    log::debug!("All condition groups matched");
    true
  }

  /// Evaluate a single condition group (all conditions must match)
  fn evaluate_condition_group(group: &ConditionGroup) -> bool {
    // All conditions in the group must match (AND logic)
    for condition in &group.conditions {
      let result = Self::evaluate_condition(condition);
      log::debug!(
        "  Condition: {} -> {}",
        condition.describe(),
        if result { "✓" } else { "✗" }
      );
      if !result {
        return false;
      }
    }
    true
  }

  /// Evaluate a single condition
  fn evaluate_condition(condition: &Condition) -> bool {
    match condition {
      Condition::Os { value } => {
        let current_os = system::get_os();
        // Support case-insensitive matching
        if value
          .to_string()
          .eq_ignore_ascii_case(&current_os)
        {
          return true;
        }
        // Support "darwin" alias for macOS
        if current_os == "macos" && value.to_string().eq_ignore_ascii_case("darwin") {
          return true;
        }
        false
      }
      Condition::Host { value } => {
        let hostname = system::get_hostname();
        value.eq_ignore_ascii_case(&hostname)
      }
      Condition::EnvVar { name, value } => {
        system::get_env_var(name).is_some_and(|actual| &actual == value)
      }
      Condition::CommandExists { command } => system::command_exists(command),
      Condition::FileExists { path } => system::path_exists(path),
      Condition::EnvVarExists { name } => system::has_env_var(name),
      Condition::Architecture { value } => {
        let arch = system::get_architecture();
        value.eq_ignore_ascii_case(&arch)
      }
      Condition::Shell { value } => {
        let shell = system::get_shell();
        value.eq_ignore_ascii_case(&shell)
      }
      Condition::User { username } => {
        let current_user = system::get_username();
        username.eq_ignore_ascii_case(&current_user)
      }
      Condition::Group { groupname } => system::user_in_group(groupname),
    }
  }

  /// Resolve an include path relative to a base directory, supporting `~` expansion.
  fn resolve_include_path(
    base_dir: &Path,
    raw: &str,
  ) -> PathBuf {
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
  fn merge_configs(
    mut base: Self,
    overlay: Self,
  ) -> Self {
    // General settings: overlay wins entirely.
    base.general = overlay.general;

    // Tools: merge tool tables, appending file entries.
    for (tool, tool_cfg) in overlay.tools {
      let entry = base.tools.entry(tool).or_default();
      entry.files.extend(tool_cfg.files);
      entry.folders.extend(tool_cfg.folders);
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
      return Ok(());
    }

    // Create parent directory if needed
    if let Some(parent) = xdg_config.parent() {
      fs::create_dir_all(parent).map_err(|error| {
        DotfilesError::Config(format!("Failed to create XDG config directory: {error}"))
      })?;
    }

    // Copy repo config to XDG
    fs::copy(&repo_config, &xdg_config)
      .map(|_| ())
      .map_err(|error| {
        DotfilesError::Config(format!("Failed to copy repo config to XDG: {error}"))
      })?;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::{Config, ToolConfig};
  use crate::types::FileEntry;
  use crate::utils::error::Result;
  use crate::utils::{home_override, system};

  #[test]
  fn legacy_conditions_are_migrated_for_files_and_folders() {
    let excluded_os = if system::get_os() == "linux" {
      "darwin"
    } else {
      "linux"
    };
    let source = format!(
      r#"
[general]
repo_path = "/tmp/dotfiles"
current_profile = "default"

[tools.example]
files = [
  {{ repo = "file", dest = ".config/example/file", conditions = {{ os = ["{excluded_os}"] }} }},
]
folders = [
  {{ repo = "folder", dest = ".config/example/folder", conditions = {{ os = ["{excluded_os}"] }} }},
]
"#,
    );

    let parsed = toml::from_str(&source);
    assert!(parsed.is_ok(), "config should parse");
    let Ok(mut config) = parsed else {
      return;
    };
    Config::migrate_legacy_conditions(&mut config);

    let tracked_result = config.get_tracked_files(None);
    assert!(tracked_result.is_ok(), "tracked files should resolve");
    let Ok(tracked) = tracked_result else {
      return;
    };
    assert!(
      tracked.is_empty(),
      "legacy conditions should exclude both file and folder entries",
    );
  }

  #[test]
  fn tracked_files_reject_destination_outside_home() -> Result<()> {
    let home = tempfile::tempdir()?;
    let repo = tempfile::tempdir()?;
    home_override::set_override(home.path().to_path_buf());

    let mut config = Config::default();
    config.general.repo_path = repo.path().display().to_string();
    let _ = config.tools.insert(
      "example".to_string(),
      ToolConfig {
        files: vec![FileEntry {
          repo: "file".to_string(),
          dest: "../outside-home".to_string(),
          profile: None,
          conditions: None,
        }],
        folders: Vec::new(),
      },
    );

    let result = config.get_tracked_files(None);
    home_override::clear_override();

    assert!(result.is_err(), "destination traversal must be rejected");
    Ok(())
  }

  #[test]
  fn tracked_files_reject_source_outside_repository() -> Result<()> {
    let home = tempfile::tempdir()?;
    let repo = tempfile::tempdir()?;
    home_override::set_override(home.path().to_path_buf());

    let mut config = Config::default();
    config.general.repo_path = repo.path().display().to_string();
    let _ = config.tools.insert(
      "example".to_string(),
      ToolConfig {
        files: vec![FileEntry {
          repo: "../../outside-repository".to_string(),
          dest: ".config/example".to_string(),
          profile: None,
          conditions: None,
        }],
        folders: Vec::new(),
      },
    );

    let result = config.get_tracked_files(None);
    home_override::clear_override();

    assert!(result.is_err(), "repository traversal must be rejected");
    Ok(())
  }
}
