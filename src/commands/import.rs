use crate::config::Config;
use crate::types::FileEntry;
use crate::utils::error::{DotfilesError, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Import files from a GNU Stow layout into flux configuration.
///
/// Stow layouts typically have files organized as:
///   stow-dir/
///     tool1/
///       .config/
///         app/
///           config
///     tool2/
///       .bashrc
///
/// This function scans the stow directory and generates a flux config
/// that mirrors the structure.
///
/// # Errors
///
/// Returns an error if the stow directory does not exist, is not a directory,
/// or if there are issues writing the configuration file.
pub fn import_stow(stow_dir: &Path, repo_path: &Path, dry_run: bool) -> Result<()> {
    if !stow_dir.exists() {
        return Err(DotfilesError::Path(format!(
            "Stow directory does not exist: {}",
            stow_dir.display()
        )));
    }

    if !stow_dir.is_dir() {
        return Err(DotfilesError::Path(format!(
            "Path is not a directory: {}",
            stow_dir.display()
        )));
    }

    println!(
        "{} Scanning Stow directory: {}",
        "→".cyan(),
        stow_dir.display()
    );

    let mut tool_configs: std::collections::HashMap<String, Vec<FileEntry>> =
        std::collections::HashMap::new();

    // Scan each tool directory in stow
    for entry in fs::read_dir(stow_dir)? {
        let entry = entry?;
        let tool_path = entry.path();

        if !tool_path.is_dir() {
            continue;
        }

        let tool_name = tool_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| DotfilesError::Path("Invalid tool directory name".to_string()))?
            .to_string();

        println!("  Processing tool: {}", tool_name.cyan());

        // Walk the tool directory and find all files
        let mut files = Vec::new();
        collect_stow_files(&tool_path, &tool_path, &mut files)?;

        for (repo_rel, dest_rel) in files {
            let entry = FileEntry {
                repo: repo_rel.clone(),
                dest: dest_rel.clone(),
                profile: None,
                conditions: None,
                secret: false,
                secret_backend: None,
                secret_key: None,
            };
            tool_configs
                .entry(tool_name.clone())
                .or_default()
                .push(entry);
        }
    }

    if tool_configs.is_empty() {
        println!("{} No tools found in Stow directory", "⊘".yellow());
        return Ok(());
    }

    // Display what would be imported
    println!("\n{} Import summary:", "→".cyan().bold());
    for (tool, entries) in &tool_configs {
        println!(
            "  {} {}: {} file(s)",
            "Tool:".bold(),
            tool.cyan(),
            entries.len()
        );
    }

    if dry_run {
        println!(
            "\n{} DRY RUN - No changes will be made",
            "⚠".yellow().bold()
        );
        return Ok(());
    }

    // Load config
    let mut config = Config::load()?;

    // Save count before moving
    let tool_count = tool_configs.len();

    // Add tools to config
    for (tool_name, entries) in tool_configs {
        let tool_config = crate::config::ToolConfig { files: entries };
        config.tools.insert(tool_name.clone(), tool_config);

        // Copy files from stow to repo
        let stow_tool_dir = stow_dir.join(&tool_name);
        let repo_tool_dir = repo_path.join(&tool_name);

        if !repo_tool_dir.exists() {
            fs::create_dir_all(&repo_tool_dir)?;
        }

        // Copy all files preserving structure
        copy_stow_files(&stow_tool_dir, &repo_tool_dir)?;
    }

    // Save config
    config.save(true)?;

    println!("\n{} Import complete!", "✓".green());
    println!("  {tool_count} tool(s) imported");

    Ok(())
}

/// Scan home directory for known dotfile patterns and suggest a config.
///
/// # Errors
///
/// Returns an error if there are issues reading the home directory or
/// writing the configuration file.
pub fn import_scan(home_dir: &Path, repo_path: &Path, dry_run: bool) -> Result<()> {
    println!("{} Scanning home directory for dotfiles...", "→".cyan());

    // Common dotfile patterns
    let patterns = vec![
        (".bashrc", "bash"),
        (".bash_profile", "bash"),
        (".zshrc", "zsh"),
        (".vimrc", "vim"),
        (".vim", "vim"),
        (".gitconfig", "git"),
        (".tmux.conf", "tmux"),
        (".config/nvim", "nvim"),
        (".config/sway", "sway"),
        (".config/waybar", "waybar"),
        (".config/alacritty", "alacritty"),
        (".config/kitty", "kitty"),
    ];

    let mut tool_configs: std::collections::HashMap<String, Vec<FileEntry>> =
        std::collections::HashMap::new();

    for (pattern, tool_name) in patterns {
        let full_path = home_dir.join(pattern);
        if full_path.exists() {
            let entry = FileEntry {
                repo: pattern.to_string(),
                dest: pattern.to_string(),
                profile: None,
                conditions: None,
                secret: false,
                secret_backend: None,
                secret_key: None,
            };
            tool_configs
                .entry(tool_name.to_string())
                .or_default()
                .push(entry);
        }
    }

    if tool_configs.is_empty() {
        println!("{} No common dotfiles found", "⊘".yellow());
        return Ok(());
    }

    println!("\n{} Found dotfiles:", "→".cyan().bold());
    for (tool, entries) in &tool_configs {
        println!(
            "  {} {}: {} file(s)",
            "Tool:".bold(),
            tool.cyan(),
            entries.len()
        );
        for entry in entries {
            println!("    - {}", entry.dest);
        }
    }

    if dry_run {
        println!(
            "\n{} DRY RUN - No changes will be made",
            "⚠".yellow().bold()
        );
        return Ok(());
    }

    // Load config
    let mut config = Config::load()?;

    // Add tools to config
    for (tool_name, entries) in tool_configs {
        let tool_config = crate::config::ToolConfig { files: entries };
        config.tools.insert(tool_name.clone(), tool_config);

        // Copy files from home to repo
        if let Some(tool_config) = config.tools.get(&tool_name) {
            for entry in &tool_config.files {
                let home_file = home_dir.join(&entry.dest);
                let repo_file = repo_path.join(&tool_name).join(&entry.repo);

                if home_file.exists() {
                    if let Some(parent) = repo_file.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    if home_file.is_dir() {
                        copy_dir_all(&home_file, &repo_file)?;
                    } else {
                        fs::copy(&home_file, &repo_file)?;
                    }
                }
            }
        }
    }

    // Save config
    config.save(true)?;

    println!("\n{} Import complete!", "✓".green());

    Ok(())
}

/// Collect files from a Stow directory.
///
/// # Errors
///
/// Returns an error if the relative path cannot be computed.
fn collect_stow_files(
    base: &Path,
    current: &Path,
    files: &mut Vec<(String, String)>,
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let repo_rel = path
                .strip_prefix(base)
                .map_err(|_| DotfilesError::Path("Failed to compute relative path".to_string()))?
                .to_string_lossy()
                .to_string();
            // In stow, the repo structure mirrors the destination structure
            let dest_rel = if repo_rel.starts_with('.') {
                repo_rel.clone()
            } else {
                format!(".{repo_rel}")
            };
            files.push((repo_rel, dest_rel));
        } else if path.is_dir() {
            collect_stow_files(base, &path, files)?;
        }
    }
    Ok(())
}

/// Copy files from a Stow directory to a repository directory.
///
/// # Errors
///
/// Returns an error if the relative path cannot be computed.
fn copy_stow_files(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(
            src_path
                .file_name()
                .ok_or_else(|| DotfilesError::Path("Invalid file name".to_string()))?,
        );

        if src_path.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_stow_files(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Copy a directory and all its contents to a destination directory.
///
/// # Errors
///
/// Returns an error if the directory cannot be created or if there are issues copying the files.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
